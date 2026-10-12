# Scores a finished run again with the current scorer and hidden tests, from the commits each chain
# left ("start", then "after <task>"): session k is scored with HEAD at commit k-1 and the working
# tree at commit k, exactly as it was scored live. Reviewer verdicts (M1 without hidden tests) are
# kept from the original records. Writes sessions-rescored.jsonl beside sessions.jsonl.
#
#   powershell -File evals\ab\rescore.ps1 -Name 2026-10-12-pilot [-Results <dir>]
param(
    [Parameter(Mandatory)][string]$Name,
    [string]$Results = (Join-Path ([System.IO.Path]::GetTempPath()) 'ab-eval'),
    # A scorer built apart from the one a running evaluation holds open:
    #   cargo build --release --offline --manifest-path evals\ab\harness\Cargo.toml --target-dir evals\ab\harness\target-next
    [string]$Ab = ''
)
. "$PSScriptRoot\..\..\xtask\agent\common.ps1"
$ErrorActionPreference = 'Continue'

$ab = if ($Ab) { $Ab } else { Join-Path $PSScriptRoot 'harness\target\release\ab.exe' }
$runDir = Join-Path $Results $Name
$gitDir = Split-Path -Parent (Split-Path -Parent (Get-Command git).Source)
$env:AB_BASH = Join-Path $gitDir 'bin\bash.exe'
$records = @(Get-Content (Join-Path $runDir 'sessions.jsonl') | Where-Object { $_ } | ForEach-Object { $_ | ConvertFrom-Json })
$out = Join-Path $runDir 'sessions-rescored.jsonl'
if (Test-Path $out) { Remove-Item $out }
$passed = @{}

foreach ($r in ($records | Sort-Object chain, pos)) {
    $work = Join-Path $runDir "work\$($r.chain)"
    $log = @(& git -C $work log --format='%H %s' --reverse 2>$null)
    $commits = @($log | ForEach-Object { ($_ -split ' ', 2)[0] })
    if ($commits.Count -le $r.pos) { Write-Host "skip $($r.id): chain has $($commits.Count) commits"; continue }
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("ab-rescore-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
    & git -c core.autocrlf=false clone -q --config core.autocrlf=false $work $tmp 2>$null | Out-Null
    & git -C $tmp -c core.autocrlf=false checkout -q $commits[$r.pos] 2>$null | Out-Null
    & git -C $tmp reset -q --soft $commits[$r.pos - 1] 2>$null | Out-Null
    $project = Join-Path $PSScriptRoot "projects\$($r.project)"
    $earlier = @($passed[$r.chain] | Where-Object { $_ })
    if ($r.track -eq 'maint') { $earlier = @('_base') + $earlier }
    $events = Join-Path $runDir "live\$($r.id).json"
    $args_ = @('score', '--project', $project, '--dir', $tmp, '--team', $r.team, '--task', $r.task, '--events', $events,
        '--pristine', $commits[0])
    if ($earlier.Count) { $args_ += @('--earlier', ($earlier -join ',')) }
    if ($r.tier -eq 'setup') { $args_ += @('--setup', 'true') }
    $score = (& $ab @args_ 2>$null | Out-String) | ConvertFrom-Json
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    if ($null -eq $score.m1_task_done) {
        $score.m1_task_done = $r.score.m1_task_done
        $score | Add-Member -NotePropertyName m1_label -NotePropertyValue $r.score.m1_label -Force
    }
    if ($r.m1_judge -eq 'hidden-tests' -and $score.m1_task_done -eq $true) {
        if (-not $passed[$r.chain]) { $passed[$r.chain] = @() }
        $passed[$r.chain] += $r.task
    }
    $r.score = $score
    Add-Content -Path $out -Value ($r | ConvertTo-Json -Depth 8 -Compress) -Encoding UTF8
    Write-Host ("{0}: M1={1} M2={2} M3 new={3}" -f $r.id, $score.m1_task_done, $score.m2_nothing_broken, $score.m3_new)
}
Write-Host "wrote $out"
