# Agent protocol A-1 (agent-runbook; R-ROAD-019, R-ROAD-012, gate G2, 60-roadmap 4.2 row 6): an
# eight-week pilot, simulated. A subject agent plays a maintainer on three public repositories (one
# TypeScript/React, one Rust, one mixed) and makes scripted changes, declaring `ovr`s where a rule
# blocks it; each simulated week ends with `fairlead seal`. A reviewer agent then classifies every
# recorded exception as stale, expired or live. Pass: stale or expired at most 30%.
#
#   cargo build --release -p fairlead
#   powershell -File xtask\agent\A-1.ps1 -Repos "<url>,<url>,<url>" `
#       -Bin target\release\fairlead.exe [-Weeks 8] [-PerWeek 5] [-Seed 1] [-Start 2026-10-12]
#
# Cloning the three repositories downloads them; whoever starts the run approves that. Subject
# sessions run with --auto inside a copy outside this repository and see only that copy.
param(
    # One comma-separated string, quoted on the command line: "url1,url2,url3".
    [Parameter(Mandatory)][string]$Repos,
    [Parameter(Mandatory)][string]$Bin,
    [string]$Tasks = '',
    [int]$Weeks = 8,
    [int]$PerWeek = 5,
    [int]$Seed = 1,
    [string]$Start = '2026-10-12'
)
. "$PSScriptRoot\common.ps1"

# Windows PowerShell 5.1 leaves $PSScriptRoot empty in parameter defaults, so the default is set
# here; and `powershell -File` passes `a,b,c` as one string, so the list is split here.
if (-not $Tasks) { $Tasks = Join-Path $PSScriptRoot 'a1-tasks.json' }
$RepoList = @($Repos | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
if ($RepoList.Count -ne 3) { throw 'A-1 runs on exactly three repositories (TS/React, Rust, mixed)' }
$bin = (Resolve-Path $Bin).Path
$env:PATH = (Split-Path -Parent $bin) + [System.IO.Path]::PathSeparator + $env:PATH
$run = New-RunDir 'A-1'
# Windows PowerShell 5.1 emits a JSON array as one object; the ForEach-Object unrolls it.
$taskList = @(Get-Content -Raw -LiteralPath $Tasks | ConvertFrom-Json | ForEach-Object { $_ })
if ($taskList.Count -lt $Weeks * $PerWeek) { throw "need $($Weeks * $PerWeek) tasks, $Tasks has $($taskList.Count)" }
$rules = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'a1-rules.toml')
$subjectBase = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'prompts\A-1-subject.md')
$reviewPrompt = Join-Path $PSScriptRoot 'prompts\A-1-review.md'
$startDate = [datetime]::ParseExact($Start, 'yyyy-MM-dd', $null)
$endDate = $startDate.AddDays(7 * $Weeks).ToString('yyyy-MM-dd')
$map = @('id,kind,repo,detail')
$log = @()

# Arguments travel as arrays, so `--flag` words are never read as PowerShell parameter names.
function G([string]$Dir, [string[]]$A) {
    $all = @('-C', $Dir, '-c', 'user.name=pilot', '-c', 'user.email=pilot@invalid', '-c', 'commit.gpgsign=false') + $A
    & git @all | Out-String
}

function FL([string]$Dir, [string[]]$A) {
    Push-Location $Dir
    try { $o = & $bin @A | Out-String; @{ code = $LASTEXITCODE; out = $o } } finally { Pop-Location }
}

$repoDirs = @()
foreach ($src in (Get-Shuffled $RepoList $Seed)) {
    $rid = Get-OpaqueId $Seed "repo|$src"
    $map += "$rid,repo,$src,"
    $dir = Join-Path $run "work\$rid"
    & git clone -q $src $dir
    if ($LASTEXITCODE -ne 0) { throw "clone failed: $src" }
    FL $dir @('init', '--yes', '--as-of', $Start) | Out-Null
    $ctl = Join-Path $dir 'fairlead\control.toml'
    Write-Utf8 $ctl ((Get-Content -Raw -LiteralPath $ctl) + "`n" + $rules)
    FL $dir @('sync', '--as-of', $Start) | Out-Null
    # The repository's existing violations are history, not the pilot's exceptions.
    FL $dir @('sync', '--baseline', '--accept-new', '--why', 'pilot start', '--as-of', $Start) | Out-Null
    G $dir @('add', '-A') | Out-Null
    G $dir @('commit', '-q', '-m', 'pilot: fairlead init, rules, baseline') | Out-Null
    $repoDirs += , @($rid, $dir)
}

