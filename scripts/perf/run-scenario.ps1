# Runs a scripted or manual perf scenario N times and then evaluates budgets.
# -Release measures the shipped configuration: the release exe with the UI
# embedded (build it with `npm --prefix ui run app:build:perf`), no Vite.
param(
    [Parameter(Mandatory = $true)][string]$Scenario,
    [int]$Runs = 5,
    [switch]$Manual,
    [switch]$SkipCheck,
    [switch]$Release,
    [string]$Exe = "",
    [int]$Port = 9222,
    [int]$Minutes = 0
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$perfDir = Join-Path $root ".superpowers\perf\runs"
New-Item -ItemType Directory -Force -Path $perfDir | Out-Null
if (-not $Exe) {
    $Exe = if ($Release) { "target/release/oxitide-app.exe" } else { "target/debug/oxitide-app.exe" }
}

function Wait-ForPort([int]$p, [int]$seconds) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-NetConnection -ComputerName localhost -Port $p -InformationLevel Quiet -WarningAction SilentlyContinue) { return $true }
        Start-Sleep -Milliseconds 500
    }
    return $false
}

# Debug builds load the Vite dev server; make sure it is up.
if (-not $Release -and -not (Test-NetConnection -ComputerName localhost -Port 1420 -InformationLevel Quiet -WarningAction SilentlyContinue)) {
    Write-Host "starting Vite on 1420 ..."
    Start-Process -FilePath "npm.cmd" -ArgumentList "--prefix", "ui", "run", "dev" -WorkingDirectory $root -WindowStyle Hidden
    if (-not (Wait-ForPort 1420 60)) { throw "Vite did not start on port 1420" }
}

$env:OXITIDE_PERF_DIR = $perfDir
$env:OXITIDE_PERF_SCENARIO = $Scenario
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
$exePath = Join-Path $root $Exe
if (-not (Test-Path -LiteralPath $exePath)) {
    $hint = if ($Release) { "npm --prefix ui run app:build:perf" } else { "cargo build -p oxitide-app" }
    throw "app binary not found: $exePath (run $hint)"
}

for ($i = 1; $i -le $Runs; $i++) {
    Write-Host "run $i/$Runs ($Scenario)"
    $proc = Start-Process -FilePath $exePath -PassThru
    try {
        if ($Manual) {
            Read-Host "Perform the $Scenario scenario in the app, then press Enter to close it"
        } else {
            $driverArgs = @("$root\scripts\perf\drive.mjs", "--scenario", $Scenario, "--port", "$Port")
            if ($Minutes -gt 0) { $driverArgs += @("--minutes", "$Minutes") }
            node @driverArgs
            if ($LASTEXITCODE -ne 0) { Write-Warning "driver reported a failure for run $i" }
        }
    } finally {
        # The driver asks the app to quit (perf_quit) so exit work runs; only
        # an app still up after 10 s is killed.
        if (-not $proc.WaitForExit(10000)) {
            Write-Warning "app did not exit on its own; killing it"
            & taskkill /PID $proc.Id /T /F | Out-Null
            $proc.WaitForExit(5000) | Out-Null
        }
        Start-Sleep -Milliseconds 1500
    }
}

if (-not $SkipCheck) {
    node "$root\scripts\perf\check.mjs" --dir $perfDir --scenario $Scenario
    exit $LASTEXITCODE
}
