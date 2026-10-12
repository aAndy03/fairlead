# Agent protocol A-5 (agent-runbook; 0.1 practice E-34): before the first bless is accepted, a
# reviewer agent compares each golden's expected outputs with the chapter rules they claim to show.
# Pass: every difference resolved or logged in the errata.
#
#   powershell -File xtask\agent\A-5.ps1 [-Goldens rebase-after-seal,shallow-clone,...] [-Seed 1]
param(
    [string[]]$Goldens = @('rebase-after-seal', 'shallow-clone', 'squash-merge', 'cherry-pick-seal', 'revert-seal', 'merge-conflict-gen'),
    [string[]]$Chapters = @('docs\23-git-integration.md', 'docs\20-versions-and-seal.md'),
    [int]$Seed = 1,
    [string]$Milestone = '0.3'
)
. "$PSScriptRoot\common.ps1"

# `powershell -File` passes `a,b` as one string; split the lists here.
$Goldens = @($Goldens | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Chapters = @($Chapters | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$root = Get-RepoRoot
$run = New-RunDir 'A-5'
$prompt = Join-Path $PSScriptRoot 'prompts\A-5-golden.md'
$chapterFiles = @($Chapters | ForEach-Object { Join-Path $root $_ })
$map = @('id,golden')
$all = @()
foreach ($g in (Get-Shuffled $Goldens $Seed)) {
    $dir = Join-Path $root "tests\golden\$g"
    $id = Get-OpaqueId $Seed $g
    $map += "$id,$g"
    $ctx = @('# Golden repository under review', '', '## golden.toml (the scripted history and the expected exits)', '', '```toml',
        (Get-Content -Raw -LiteralPath (Join-Path $dir 'golden.toml')), '```', '')
    foreach ($f in (Get-ChildItem -File (Join-Path $dir 'expected') | Sort-Object Name)) {
        $ctx += @("## expected/$($f.Name)", '', '```', (Get-Content -Raw -LiteralPath $f.FullName), '```', '')
    }
    $ctxFile = Join-Path $run "work\$id\golden.md"
    Write-Utf8 $ctxFile ($ctx -join "`n")
    $answer = Invoke-Sample -PromptFile $prompt -WorkDir (Join-Path $run "work\$id") `
        -Files (@($ctxFile) + $chapterFiles) -OutFile (Join-Path $run "answers\$id.json")
    $items = Get-Items $answer
    $all += @($items | ForEach-Object { "- $g`: $_" })
    Write-Host ("{0} {1} item(s), label {2}" -f $g, $items.Count, (Get-Label $answer @('clean', 'issues')))
}
Write-Utf8 (Join-Path $run 'private\map.csv') (($map -join "`n") + "`n")
$settings = Get-Settings 'A-5' $Seed
$verdict = if ($all.Count -eq 0) { 'PASS: no difference reported' } else { "OPEN: $($all.Count) difference(s); resolve each in the code or log it in the errata (E-34 practice)" }
$out = Write-Result -Settings $settings -Counts ("goldens=$($Goldens.Count) items=$($all.Count)`n" + ($all -join "`n")) `
    -Verdict $verdict -Hashes (Get-AnswerHashes $run) -Milestone $Milestone `
    -Threats 'The reviewer reads two chapters, not every rule an output touches; a golden name hints at its scenario, which the review needs.'
Write-Host "result: $out"
