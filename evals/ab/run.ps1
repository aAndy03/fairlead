# Runs the A/B/C evaluation (evals/ab/PROTOCOL.md): prepares every chain (track x project x arm x
# run), runs its tasks one session at a time, scores each session from outside, and writes the
# results where the dashboard reads them. Watch it live with:
#
#   evals\ab\harness\target\release\ab.exe serve --results $env:TEMP\fairlead-ab
#
# Pilot (default): first 3 tasks, 1 run, both tracks, 54 sessions.
#   powershell -File evals\ab\run.ps1
# Full run:
#   powershell -File evals\ab\run.ps1 -Name full -Tasks 9 -Runs 3
# Continue an interrupted run (finished sessions are skipped; an unfinished chain restarts):
#   powershell -File evals\ab\run.ps1 -Name <same name>
param(
    [string]$Name = ('{0}-pilot' -f (Get-Date -Format 'yyyy-MM-dd')),
    [string[]]$Tracks = @('maint', 'scratch'),
    [string[]]$Projects = @('shelf', 'ledger', 'relay'),
    [string[]]$Arms = @('A', 'B', 'C'),
    [int]$Tasks = 3,
    [int]$Runs = 1,
    [int]$Seed = 1,
    [int]$TimeoutMinutes = 20,
    [string]$Results = (Join-Path ([System.IO.Path]::GetTempPath()) 'fairlead-ab'),
    [string]$Fairlead = ''
)
. "$PSScriptRoot\..\..\xtask\agent\common.ps1"

