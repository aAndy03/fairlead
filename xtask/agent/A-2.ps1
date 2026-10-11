# Agent protocol A-2 (agent-runbook; 60-roadmap 4.2 row 3): a reviewer agent labels each file that
# `cargo xtask coverage-replay` flagged. Pass: at most 5% wrong flags after one tuning of exempt globs.
#
#   cargo xtask coverage-replay --repo <path> --commits 100 --out <csv>
#   powershell -File xtask\agent\A-2.ps1 -Csv <csv> -Repo <path> [-Seed 1] [-Limit 0]
#
# The reviewer sees the commit's diff of the file, the change docs present at that commit and the
# [docs-first] table then in force; never the coverage code or which replay produced the row.
param(
    [Parameter(Mandatory)][string]$Csv,
    [Parameter(Mandatory)][string]$Repo,
    [int]$Seed = 1,
    [int]$Limit = 0
)
. "$PSScriptRoot\common.ps1"

$run = New-RunDir 'A-2'
$prompt = Join-Path $PSScriptRoot 'prompts\A-2-label.md'
$rows = @(Import-Csv -LiteralPath $Csv)
$order = Get-Shuffled (0..($rows.Count - 1)) $Seed
if ($Limit -gt 0) { $order = @($order | Select-Object -First $Limit) }
$map = @('id,row,commit,path')

function Get-Clipped([string]$Text, [int]$Max) {
    $l = $Text -split "`n"
    if ($l.Count -le $Max) { return $Text }
    (($l | Select-Object -First $Max) -join "`n") + "`n[... $($l.Count - $Max) more lines cut ...]"
}

foreach ($i in $order) {
    $r = $rows[$i]
    $id = Get-OpaqueId $Seed "$($r.commit)|$($r.path)"
    $map += '{0},{1},{2},{3}' -f $id, $i, $r.commit, $r.path
    $diff = & git -C $Repo show --format= $r.commit -- $r.path | Out-String
    $tree = @(& git -C $Repo ls-tree -r --name-only $r.commit)
    $docs = $tree | Where-Object { $_ -match '/docs/changes/[^/]+\.md$' }
    $ctx = @("# Flagged file", '', "path: $($r.path)", '', '## Its change in this commit', '', '```diff', (Get-Clipped $diff 300), '```', '')
    $ctx += @('## Change docs present at this commit', '')
    if (-not $docs) { $ctx += 'none' }
    foreach ($d in $docs) {
        $text = & git -C $Repo show "$($r.commit):$d" | Out-String
        $ctx += @("### $d", '', '```', (Get-Clipped $text 60), '```', '')
    }
    $controls = $tree | Where-Object { $_ -match '(^|/)fairlead/control\.toml$' } | Select-Object -First 1
    if ($controls) {
        $ctl = & git -C $Repo show "$($r.commit):$controls" | Out-String
        $m = [regex]::Match($ctl, '(?ms)^\[docs-first\].*?(?=^\[|\z)')
        $ctx += @('## [docs-first] in force', '', '```toml', $(if ($m.Success) { $m.Value.Trim() } else { '(none)' }), '```')
    }
    $ctxFile = Join-Path $run "work\$id\context.md"
    Write-Utf8 $ctxFile ($ctx -join "`n")
    $answer = Invoke-Sample -PromptFile $prompt -WorkDir (Join-Path $run "work\$id") -Files @($ctxFile) `
        -OutFile (Join-Path $run "answers\$id.json")
    $r.label = Get-Label $answer @('right', 'wrong')
    Write-Host ("{0} {1}" -f $id, $r.label)
}
Write-Utf8 (Join-Path $run 'private\map.csv') (($map -join "`n") + "`n")

$labelled = Join-Path $run 'labelled.csv'
$rows | Export-Csv -NoTypeInformation -Encoding UTF8 -LiteralPath $labelled
$score = & cargo run -q -p xtask --locked -- coverage-score $labelled | Out-String
# Kept at once: Get-Settings below runs git and opencode, which reset $LASTEXITCODE (a run on
# 2026-10-11 printed PASS for an 86.7% share that way).
$scoreCode = $LASTEXITCODE
$right = @($rows | Where-Object { $_.label -eq 'right' }).Count
$wrong = @($rows | Where-Object { $_.label -eq 'wrong' }).Count
$unparsed = @($rows | Where-Object { $_.label -eq 'unparsed' }).Count
$settings = Get-Settings 'A-2' $Seed
$verdict = if ($scoreCode -eq 0) { "PASS: $($score.Trim())" } else { "MISSED: $($score.Trim())" }
$out = Write-Result -Settings $settings `
    -Counts "rows=$($rows.Count) labelled=$($order.Count) right=$right wrong=$wrong unparsed=$unparsed; csv=$Csv" `
    -Verdict $verdict -Hashes (Get-AnswerHashes $run) `
    -Threats 'Labels come from one reviewer model; unparsed answers count as unlabelled. The exempt globs are tuned at most once, from the wrong labels, and the tuning is recorded.'
Write-Host "labelled CSV: $labelled"
Write-Host "result: $out"
