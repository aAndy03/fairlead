# Agent protocol A-1 (agent-runbook; R-ROAD-019, R-ROAD-012, gate G2, 60-roadmap 4.2 row 6): an
# eight-week pilot, simulated. A subject agent plays a maintainer on three public repositories (one
# TypeScript/React, one Rust, one mixed) and makes scripted changes, declaring `ovr`s where a rule
# blocks it; each simulated week ends with `fairlead seal`. A reviewer agent then classifies every
# recorded exception as stale, expired or live. Pass: stale or expired at most 30%.
#
#   cargo build --release -p fairlead
#   powershell -ExecutionPolicy Bypass -File xtask\agent\A-1.ps1
#
# An interrupted run continues where it stopped: -Resume <its run folder> (printed at the start).
# Committed tasks and seals are kept; a task without its commit is redone from a clean tree.
#
# Defaults: the three repositories below and target\release\fairlead.exe. Other repositories go in
# -Repos, comma-separated; any spelling a shell produces (one string, an array, separate words) is
# accepted. Optional: -Weeks 8 -PerWeek 5 -Seed 1 -Start 2026-10-12.
#
# Cloning the three repositories downloads them; whoever starts the run approves that. Subject
# sessions run with --auto inside a copy outside this repository and see only that copy.
param(
    [string[]]$Repos = @(),
    [string]$Bin = '',
    [string]$Tasks = '',
    [int]$Weeks = 8,
    [int]$PerWeek = 5,
    [int]$Seed = 1,
    [string]$Start = '2026-10-12',
    # Print what would run, then stop.
    [switch]$Check,
    # Continue an interrupted run in this folder.
    [string]$Resume = '',
    # Words after the known parameters (a list a shell split apart) are more repositories.
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$More = @()
)
. "$PSScriptRoot\common.ps1"

# Windows PowerShell 5.1 leaves $PSScriptRoot empty in parameter defaults, so defaults are set here.
if (-not $Tasks) { $Tasks = Join-Path $PSScriptRoot 'a1-tasks.json' }
if (-not $Bin) { $Bin = Join-Path (Get-RepoRoot) 'target\release\fairlead.exe' }
$RepoList = @(@($Repos) + @($More) | ForEach-Object { "$_" -split '[,\s]+' } | Where-Object { $_ })
if ($RepoList.Count -eq 0) {
    # Suggested on 2026-10-11: a TypeScript/React, a Rust and a mixed (Rust, TypeScript, Python) project.
    $RepoList = @('https://github.com/pmndrs/zustand', 'https://github.com/sharkdp/hyperfine', 'https://github.com/ast-grep/ast-grep')
}
if (-not (Test-Path $Bin)) { throw "no fairlead binary at $Bin; run: cargo build --release -p fairlead" }
if ($Check) {
    Write-Host "repositories: $($RepoList -join ' | ')"
    Write-Host "fairlead: $Bin"
    Write-Host "weeks: $Weeks x $PerWeek tasks, seed $Seed, from $Start"
    return
}
if ($RepoList.Count -ne 3) { throw 'A-1 runs on exactly three repositories (TS/React, Rust, mixed)' }
$bin = (Resolve-Path $Bin).Path
$env:PATH = (Split-Path -Parent $bin) + [System.IO.Path]::PathSeparator + $env:PATH
if ($Resume) {
    $run = (Resolve-Path $Resume).Path
    Write-Host "resuming in $run"
} else {
    $run = New-RunDir 'A-1'
    Write-Host "run folder (for -Resume): $run"
}
$sealLog = Join-Path $run 'private\seals.log'
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
# (Not `FL` or `G`: `fl` is the built-in alias of Format-List, and an alias wins over a function.)
# A native command's stderr (a git warning, a fairlead refusal) is data here, never a reason to stop
# the run: it is captured apart from stdout with the error preference relaxed for the call.
function Invoke-Native([string]$Exe, [string[]]$A) {
    $eap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $all = @(& $Exe @A 2>&1)
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $eap
    }
    $isErr = { $_ -is [System.Management.Automation.ErrorRecord] }
    @{
        code = $code
        out  = ($all | Where-Object { -not (& $isErr) } | ForEach-Object { "$_" }) -join "`n"
        err  = ($all | Where-Object $isErr | ForEach-Object { "$_" }) -join "`n"
    }
}

function Invoke-Git([string]$Dir, [string[]]$A) {
    $all = @('-C', $Dir, '-c', 'user.name=pilot', '-c', 'user.email=pilot@invalid', '-c', 'commit.gpgsign=false', '-c', 'core.autocrlf=false') + $A
    (Invoke-Native 'git' $all).out
}

function Invoke-Fairlead([string]$Dir, [string[]]$A) {
    Push-Location $Dir
    try { Invoke-Native $bin $A } finally { Pop-Location }
}

