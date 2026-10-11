# Shared runner for the agent protocols (docs/plans/agent-runbook.md, "Runner"; 0.2 P7-T9).
# Windows PowerShell 5.1 compatible. Dot-source it: . "$PSScriptRoot\common.ps1"
#
# Rules it carries out: one fresh `opencode run` per sample (no -c, no -s), in an empty working
# directory outside this repository; a seeded run order; answers stored under opaque IDs with the
# private map kept apart; the settings recorded; results written as "agent-run" evidence.

$ErrorActionPreference = 'Stop'

# Runbook rule 4: set by the owner on 2026-10-10. A different model needs a new dated runbook entry.
$Model = 'opencode-go/muse-spark-1.3-contributor'
$Utf8 = New-Object System.Text.UTF8Encoding $false

function Get-RepoRoot {
    (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
}

function Write-Utf8([string]$Path, [string]$Text) {
    $dir = Split-Path -Parent $Path
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }
    [System.IO.File]::WriteAllText($Path, ($Text -replace "`r`n", "`n"), $Utf8)
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

# A run folder outside the repository: answers\ (opaque IDs), private\ (map.csv, never given to a
# reviewer), work\ (one empty folder per session).
function New-RunDir([string]$Protocol) {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $dir = Join-Path ([System.IO.Path]::GetTempPath()) "fairlead-agent\$Protocol-$stamp"
    foreach ($sub in 'answers', 'private', 'work') {
        New-Item -ItemType Directory -Force (Join-Path $dir $sub) | Out-Null
    }
    $dir
}

# Fisher-Yates with a seeded generator: the order depends only on the seed.
function Get-Shuffled([object[]]$Items, [int]$Seed) {
    $rng = New-Object System.Random $Seed
    $a = @($Items)
    for ($i = $a.Count - 1; $i -gt 0; $i--) {
        $j = $rng.Next($i + 1)
        $t = $a[$i]; $a[$i] = $a[$j]; $a[$j] = $t
    }
    , $a
}

# An opaque sample ID: nothing in it names the arm, the run or the source.
function Get-OpaqueId([int]$Seed, [string]$Key) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $bytes = $sha.ComputeHash($Utf8.GetBytes("$Seed|$Key"))
    -join ($bytes[0..4] | ForEach-Object { $_.ToString('x2') })
}

