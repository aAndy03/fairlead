# Agent protocol B-3 (agent-runbook; 60-roadmap 4.3 row 4): variation-cycle realism. A subject
# agent plays the web team of the `hier-ui` golden: its home page needs a variant the library lacks,
# and it may learn the rules only from fairlead's help, `context` and `explain`. The final tree is
# scored mechanically: a request filed, an interim wrapper with `flag awaiting`, no copy-and-edit
# fork, no edit outside `web/`, and `check` without errors. Pass: every sample meets all five.
#
#   cargo build --release -p fairlead
#   powershell -File xtask\agent\B-3.ps1 [-Samples 3] [-Seed 1] [-Bin target\release\fairlead.exe]
#
# The fixture is the golden's repository with its two deliberate errors removed (the upward
# `use cmp.card` in the button and the `admin:cmp.audit-table` visibility error) and
# `[variation] fork-detect = "warn"`, committed in a fresh git repository outside this one.
param(
    [int]$Samples = 3,
    [int]$Seed = 1,
    [string]$Bin = '',
    [string]$Milestone = '0.3'
)
. "$PSScriptRoot\common.ps1"

$root = Get-RepoRoot
if (-not $Bin) { $Bin = Join-Path $root 'target\release\fairlead.exe' }
if (-not (Test-Path $Bin)) { throw "no fairlead binary at $Bin; run: cargo build --release -p fairlead" }
$bin = (Resolve-Path $Bin).Path
$env:PATH = (Split-Path -Parent $bin) + [System.IO.Path]::PathSeparator + $env:PATH
$AsOf = '2026-10-12'
$run = New-RunDir 'B-3'
$prompt = Join-Path $PSScriptRoot 'prompts\B-3-subject.md'

function Invoke-Quiet([string]$Dir, [string]$Exe, [string[]]$Arguments) {
    $eap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    Push-Location $Dir
    try { $out = (& $Exe @Arguments 2>&1 | Out-String) } finally { Pop-Location; $ErrorActionPreference = $eap }
    $out
}

function New-Fixture([string]$Dir) {
    Copy-Item -Recurse (Join-Path $root 'tests\golden\hier-ui\repo') $Dir
    $button = Join-Path $Dir 'src\atoms\button.tsx'
    Write-Utf8 $button (((Get-Content -Raw $button) -split "`n" | Where-Object { $_ -notmatch 'use cmp\.card' }) -join "`n")
    $home_ = Join-Path $Dir 'web\src\home.tsx'
    Write-Utf8 $home_ (((Get-Content -Raw $home_) -split "`n" | Where-Object { $_ -notmatch 'use admin:cmp\.audit-table' }) -join "`n")
    $ctl = Join-Path $Dir 'fairlead\control.toml'
    Write-Utf8 $ctl ((Get-Content -Raw $ctl) + "`n[variation]`nfork-detect = `"warn`"`n")
    Invoke-Quiet $Dir 'git' @('init', '-q') | Out-Null
    Invoke-Quiet $Dir 'git' @('-c', 'core.autocrlf=false', 'add', '-A') | Out-Null
    Invoke-Quiet $Dir 'git' @('-c', 'user.name=fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture') | Out-Null
    $c = Invoke-Quiet $Dir $bin @('check', '--as-of', $AsOf, '--format', 'terse')
    if ($c -notmatch 'errors 0 ') { throw "the B-3 fixture is not clean:`n$c" }
}

function Get-Score([string]$Dir) {
    $status = Invoke-Quiet $Dir 'git' @('status', '--porcelain', '--untracked-files=all')
    $paths = @($status -split "`n" | Where-Object { $_.Length -gt 3 } | ForEach-Object { $_.Substring(3).Trim().Trim('"') })
    $outside = @($paths | Where-Object { $_ -notmatch '^web/' -and $_ -notmatch '^\.fairlead/' })
    $reqDir = Join-Path $Dir 'web\fairlead\docs\requests'
    $request = (Test-Path $reqDir) -and (@(Get-ChildItem -File $reqDir -Filter '*.md').Count -gt 0)
    $wrapper = @(Get-ChildItem -Recurse -File (Join-Path $Dir 'web\src') | Where-Object {
            $t = Get-Content -Raw $_.FullName
            $t -match 'wraps=\S*cmp\.button' -and $t -match 'flag awaiting to='
        }).Count -gt 0
    $check = Invoke-Quiet $Dir $bin @('check', '--as-of', $AsOf, '--format', 'terse', '--show-observe')
    $fork = $check -match 'HN-W003'
    $clean = $check -match 'errors 0 '
    [ordered]@{
        request = $request; wrapper = $wrapper; 'no-fork' = (-not $fork)
        'only-web' = ($outside.Count -eq 0); 'check-clean' = $clean
        outside = ($outside -join ' ')
    }
}

$map = @('id,sample')
$lines = @()
$passed = 0
for ($i = 1; $i -le $Samples; $i++) {
    $id = Get-OpaqueId $Seed "b3-$i"
    $map += "$id,$i"
    $work = Join-Path $run "work\$id"
    New-Fixture $work
    Invoke-Sample -PromptFile $prompt -WorkDir $work -OutFile (Join-Path $run "answers\$id.json") -Auto -TimeoutMinutes 15 | Out-Null
    $s = Get-Score $work
    $ok = $s.request -and $s.wrapper -and $s['no-fork'] -and $s['only-web'] -and $s['check-clean']
    if ($ok) { $passed++ }
    $lines += ('- {0}: request={1} wrapper={2} no-fork={3} only-web={4} check-clean={5}{6}' -f $id, $s.request, $s.wrapper, $s['no-fork'], $s['only-web'], $s['check-clean'], $(if ($s.outside) { " (outside web/: $($s.outside))" } else { '' }))
    Write-Host $lines[-1]
}
Write-Utf8 (Join-Path $run 'private\map.csv') (($map -join "`n") + "`n")
$settings = Get-Settings 'B-3' $Seed
$verdict = if ($passed -eq $Samples) { "PASS: $passed of $Samples samples followed the protocol" } else { "MISSED: $passed of $Samples samples followed the protocol" }
$out = Write-Result -Settings $settings -Counts ("samples=$Samples passed=$passed`n" + ($lines -join "`n")) -Verdict $verdict `
    -Hashes (Get-AnswerHashes $run) -Milestone $Milestone `
    -Threats 'The subject sees one small fixture; a request and a wrapper are scored by their presence, not by the quality of their text. Final trees stay in the run folder for reading.'
Write-Host "run folder: $run"
Write-Host "result: $out"