$repoDirs = @()
foreach ($src in (Get-Shuffled $RepoList $Seed)) {
    $rid = Get-OpaqueId $Seed "repo|$src"
    $map += "$rid,repo,$src,"
    $dir = Join-Path $run "work\$rid"
    if (Test-Path (Join-Path $dir '.git')) {
        if ((Invoke-Git $dir @('log', '--format=%s')) -match 'pilot: fairlead init') {
            # Resume: drop the half-done edits of an interrupted task; its commit never happened.
            Invoke-Git $dir @('reset', '-q', '--hard') | Out-Null
            Invoke-Git $dir @('clean', '-q', '-fd') | Out-Null
            Write-Host "kept $src"
            $repoDirs += , @($rid, $dir)
            continue
        }
        Remove-Item -Recurse -Force $dir
    }
    Write-Host "downloading $src ..."
    $clone = Invoke-Native 'git' @('clone', '-q', $src, $dir)
    if ($clone.code -ne 0) { throw "clone failed: $src`n$($clone.err)" }
    Invoke-Fairlead $dir @('init', '--yes', '--as-of', $Start) | Out-Null
    $ctl = Join-Path $dir 'fairlead\control.toml'
    Write-Utf8 $ctl ((Get-Content -Raw -LiteralPath $ctl) + "`n" + $rules)
    Invoke-Fairlead $dir @('sync', '--as-of', $Start) | Out-Null
    # The repository's existing violations are history, not the pilot's exceptions.
    Invoke-Fairlead $dir @('sync', '--baseline', '--accept-new', '--why', 'pilot start', '--as-of', $Start) | Out-Null
    Invoke-Git $dir @('add', '-A') | Out-Null
    Invoke-Git $dir @('commit', '-q', '-m', 'pilot: fairlead init, rules, baseline') | Out-Null
    $repoDirs += , @($rid, $dir)
}

# The subject phase: per repository a seeded task order, $PerWeek tasks a week, a seal each week.
foreach ($rd in $repoDirs) {
    $rid = $rd[0]; $dir = $rd[1]
    $order = Get-Shuffled $taskList ($Seed + [Convert]::ToInt32($rid.Substring(0, 6), 16))
    $done = @((Invoke-Git $dir @('log', '--format=%s')) -split "`n")
    for ($w = 0; $w -lt $Weeks; $w++) {
        $date = $startDate.AddDays(7 * $w + 4).ToString('yyyy-MM-dd')
        for ($t = 0; $t -lt $PerWeek; $t++) {
            $task = $order[$w * $PerWeek + $t]
            $sid = Get-OpaqueId $Seed "subject|$rid|$w|$t"
            $map += "$sid,subject,$rid,week $($w + 1) $($task.id)"
            if ($done -contains "week $($w + 1): $($task.id)") { continue }
            $p = Join-Path $run "private\prompts\$sid.md"
            Write-Utf8 $p ($subjectBase.Replace('{{DATE}}', $date).Replace('{{TASK}}', $task.text))
            # A maintainer's task can take a while; only a hang is cut short.
            Invoke-Sample -PromptFile $p -WorkDir $dir -OutFile (Join-Path $run "answers\$sid.json") -Auto -TimeoutMinutes 30 | Out-Null
            Invoke-Git $dir @('add', '-A') | Out-Null
            Invoke-Git $dir @('commit', '-q', '--allow-empty', '-m', "week $($w + 1): $($task.id)") | Out-Null
        }
        # Resume: a week already sealed, or followed by later work, is not sealed again.
        $later = @($done | Where-Object { $_ -match '^week (\d+): ' -and [int]$Matches[1] -gt ($w + 1) })
        if (($done -contains "week $($w + 1): seal") -or $later.Count -gt 0) { continue }
        $seal = Invoke-Fairlead $dir @('seal', '--yes', '--anywhere', '--as-of', $date)
        $why = if ($seal.code -eq 0) { '' } else { ': ' + (($seal.err -split "`n")[0]).Trim() }
        Add-Content -Encoding utf8 -LiteralPath $sealLog "- $rid week $($w + 1) ($date): seal exit $($seal.code)$why"
        if ($seal.code -eq 0) {
            Invoke-Git $dir @('add', '-A') | Out-Null
            Invoke-Git $dir @('commit', '-q', '-m', "week $($w + 1): seal") | Out-Null
        }
        Write-Host "$rid week $($w + 1): seal exit $($seal.code)$why"
    }
}

# The seal record: the log written as the run went, plus the git history for weeks sealed before a
# resume (their refusal reasons were not recorded then).
$logged = @(if (Test-Path $sealLog) { Get-Content -LiteralPath $sealLog })
foreach ($rd in $repoDirs) {
    $subjects = @((Invoke-Git $rd[1] @('log', '--format=%s')) -split "`n")
    for ($w = 1; $w -le $Weeks; $w++) {
        $line = $logged | Where-Object { $_ -like "- $($rd[0]) week $w *" } | Select-Object -Last 1
        if ($line) { $log += $line }
        elseif ($subjects -contains "week ${w}: seal") { $log += "- $($rd[0]) week ${w}: seal exit 0" }
        else { $log += "- $($rd[0]) week ${w}: not sealed (reason not recorded before a resume)" }
    }
}

# The reviewer phase: every exception still recorded at the end, judged one per fresh session.
$labels = @{ stale = 0; expired = 0; live = 0; unparsed = 0 }
$checks = @()
foreach ($rd in $repoDirs) {
    $rid = $rd[0]; $dir = $rd[1]
    $json = Invoke-Fairlead $dir @('check', '--format', 'json', '--as-of', $endDate)
    Write-Utf8 (Join-Path $run "private\$rid-check.json") $json.out
    $terse = (Invoke-Fairlead $dir @('check', '--format', 'terse', '--max', '0', '--as-of', $endDate)).out
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