# The subject phase: per repository a seeded task order, $PerWeek tasks a week, a seal each week.
foreach ($rd in $repoDirs) {
    $rid = $rd[0]; $dir = $rd[1]
    $order = Get-Shuffled $taskList ($Seed + [Convert]::ToInt32($rid.Substring(0, 6), 16))
    for ($w = 0; $w -lt $Weeks; $w++) {
        $date = $startDate.AddDays(7 * $w + 4).ToString('yyyy-MM-dd')
        for ($t = 0; $t -lt $PerWeek; $t++) {
            $task = $order[$w * $PerWeek + $t]
            $sid = Get-OpaqueId $Seed "subject|$rid|$w|$t"
            $map += "$sid,subject,$rid,week $($w + 1) $($task.id)"
            $p = Join-Path $run "private\prompts\$sid.md"
            Write-Utf8 $p ($subjectBase.Replace('{{DATE}}', $date).Replace('{{TASK}}', $task.text))
            Invoke-Sample -PromptFile $p -WorkDir $dir -OutFile (Join-Path $run "answers\$sid.json") -Auto | Out-Null
            G $dir @('add', '-A') | Out-Null
            G $dir @('commit', '-q', '--allow-empty', '-m', "week $($w + 1): $($task.id)") | Out-Null
        }
        $seal = FL $dir @('seal', '--yes', '--anywhere', '--as-of', $date)
        $log += "- $rid week $($w + 1) ($date): seal exit $($seal.code)"
        if ($seal.code -eq 0) {
            G $dir @('add', '-A') | Out-Null
            G $dir @('commit', '-q', '-m', "week $($w + 1): seal") | Out-Null
        }
        Write-Host "$rid week $($w + 1): seal exit $($seal.code)"
    }
}

# The reviewer phase: every exception still recorded at the end, judged one per fresh session.
$labels = @{ stale = 0; expired = 0; live = 0; unparsed = 0 }
$checks = @()
foreach ($rd in $repoDirs) {
    $rid = $rd[0]; $dir = $rd[1]
    $json = FL $dir @('check', '--format', 'json', '--as-of', $endDate)
    Write-Utf8 (Join-Path $run "private\$rid-check.json") $json.out
    $terse = (FL $dir @('check', '--format', 'terse', '--max', '0', '--as-of', $endDate)).out
    $checks += "- $rid`: check exit $($json.code)"
    $ledger = (Get-ChildItem -Recurse -File -Path $dir -Filter '*.hl' -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -notmatch '\\.git\\' } | ForEach-Object { Get-Content -Raw -LiteralPath $_.FullName }) -join "`n"
    $sites = @()
    foreach ($hit in @(& git -C $dir grep -n 'fairlead: ovr')) {
        if ($hit -match '^([^:]+):(\d+):') { $sites += , @($Matches[1], [int]$Matches[2]) }
    }
    foreach ($s in $sites) {
        $file = Join-Path $dir $s[0]
        $lines = @(Get-Content -LiteralPath $file)
        $from = [Math]::Max(0, $s[1] - 13); $to = [Math]::Min($lines.Count - 1, $s[1] + 11)
        $eid = Get-OpaqueId $Seed "exception|$rid|$($s[0])|$($s[1])"
        $map += "$eid,exception,$rid,$($s[0]):$($s[1])"
        $findings = ($terse -split "`n" | Where-Object { $_ -like "*$($s[0])*" }) -join "`n"
        $ctx = @("# Exception under review", '', "Date of the review: $endDate", '', "## $($s[0]), lines $($from + 1)-$($to + 1) (the ovr is on line $($s[1]))", '', '```',
            (($from..$to | ForEach-Object { '{0,5}: {1}' -f ($_ + 1), $lines[$_] }) -join "`n"), '```', '',
            '## Findings the tool reports for this file today', '', '```', $(if ($findings) { $findings } else { '(none)' }), '```', '',
            '## Ledger (sealed history)', '', '```', $(if ($ledger) { $ledger } else { '(none)' }), '```')
        $ctxFile = Join-Path $run "work\review-$eid\exception.md"
        Write-Utf8 $ctxFile ($ctx -join "`n")
        $answer = Invoke-Sample -PromptFile $reviewPrompt -WorkDir (Join-Path $run "work\review-$eid") `
            -Files @($ctxFile) -OutFile (Join-Path $run "answers\$eid.json")
        $labels[(Get-Label $answer @('stale', 'expired', 'live'))]++
    }
}
Write-Utf8 (Join-Path $run 'private\map.csv') (($map -join "`n") + "`n")

$total = $labels.stale + $labels.expired + $labels.live
$bad = $labels.stale + $labels.expired
$share = if ($total -gt 0) { [Math]::Round(100.0 * $bad / $total, 1) } else { 0 }
$verdict = if ($total -eq 0) { 'NO DATA: no exception was declared; the pilot exercised nothing (report as missed)' }
    elseif ($share -le 30) { "PASS: $share% stale or expired (threshold 30%)" }
    else { "MISSED: $share% stale or expired (threshold 30%)" }
$settings = Get-Settings 'A-1' $Seed
$settings['weeks'] = $Weeks; $settings['per-week'] = $PerWeek; $settings['start'] = $Start; $settings['end'] = $endDate
$counts = "exceptions=$total stale=$($labels.stale) expired=$($labels.expired) live=$($labels.live) unparsed=$($labels.unparsed)`n" +
    ($checks -join "`n") + "`n" + ($log -join "`n")
$out = Write-Result -Settings $settings -Counts $counts -Verdict $verdict -Hashes (Get-AnswerHashes $run) `
    -Threats 'Simulated weeks: time passes only through --as-of, so code does not age between seals as it would; the task list is generic, not each project''s backlog; one model plays every maintainer.'
Write-Host "result: $out"