$Tracks = @($Tracks | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Projects = @($Projects | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Arms = @($Arms | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$repo = Get-RepoRoot
$ab = Join-Path $PSScriptRoot 'harness\target\release\ab.exe'
if (-not $Fairlead) { $Fairlead = Join-Path $repo 'target\release\fairlead.exe' }
if (-not (Test-Path $Fairlead)) { throw "no fairlead at $Fairlead; run: cargo build --release -p fairlead" }
& cargo build --release --offline -q --manifest-path (Join-Path $PSScriptRoot 'harness\Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'the ab harness did not build' }

$runDir = Join-Path $Results $Name
foreach ($d in 'live', 'work', 'prompts', 'review', 'bin-stub', 'bin-c') { New-Item -ItemType Directory -Force (Join-Path $runDir $d) | Out-Null }
# Arms A and B: a `fairlead` that is not found. Arm C: the fairlead under evaluation.
Copy-Item -Force $ab (Join-Path $runDir 'bin-stub\fairlead.exe')
Copy-Item -Force $Fairlead (Join-Path $runDir 'bin-c\fairlead.exe')
$fairleadVersion = (& $Fairlead --version | Out-String).Trim()
$gitDir = Split-Path -Parent (Split-Path -Parent (Get-Command git).Source)
$env:AB_BASH = Join-Path $gitDir 'bin\bash.exe'
$basePath = $env:PATH

function Read-Tasks([string]$project, [string]$track) {
    $file = Join-Path $PSScriptRoot ("projects\$project\" + $(if ($track -eq 'maint') { 'tasks.toml' } else { 'increments.toml' }))
    $json = & $ab tasks $file | Out-String
    @(($json | ConvertFrom-Json).task | Select-Object -First $Tasks)
}

# The plan: one entry per session. Chains are interleaved by position, in a seeded order, so the
# arms of one task run close together in time.
$chains = @()
foreach ($t in $Tracks) { foreach ($p in $Projects) { foreach ($r in 1..$Runs) { foreach ($a in $Arms) {
    $chains += [pscustomobject]@{ track = $t; project = $p; run = $r; arm = $a; key = "$t-$p-$a-r$r"; tasks = (Read-Tasks $p $t) }
} } } }
$order = Get-Shuffled $chains $Seed
$plan = @()
foreach ($pos in 1..$Tasks) {
    foreach ($c in $order) {
        $task = $c.tasks[$pos - 1]
        if (-not $task) { continue }
        $plan += [pscustomobject]@{
            id = "$($c.key)-$($task.id)"; chain = $c.key; track = $c.track; project = $c.project; arm = $c.arm; run = $c.run
            pos = $pos; task = $task.id; team = ($task.team -replace '\s+team$', ''); tier = $task.tier; prompt = $task.prompt
        }
    }
}
Write-Utf8 (Join-Path $runDir 'plan.json') ($plan | ConvertTo-Json -Depth 4)
$sessionsFile = Join-Path $runDir 'sessions.jsonl'
$done = @{}
if (Test-Path $sessionsFile) {
    foreach ($l in Get-Content $sessionsFile) { if ($l) { $done[($l | ConvertFrom-Json).id] = $true } }
}
Write-Host "run $Name : $($plan.Count) sessions, $($done.Count) already done. Results: $runDir"

foreach ($s in $plan) {
    if ($done[$s.id]) { continue }
    $work = Join-Path $runDir "work\$($s.chain)"
    $project = Join-Path $PSScriptRoot "projects\$($s.project)"
    if ($s.pos -eq 1) {
        if (Test-Path $work) { Remove-Item -Recurse -Force $work }
        $out = & $ab prepare --project $project --track $s.track --arm $s.arm --out $work --fairlead (Join-Path $runDir 'bin-c\fairlead.exe') 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) { throw "prepare $($s.chain): $out" }
    } elseif (-not (Test-Path $work)) {
        Write-Host "skip $($s.id): its chain was not prepared (an earlier session is missing)"
        continue
    }
    $promptFile = Join-Path $runDir "prompts\$($s.id).md"
    Write-Utf8 $promptFile ("You are on the $($s.team) team.`n`n$($s.prompt)`n")
    $env:PATH = $(if ($s.arm -eq 'C') { Join-Path $runDir 'bin-c' } else { Join-Path $runDir 'bin-stub' }) + [System.IO.Path]::PathSeparator + $basePath
    $events = Join-Path $runDir "live\$($s.id).json"
    $started = Get-Date
    Invoke-Sample -PromptFile $promptFile -WorkDir $work -OutFile $events -Auto -TimeoutMinutes $TimeoutMinutes -Attempts 1 | Out-Null
    $env:PATH = $basePath

    $earlier = @($plan | Where-Object { $_.chain -eq $s.chain -and $_.pos -lt $s.pos } | ForEach-Object { $_.task })
    if ($s.track -eq 'maint') { $earlier = @('_base') + $earlier }
    $scoreArgs = @('score', '--project', $project, '--dir', $work, '--team', $s.team, '--task', $s.task, '--events', $events)
    if ($earlier.Count) { $scoreArgs += @('--earlier', ($earlier -join ',')) }
    if ($s.tier -eq 'setup') { $scoreArgs += @('--setup', 'true') }
    $scoreJson = & $ab @scoreArgs | Out-String
    $score = $scoreJson | ConvertFrom-Json
    $judge = 'hidden-tests'
    if ($null -eq $score.m1_task_done) {
        # PROTOCOL.md: no hidden tests for this task, so a blind reviewer judges it.
        $judge = 'reviewer'
        $rv = Join-Path $runDir "review\$($s.id)"
        New-Item -ItemType Directory -Force $rv | Out-Null
        Write-Utf8 (Join-Path $rv 'task.md') ("$($s.prompt)`n")
        Write-Utf8 (Join-Path $rv 'change.diff') ((& $ab diff --dir $work | Out-String))
        $ans = Invoke-Sample -PromptFile (Join-Path $PSScriptRoot 'prompts\review.md') -WorkDir $rv `
            -Files @((Join-Path $rv 'task.md'), (Join-Path $rv 'change.diff')) -OutFile (Join-Path $rv 'answer.json') -TimeoutMinutes 6
        $label = Get-Label $ans @('done', 'partial', 'not-done')
        $score.m1_task_done = ($label -eq 'done')
        $score | Add-Member -NotePropertyName m1_label -NotePropertyValue $label -Force
    }
    & git -C $work add -A 2>$null | Out-Null
    & git -C $work -c user.name=ab -c user.email=ab@example.invalid commit -q --allow-empty -m "after $($s.task)" 2>$null | Out-Null

    $record = [ordered]@{
        id = $s.id; chain = $s.chain; track = $s.track; project = $s.project; arm = $s.arm; run = $s.run; pos = $s.pos
        task = $s.task; team = $s.team; tier = $s.tier; prompt = $s.prompt; m1_judge = $judge
        started = $started.ToString('o'); model = $Model; fairlead = $fairleadVersion; seed = $Seed
        events_sha256 = (Get-Sha256 $events); score = $score
    }
    Add-Content -Path $sessionsFile -Value ($record | ConvertTo-Json -Depth 8 -Compress) -Encoding UTF8
    Write-Host ("{0}: M1={1} M2={2} M3 new={3} tokens={4}" -f $s.id, $score.m1_task_done, $score.m2_nothing_broken, $score.m3_new,
        ($score.cost.tokens_in + $score.cost.tokens_out))
}

# Keep the scored results (not the raw logs) in the repository.
$keep = Join-Path $PSScriptRoot "results\$Name"
New-Item -ItemType Directory -Force $keep | Out-Null
Copy-Item -Force (Join-Path $runDir 'plan.json'), $sessionsFile $keep
Write-Host "done. Scored results copied to $keep"
