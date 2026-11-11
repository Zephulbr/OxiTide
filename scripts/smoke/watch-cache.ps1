# Cache watcher for the 2026-10-24 engine smokes.
#
#   powershell -ExecutionPolicy Bypass -File scripts/smoke/watch-cache.ps1 -Mode new   -Seconds 90
#   powershell -ExecutionPolicy Bypass -File scripts/smoke/watch-cache.ps1 -Mode spill -Seconds 90
#
# new   - prints each .fmp4 that appears after the baseline (download-ahead row)
# spill - prints each spill-*.part that appears and disappears (seek row)
param(
  [ValidateSet("new", "spill")] [string]$Mode = "new",
  [int]$Seconds = 90,
  [string]$CacheDir = "$env:APPDATA\OxiTide\config\cache"
)

$start = Get-Date
$elapsed = { "{0:N1}s" -f ((Get-Date) - $start).TotalSeconds }

if ($Mode -eq "new") {
  $known = @{}
  Get-ChildItem $CacheDir -Filter "*.fmp4" -ErrorAction SilentlyContinue |
    ForEach-Object { $known[$_.Name] = $true }
  Write-Host "baseline: $($known.Count) cached tracks; watching $Seconds s"
  while (((Get-Date) - $start).TotalSeconds -lt $Seconds) {
    Get-ChildItem $CacheDir -Filter "*.fmp4" -ErrorAction SilentlyContinue | ForEach-Object {
      if (-not $known.ContainsKey($_.Name)) {
        $known[$_.Name] = $true
        $meta = Test-Path ($_.FullName + ".meta")
        Write-Host ("[{0}] new {1}  {2:N1} MB  sidecar={3}" -f (& $elapsed), $_.Name, ($_.Length / 1MB), $meta)
      }
    }
    Start-Sleep -Milliseconds 500
  }
} else {
  $present = @{}
  Write-Host "watching for spill files in $CacheDir for $Seconds s"
  while (((Get-Date) - $start).TotalSeconds -lt $Seconds) {
    Get-ChildItem $CacheDir -Filter "spill-*.part" -ErrorAction SilentlyContinue | ForEach-Object {
      if (-not $present.ContainsKey($_.Name)) {
        $present[$_.Name] = $true
        Write-Host ("[{0}] appeared {1}  {2:N1} MB" -f (& $elapsed), $_.Name, ($_.Length / 1MB))
      }
    }
    foreach ($name in @($present.Keys)) {
      if (-not (Test-Path (Join-Path $CacheDir $name))) {
        $present.Remove($name)
        Write-Host ("[{0}] gone {1}" -f (& $elapsed), $name)
      }
    }
    Start-Sleep -Milliseconds 250
  }
}
Write-Host "done"
