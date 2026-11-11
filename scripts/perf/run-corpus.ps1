# Unattended release corpus: runs a list of perf scenarios back to back
# (release build), then prints `check.mjs --metrics` and the budget check.
#
#   powershell -ExecutionPolicy Bypass -File scripts/perf/run-corpus.ps1
#
# - Moves any existing JSONL in .superpowers/perf/runs/ into runs-archive-<stamp>/,
#   so check.mjs sees only this corpus.
# - Backs up config.toml, session.json and history.jsonl, turns scrobbling off for
#   the run (no perf plays reach Last.fm/ListenBrainz), and restores all three at
#   the end, so the perf plays leave no trace in history or the saved queue. The
#   backup is deleted once the restore is verified, and kept (with a RESTORE
#   INCOMPLETE line in corpus.log) if it is not.
# - Copies the corpus's raw JSONL into corpus-<stamp>\runs\ at the end, so each
#   corpus folder can be re-checked on its own.
# - Keeps the display on and the PC awake while it runs (SetThreadExecutionState):
#   a display that powers off marks the WebView occluded and throttles its frames.
# - Each scenario runs in a child process with a timeout; a hung one is killed
#   and the corpus goes on.
# S12 hides the window to the tray itself (perf_close_window), so it runs unattended too.
param(
    [string[]]$Scenarios = @(
        "S1", "S2",
        "S5-instrument", "S5-atmosphere", "S5-atmosphere-light", "S6", "S7", "S14", "S13",
        "S3-miss", "S3-hit",
        "S11", "S12",
        "S4", "S8", "S9"
    ),
    [int]$Runs = 5,
    [int]$Port = 9222
)
$ErrorActionPreference = "Stop"
# `-File` passes "S1,S2" as one string.
$Scenarios = @($Scenarios | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$perfRoot = Join-Path $root ".superpowers\perf"
$runsDir = Join-Path $perfRoot "runs"
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$outDir = Join-Path $perfRoot "corpus-$stamp"
$backupDir = Join-Path $outDir "state-backup"
$cfgDir = Join-Path $env:APPDATA "OxiTide\config"
$exe = Join-Path $root "target\release\oxitide-app.exe"
$runScenario = Join-Path $PSScriptRoot "run-scenario.ps1"

# Runs per scenario and the per-scenario timeout in minutes. S8/S9 are long
# single runs (S8 plays its 10 shortest favorites twice, S9 is a 2 h soak).
$plan = @{
    "S1" = @($Runs, 10); "S2" = @($Runs, 10)
    "S3-miss" = @($Runs, 20); "S3-hit" = @($Runs, 30)
    "S4" = @($Runs, 90)
    "S5-instrument" = @($Runs, 15); "S5-atmosphere" = @($Runs, 15); "S5-atmosphere-light" = @($Runs, 15)
    "S6" = @($Runs, 15); "S7" = @($Runs, 15)
    "S8" = @(1, 120); "S9" = @(1, 150)
    "S11" = @($Runs, 50); "S12" = @($Runs, 75); "S13" = @($Runs, 30); "S14" = @($Runs, 15)
}

New-Item -ItemType Directory -Force -Path $outDir, $backupDir, $runsDir | Out-Null
$log = Join-Path $outDir "corpus.log"
function Log([string]$msg) {
    $line = "$(Get-Date -Format 'HH:mm:ss') $msg"
    Write-Host $line
    Add-Content -LiteralPath $log -Value $line -Encoding utf8
}

# --- pre-flight -------------------------------------------------------------
if (-not (Test-Path -LiteralPath $exe)) { throw "release exe missing: run npm --prefix ui run app:build:perf" }
if (Get-Process oxitide-app -ErrorAction SilentlyContinue) { throw "an oxitide-app is running; quit it first (single instance would swallow the perf launch)" }
if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) { throw "port $Port is already in use" }
$unknown = $Scenarios | Where-Object { -not $plan.ContainsKey($_) }
if ($unknown) { throw "unknown scenario(s): $($unknown -join ', ')" }

$commit = (git -C $root rev-parse --short HEAD).Trim()
$dirty = [bool](git -C $root status --porcelain --untracked-files=no)
$exeInfo = Get-Item -LiteralPath $exe
Log "corpus ${stamp}: commit $commit$(if ($dirty) { ' (tracked changes!)' }), exe built $($exeInfo.LastWriteTime)"
Log "scenarios: $($Scenarios -join ', ')"

# --- keep awake -------------------------------------------------------------
Add-Type -Namespace OxiPerf -Name Power -MemberDefinition @'
[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint esFlags);
'@
# ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
[OxiPerf.Power]::SetThreadExecutionState([uint32]"0x80000003") | Out-Null

# --- isolate the corpus and the owner's state -------------------------------
$old = Get-ChildItem -LiteralPath $runsDir -Filter *.jsonl -File
if ($old) {
    $archive = Join-Path $perfRoot "runs-archive-$stamp"
    New-Item -ItemType Directory -Force -Path $archive | Out-Null
    $old | Move-Item -Destination $archive
    Log "moved $($old.Count) old run file(s) to $archive"
}
$stateFiles = @("config.toml", "session.json", "history.jsonl")
foreach ($f in $stateFiles) {
    $src = Join-Path $cfgDir $f
    if (Test-Path -LiteralPath $src) { Copy-Item -LiteralPath $src -Destination $backupDir }
}
Log "backed up $($stateFiles -join ', ') to $backupDir"
$cfgPath = Join-Path $cfgDir "config.toml"
$cfg = [IO.File]::ReadAllText($cfgPath)
$cfg = $cfg -replace '(?m)^(lastfm_enabled|listenbrainz_enabled) = true', '$1 = false'
[IO.File]::WriteAllText($cfgPath, $cfg, (New-Object Text.UTF8Encoding($false)))
Log "scrobbling off for the run"

