# Agent protocol B-4 (agent-runbook; D-53): before the v1 API freeze, a reviewer agent reads
# docs/v1-api.md against chapter 15 and the snapshots in tests/v1_api/ and lists every
# inconsistency, unclear flag or likely-regretted name. Pass: every item fixed or moved to the 1.0
# gap list (docs/plans/1.0-gaps.md); the runner reports the count, the resolution is recorded by hand.
#
#   powershell -File xtask\agent\B-4.ps1 [-Seed 1]
param(
    [int]$Seed = 1,
    [string]$Milestone = '0.3'
)
. "$PSScriptRoot\common.ps1"

$root = Get-RepoRoot
$run = New-RunDir 'B-4'
$id = Get-OpaqueId $Seed 'v1-api'
$work = Join-Path $run "work\$id"
New-Item -ItemType Directory -Force $work | Out-Null
Copy-Item (Join-Path $root 'docs\v1-api.md') (Join-Path $work 'v1-api.md')
Copy-Item (Join-Path $root 'docs\15-cli.md') (Join-Path $work '15-cli.md')
$ctx = @('# Snapshots of the surface (tests/v1_api/)', '')
foreach ($f in (Get-ChildItem -File (Join-Path $root 'tests\v1_api') | Sort-Object Name)) {
    $ctx += @("## $($f.Name)", '', '```', (Get-Content -Raw -LiteralPath $f.FullName), '```', '')
}
Write-Utf8 (Join-Path $work 'surface.md') ($ctx -join "`n")
$answer = Invoke-Sample -PromptFile (Join-Path $PSScriptRoot 'prompts\B-4-api.md') -WorkDir $work `
    -Files @((Join-Path $work 'v1-api.md'), (Join-Path $work '15-cli.md'), (Join-Path $work 'surface.md')) `
    -OutFile (Join-Path $run "answers\$id.json") -TimeoutMinutes 10
$items = Get-Items $answer
$label = Get-Label $answer @('clean', 'issues')
$settings = Get-Settings 'B-4' $Seed
$verdict = if ($items.Count -eq 0) { 'PASS: no item' } else { "OPEN: $($items.Count) item(s), label $label; fix each or move it to the 1.0 gap list" }
$out = Write-Result -Settings $settings -Counts ("items=$($items.Count)`n" + (($items | ForEach-Object { "- $_" }) -join "`n")) `
    -Verdict $verdict -Hashes (Get-AnswerHashes $run) -Milestone $Milestone `
    -Threats 'One reviewer session; it reads the snapshots, not the code, so a behaviour the help text hides is not reviewed.'
Write-Host "result: $out"
