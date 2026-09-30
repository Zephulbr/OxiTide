# Runs a scripted or manual perf scenario N times and then evaluates budgets.
param(
    [Parameter(Mandatory = $true)][string]$Scenario,
    [int]$Runs = 5,
    [switch]$Manual,
    [switch]$SkipCheck,
    [string]$Exe = "target/debug/oxitide-app.exe",
    [int]$Port = 9222
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$perfDir = Join-Path $root ".superpowers\perf\runs"
New-Item -ItemType Directory -Force -Path $perfDir | Out-Null

function Wait-ForPort([int]$p, [int]$seconds) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-NetConnection -ComputerName localhost -Port $p -InformationLevel Quiet -WarningAction SilentlyContinue) { return $true }
        Start-Sleep -Milliseconds 500
    }
    return $false
}

# Debug builds load the Vite dev server; make sure it is up.
if (-not (Test-NetConnection -ComputerName localhost -Port 1420 -InformationLevel Quiet -WarningAction SilentlyContinue)) {
    Write-Host "starting Vite on 1420 ..."
    Start-Process -FilePath "npm.cmd" -ArgumentList "--prefix", "ui", "run", "dev" -WorkingDirectory $root -WindowStyle Hidden
    if (-not (Wait-ForPort 1420 60)) { throw "Vite did not start on port 1420" }
}

$env:OXITIDE_PERF_DIR = $perfDir
$env:OXITIDE_PERF_SCENARIO = $Scenario
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
$exePath = Join-Path $root $Exe
if (-not (Test-Path -LiteralPath $exePath)) { throw "app binary not found: $exePath (run npm --prefix ui run app:build or cargo build -p oxitide-app)" }

for ($i = 1; $i -le $Runs; $i++) {
    Write-Host "run $i/$Runs ($Scenario)"
    $proc = Start-Process -FilePath $exePath -PassThru
    try {
        if ($Manual) {
            Read-Host "Perform the $Scenario scenario in the app, then press Enter to close it"
        } else {
            node "$root\scripts\perf\drive.mjs" --scenario $Scenario --port $Port
            if ($LASTEXITCODE -ne 0) { Write-Warning "driver reported a failure for run $i" }
        }
    } finally {
        if (-not $proc.HasExited) { & taskkill /PID $proc.Id /T /F | Out-Null }
        $proc.WaitForExit(5000) | Out-Null
        Start-Sleep -Milliseconds 1500
    }
}

if (-not $SkipCheck) {
    node "$root\scripts\perf\check.mjs" --dir $perfDir --scenario $Scenario
    exit $LASTEXITCODE
}
