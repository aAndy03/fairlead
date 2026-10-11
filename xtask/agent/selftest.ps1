# Offline self-test of the agent runners (0.2 P7-T9): parses every script and checks the shared
# helpers. Runs no model.  powershell -File xtask\agent\selftest.ps1
. "$PSScriptRoot\common.ps1"
$fail = 0
function Expect([bool]$Ok, [string]$What) {
    if ($Ok) { Write-Host "ok   $What" } else { Write-Host "FAIL $What"; $script:fail++ }
}

foreach ($f in Get-ChildItem -File (Join-Path $PSScriptRoot '*.ps1')) {
    $errors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($f.FullName, [ref]$null, [ref]$errors) | Out-Null
    Expect ($errors.Count -eq 0) "parses: $($f.Name) $($errors | ForEach-Object { $_.Message })"
}

$a = Get-Shuffled (1..20) 7
$b = Get-Shuffled (1..20) 7
$c = Get-Shuffled (1..20) 8
Expect (($a -join ',') -eq ($b -join ',')) 'shuffle is seeded'
Expect (($a -join ',') -ne ($c -join ',')) 'another seed, another order'
Expect ((($a | Sort-Object) -join ',') -eq ((1..20) -join ',')) 'shuffle is a permutation'
Expect ((Get-OpaqueId 1 'x') -match '^[0-9a-f]{10}$') 'opaque id is 10 hex'
Expect ((Get-OpaqueId 1 'x') -ne (Get-OpaqueId 2 'x')) 'opaque id depends on the seed'
Expect ((Get-Label "reason`nLABEL: Wrong" @('right', 'wrong')) -eq 'wrong') 'label is read, case-folded'
Expect ((Get-Label '{"text":"x LABEL: live\n"}' @('stale', 'expired', 'live')) -eq 'live') 'label inside JSON text'
Expect ((Get-Label 'LABEL: maybe' @('right', 'wrong')) -eq 'unparsed') 'an unknown label is unparsed'
Expect ((Get-Items "ITEM: a b`nx`nITEM: c").Count -eq 2) 'items are read'
$tasks = @(Get-Content -Raw (Join-Path $PSScriptRoot 'a1-tasks.json') | ConvertFrom-Json | ForEach-Object { $_ })
Expect ($tasks.Count -ge 40) "a1 tasks: $($tasks.Count) (8 weeks x 5)"
foreach ($p in 'A-1-subject', 'A-1-review', 'A-2-label', 'A-3-thresholds', 'A-4-changelog', 'A-5-golden') {
    Expect (Test-Path (Join-Path $PSScriptRoot "prompts\$p.md")) "prompt $p"
}
if ($fail) { Write-Host "$fail failure(s)"; exit 1 }
Write-Host 'selftest: ok'
