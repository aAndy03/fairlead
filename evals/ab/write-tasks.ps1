# Writes the task lists of evals/ab (PROTOCOL.md, "Tasks" and the pre-run revision): a separate
# agent session per project sees only the project's README (maintenance tasks) or SPEC (build
# steps) and writes nine tasks. Runs once; the lists are committed before any measured session.
#
#   powershell -File evals\ab\write-tasks.ps1 [-Projects shelf,ledger,relay] [-Force]
param(
    [string[]]$Projects = @('shelf', 'ledger', 'relay'),
    [switch]$Force
)
. "$PSScriptRoot\..\..\xtask\agent\common.ps1"

$Projects = @($Projects | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$root = Join-Path $PSScriptRoot 'projects'
$run = New-RunDir 'ab-tasks'

function Get-Toml([string]$Raw) {
    $text = Get-AnswerText $Raw
    $m = [regex]::Match($text, '```toml\s*(.*?)```', 'Singleline')
    if ($m.Success) { $m.Groups[1].Value.Trim() + "`n" } else { $text.Trim() + "`n" }
}

foreach ($p in $Projects) {
    foreach ($kind in @(
            @{ src = 'repo\README.md'; as = 'README.md'; prompt = 'write-tasks.md'; out = 'tasks.toml' },
            @{ src = 'spec.md'; as = 'SPEC.md'; prompt = 'write-increments.md'; out = 'increments.toml' })) {
        $out = Join-Path $root "$p\$($kind.out)"
        if ((Test-Path $out) -and -not $Force) { Write-Host "$out exists; -Force to rewrite"; continue }
        $work = Join-Path $run "work\$p-$($kind.out)"
        New-Item -ItemType Directory -Force $work | Out-Null
        $file = Join-Path $work $kind.as
        Copy-Item (Join-Path $root "$p\$($kind.src)") $file
        $raw = Invoke-Sample -PromptFile (Join-Path $PSScriptRoot "prompts\$($kind.prompt)") -WorkDir $work `
            -Files @($file) -OutFile (Join-Path $run "answers\$p-$($kind.out).json") -TimeoutMinutes 8
        $settings = Get-Settings 'ab-tasks' 0
        $head = "# Written by an agent session that saw only $($kind.as) (evals/ab/PROTOCOL.md).`n" +
            "# model=$($settings.model) opencode=$($settings.opencode) date=$($settings.date) commit=$($settings.commit)`n" +
            "# Do not edit after the first measured run.`n`n"
        Write-Utf8 $out ($head + (Get-Toml $raw))
        Write-Host "wrote $out"
    }
}