# One sample: a fresh session in $WorkDir (created empty unless it exists, as a subject's fixture
# copy does). The message is fixed text; the prompt and the context travel as attached files, so
# nothing the agent reads passes through command-line quoting. Returns the raw output.
function Invoke-Sample {
    param(
        [Parameter(Mandatory)][string]$PromptFile,
        [Parameter(Mandatory)][string]$WorkDir,
        [Parameter(Mandatory)][string]$OutFile,
        [string[]]$Files = @(),
        [switch]$Auto,
        [int]$TimeoutMinutes = 6,
        [int]$Attempts = 3
    )
    if (-not (Test-Path $WorkDir)) { New-Item -ItemType Directory -Force $WorkDir | Out-Null }
    $a = @('run', '-m', $Model, '--format', 'json', '--file', $PromptFile)
    foreach ($f in $Files) { $a += @('--file', $f) }
    if ($Auto) { $a += '--auto' }
    $a += 'Follow the instructions in the attached prompt file. The other attached files are your context.'
    # A failed inference can leave `opencode run` waiting forever (seen 2026-10-11), so every
    # attempt has a time limit and is retried; the last attempt's output is kept either way.
    $line = ($a | ForEach-Object { if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ } }) -join ' '
    $out = ''
    for ($try = 1; $try -le $Attempts; $try++) {
        # A sample takes a minute or more; say so, so a quiet terminal is not mistaken for a hang.
        Write-Host ("[{0}] asking the model ({1}), attempt {2} of {3}..." -f (Get-Date -Format 'HH:mm:ss'), (Split-Path -Leaf $OutFile), $try, $Attempts)
        $tmp = "$OutFile.part"
        $p = Start-Process -FilePath (Get-OpencodeExe) -ArgumentList $line -WorkingDirectory $WorkDir `
            -NoNewWindow -PassThru -RedirectStandardOutput $tmp -RedirectStandardError "$tmp.err"
        if ($p.WaitForExit($TimeoutMinutes * 60 * 1000)) {
            $out = Read-Shared $tmp
            if ($out -match '"type":"text"') { break }
            Write-Host '  no answer in the output; retrying'
        } else {
            Write-Host "  no answer after $TimeoutMinutes minutes; stopping that attempt"
            $eap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
            & taskkill /T /F /PID $p.Id 2>&1 | Out-Null
            $ErrorActionPreference = $eap
            # The stopped process may hold the output file a moment longer (seen 2026-10-11).
            $p.WaitForExit(15000) | Out-Null
            $out = Read-Shared $tmp
        }
    }
    Remove-Item -ErrorAction SilentlyContinue $tmp, "$tmp.err"
    Write-Utf8 $OutFile $out
    $out
}

# Reads a file another process may still hold open; empty when it cannot be read after retries.
function Read-Shared([string]$Path) {
    for ($i = 0; $i -lt 10; $i++) {
        try {
            if (-not (Test-Path $Path)) { return '' }
            $fs = New-Object System.IO.FileStream($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
            try { return (New-Object System.IO.StreamReader($fs)).ReadToEnd() } finally { $fs.Dispose() }
        } catch {
            Start-Sleep -Milliseconds 500
        }
    }
    ''
}

# The real opencode executable: the npm wrapper (`opencode.cmd`) cannot be timed out and stopped
# as one process.
function Get-OpencodeExe {
    $cmd = (Get-Command opencode.cmd -ErrorAction SilentlyContinue).Source
    if ($cmd) {
        $exe = Join-Path (Split-Path -Parent $cmd) 'node_modules\@opencode\cli\bin\opencode.exe'
        if (Test-Path $exe) { return $exe }
    }
    (Get-Command opencode -CommandType Application | Select-Object -First 1).Source
}

# The last `LABEL: <word>` line of an answer, lower-cased; `unparsed` when absent or not allowed.
function Get-Label([string]$Text, [string[]]$Allowed) {
    $m = [regex]::Matches($Text, 'LABEL:\s*([A-Za-z-]+)')
    if ($m.Count -eq 0) { return 'unparsed' }
    $w = $m[$m.Count - 1].Groups[1].Value.ToLowerInvariant()
    if ($Allowed -contains $w) { $w } else { 'unparsed' }
}

# Every `ITEM: ...` line of an answer (checklist protocols).
function Get-Items([string]$Text) {
    $m = [regex]::Matches($Text, 'ITEM:\s*([^"\\\r\n]+)')
    @($m | ForEach-Object { $_.Groups[1].Value.Trim() })
}

# Runbook rule 4: model, opencode version, date, commit and seed.
function Get-Settings([string]$Protocol, [int]$Seed) {
    $root = Get-RepoRoot
    [ordered]@{
        protocol = $Protocol
        model    = $Model
        opencode = (& opencode --version | Out-String).Trim()
        date     = (Get-Date -Format 'yyyy-MM-dd')
        commit   = (& git -C $root rev-parse HEAD | Out-String).Trim()
        seed     = $Seed
    }
}

# Runbook rule 6: docs/plans/0.2/results/<date>-<protocol>.md, labelled agent-run (rule 8).
function Write-Result {
    param(
        [Parameter(Mandatory)]$Settings,
        [Parameter(Mandatory)][string]$Counts,
        [Parameter(Mandatory)][string]$Verdict,
        [string]$Threats = 'none noticed',
        [string]$Hashes = '',
        [string]$Milestone = '0.2'
    )
    $root = Get-RepoRoot
    $path = Join-Path $root ("docs\plans\$Milestone\results\{0}-{1}.md" -f $Settings.date, $Settings.protocol)
    $lines = @("# $($Settings.protocol) result ($($Settings.date), agent-run)", '', '## Settings', '')
    foreach ($k in $Settings.Keys) { $lines += "- $k`: $($Settings[$k])" }
    $lines += @('', '## Raw counts', '', $Counts, '', '## Verdict', '', $Verdict, '', '## Threats noticed', '', $Threats)
    if ($Hashes) { $lines += @('', '## Transcript hashes (SHA-256; transcripts stay outside the repository)', '', $Hashes) }
    Write-Utf8 $path (($lines -join "`n") + "`n")
    $path
}

# The SHA-256 of every answer file of a run, one line each.
function Get-AnswerHashes([string]$RunDir) {
    (Get-ChildItem (Join-Path $RunDir 'answers') -File | Sort-Object Name |
        ForEach-Object { '- {0} {1}' -f $_.Name, (Get-Sha256 $_.FullName) }) -join "`n"
}
