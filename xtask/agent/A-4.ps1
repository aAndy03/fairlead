# Agent protocol A-4 (agent-runbook; R-REL-025): before tagging, a reviewer agent checks the
# generated CHANGELOG.md against the sealed change docs. Pass: zero unresolved items.
#
#   powershell -File xtask\agent\A-4.ps1 [-Changelog CHANGELOG.md] [-Seed 1]
#
# Run it after the release seal (`fairlead seal --yes --commit` on main), before the owner tags.
param(
    [string]$Changelog = 'CHANGELOG.md',
    [int]$Seed = 1,
    [string]$Milestone = '0.3'
)
. "$PSScriptRoot\common.ps1"

$root = Get-RepoRoot
$log = Join-Path $root $Changelog
if (-not (Test-Path $log)) { throw "no ${Changelog}: seal the release first" }
$run = New-RunDir 'A-4'
$id = Get-OpaqueId $Seed 'changelog'
$sealed = Get-ChildItem -Recurse -File -Path $root -Filter '*.md' |
    Where-Object { $_.FullName -match '\\fairlead\\docs\\changes\\' -and $_.FullName -notmatch '\\(target|tests|fixtures)\\' } |
    Where-Object { (Get-Content -Raw -LiteralPath $_.FullName) -match 'status = "sealed"' } |
    Sort-Object FullName
$ctx = @('# Sealed change docs', '')
foreach ($d in $sealed) {
    $rel = $d.FullName.Substring($root.Length + 1) -replace '\\', '/'
    $ctx += @("## $rel", '', '```', (Get-Content -Raw -LiteralPath $d.FullName), '```', '')
}
$ctxFile = Join-Path $run "work\$id\sealed-docs.md"
Write-Utf8 $ctxFile ($ctx -join "`n")
$copy = Join-Path $run "work\$id\CHANGELOG.md"
Copy-Item -LiteralPath $log $copy
$answer = Invoke-Sample -PromptFile (Join-Path $PSScriptRoot 'prompts\A-4-changelog.md') `
    -WorkDir (Join-Path $run "work\$id") -Files @($copy, $ctxFile) -OutFile (Join-Path $run "answers\$id.json")
$items = Get-Items $answer
$label = Get-Label $answer @('clean', 'issues')
$settings = Get-Settings 'A-4' $Seed
$verdict = if ($label -eq 'clean' -and $items.Count -eq 0) { 'PASS: zero items' } else { "OPEN: $($items.Count) item(s), label $label; resolve each or record why it stands, then re-run" }
$out = Write-Result -Settings $settings -Counts ("sealed docs=$($sealed.Count) items=$($items.Count)`n" + (($items | ForEach-Object { "- $_" }) -join "`n")) `
    -Verdict $verdict -Hashes (Get-AnswerHashes $run) -Milestone $Milestone `
    -Threats 'One reviewer session; the checklist cannot judge whether a sealed doc should exist at all.'
Write-Host "result: $out"
