# Agent protocol A-3 (agent-runbook; R-ROAD-020): a reviewer agent reads the 0.1 measurements and
# the A-1 and A-2 results and drafts the 0.2 and 0.3 threshold amendment. The owner reads it; the
# pass rule is an amendment committed, or "no change" recorded.
#
#   powershell -File xtask\agent\A-3.ps1 -A1 <result.md> -A2 <result.md> [-Seed 1]
param(
    [Parameter(Mandatory)][string]$A1,
    [Parameter(Mandatory)][string]$A2,
    [int]$Seed = 1
)
. "$PSScriptRoot\common.ps1"

$root = Get-RepoRoot
$run = New-RunDir 'A-3'
$id = Get-OpaqueId $Seed 'thresholds'
$files = @(
    (Join-Path $root 'docs\60-roadmap.md'),
    (Join-Path $root 'bench\baseline.toml'),
    (Resolve-Path $A1).Path,
    (Resolve-Path $A2).Path
)
$answer = Invoke-Sample -PromptFile (Join-Path $PSScriptRoot 'prompts\A-3-thresholds.md') `
    -WorkDir (Join-Path $run "work\$id") -Files $files -OutFile (Join-Path $run "answers\$id.json")
$label = Get-Label $answer @('change', 'no-change')
$settings = Get-Settings 'A-3' $Seed
$draft = Join-Path $root ("docs\plans\0.2\results\{0}-A-3-draft.md" -f $settings.date)
$m = [regex]::Match($answer, '(?s)BEGIN-DRAFT(.*?)END-DRAFT')
$body = if ($m.Success) { $m.Groups[1].Value -replace '\\n', "`n" -replace '\\"', '"' } else { '(no draft block found; read the raw answer)' }
Write-Utf8 $draft ("# Threshold amendment draft ($($settings.date), agent-run, for the owner)`n`n" + $body.Trim() + "`n")
$out = Write-Result -Settings $settings -Counts "proposal=$label; draft=$draft" `
    -Verdict 'PENDING: the owner reads the draft, then commits the amendment to 60-roadmap or records "no change".' `
    -Hashes (Get-AnswerHashes $run) -Threats 'The proposal rests on one model reading two agent-run results.'
Write-Host "draft: $draft"
Write-Host "result: $out"