$summary = @()
try {
    foreach ($id in $Scenarios) {
        $n = $plan[$id][0]; $limit = $plan[$id][1]
        $scenarioLog = Join-Path $outDir "$id.log"
        Log "start $id ($n run(s), timeout $limit min)"
        $started = Get-Date
        $p = Start-Process -FilePath "powershell.exe" -PassThru -NoNewWindow `
            -RedirectStandardOutput $scenarioLog -RedirectStandardError "$scenarioLog.err" `
            -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "`"$runScenario`"",
                "-Scenario", $id, "-Runs", "$n", "-Release", "-SkipCheck", "-Port", "$Port")
        $timedOut = -not $p.WaitForExit($limit * 60 * 1000)
        if ($timedOut) {
            Log "TIMEOUT $id after $limit min; killing it"
            & taskkill /PID $p.Id /T /F 2>$null | Out-Null
        }
        # Nothing may outlive a scenario: a leftover app would take the next launch.
        Get-Process oxitide-app -ErrorAction SilentlyContinue | ForEach-Object {
            Log "killing leftover oxitide-app $($_.Id)"
            & taskkill /PID $_.Id /T /F 2>$null | Out-Null
        }
        Start-Sleep -Seconds 3
        $warnings = (Select-String -LiteralPath $scenarioLog -Pattern "WARNING|failure|Error" -SimpleMatch:$false -ErrorAction SilentlyContinue | Measure-Object).Count
        $files = (Get-ChildItem -LiteralPath $runsDir -Filter "$id-*.jsonl" -File | Measure-Object).Count
        $mins = [math]::Round(((Get-Date) - $started).TotalMinutes, 1)
        Log "done  $id in $mins min: $files run file(s), $warnings warning line(s)$(if ($timedOut) { ', TIMED OUT' })"
        $summary += [pscustomobject]@{ scenario = $id; runs = $n; files = $files; minutes = $mins; timed_out = $timedOut; warning_lines = $warnings }
    }
} finally {
    # Nothing of the app may still be writing when the files go back: a late
    # write would leave a perf queue/history behind, and the next backup would
    # keep it. Restore, then check again after a pause.
    Get-Process oxitide-app -ErrorAction SilentlyContinue | ForEach-Object { & taskkill /PID $_.Id /T /F 2>$null | Out-Null }
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        foreach ($f in $stateFiles) {
            $bak = Join-Path $backupDir $f
            if (Test-Path -LiteralPath $bak) { Copy-Item -LiteralPath $bak -Destination $cfgDir -Force }
        }
        Start-Sleep -Seconds 5
        $drift = @($stateFiles | Where-Object {
            $bak = Join-Path $backupDir $_
            (Test-Path -LiteralPath $bak) -and ((Get-FileHash -LiteralPath $bak).Hash -ne (Get-FileHash -LiteralPath (Join-Path $cfgDir $_)).Hash)
        })
        if ($drift.Count -eq 0) { break }
        Log "state changed after restore ($($drift -join ', ')); restoring again"
    }
    if ($drift.Count -eq 0) {
        Log "restored $($stateFiles -join ', ') (scrobbling back as it was)"
        # The backup is a copy of the owner's history and config (Last.fm key
        # included); once the restore is verified it has no further use.
        Remove-Item -LiteralPath $backupDir -Recurse -Force
        Log "removed $backupDir (restore verified)"
    } else {
        Log "RESTORE INCOMPLETE: $($drift -join ', ') still differ after 3 attempts; the backup stays in $backupDir. Copy it to $cfgDir by hand once nothing is running."
    }
    [OxiPerf.Power]::SetThreadExecutionState([uint32]"0x80000000") | Out-Null
    $summary | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outDir "summary.json") -Encoding utf8
    $summary | Format-Table -AutoSize | Out-String | ForEach-Object { Log $_ }
}

# Keep this corpus's raw rows with it, so it can be re-checked later
# (`node scripts/perf/check.mjs --dir <corpus>\runs`) after the next corpus
# has moved them out of runs\.
$corpusRuns = Join-Path $outDir "runs"
New-Item -ItemType Directory -Force -Path $corpusRuns | Out-Null
Get-ChildItem -LiteralPath $runsDir -Filter *.jsonl -File | Copy-Item -Destination $corpusRuns
node (Join-Path $PSScriptRoot "check.mjs") --dir $runsDir --metrics 2>&1 | Out-File -LiteralPath (Join-Path $outDir "metrics.txt") -Encoding utf8
node (Join-Path $PSScriptRoot "check.mjs") --dir $runsDir 2>&1 | Out-File -LiteralPath (Join-Path $outDir "check.txt") -Encoding utf8
Log "check.mjs exit $LASTEXITCODE; results in $outDir (record the corpus id and this exit code in the gate note)"
