# Live log watcher for the 2026-10-24 engine smokes.
#
#   powershell -ExecutionPolicy Bypass -File scripts/smoke/watch-log.ps1 -Seconds 300
#
# Prints new matching lines from the newest app log, with the UTC timestamp
# converted to local time (the file is named and stamped in UTC).
param(
  [int]$Seconds = 300,
  [string]$Pattern = "network|waiting|resumed|ERROR|underrun|failed|offline",
  [string]$LogDir = "$env:APPDATA\OxiTide\logs"
)

$file = Get-ChildItem $LogDir -Filter "oxitide.*.log" -ErrorAction SilentlyContinue |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $file) { Write-Host "no log file in $LogDir"; exit 1 }

$seen = @(Get-Content -LiteralPath $file.FullName -ErrorAction SilentlyContinue).Count
Write-Host "watching $($file.Name) from line $seen for $Seconds s"
$start = Get-Date

while (((Get-Date) - $start).TotalSeconds -lt $Seconds) {
  $lines = @(Get-Content -LiteralPath $file.FullName -ErrorAction SilentlyContinue)
  if ($lines.Count -gt $seen) {
    for ($i = $seen; $i -lt $lines.Count; $i++) {
      $line = $lines[$i]
      if ($line -match $Pattern) {
        $stamp = ""
        if ($line -match '^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?)Z') {
          try {
            $utc = [datetime]::Parse($Matches[1], [cultureinfo]::InvariantCulture, [System.Globalization.DateTimeStyles]::AssumeUniversal)
            $stamp = $utc.ToLocalTime().ToString("HH:mm:ss")
          } catch { $stamp = "" }
        }
        $body = $line -replace '^\S+Z\s+', ''
        Write-Host "[$stamp] $body"
      }
    }
    $seen = $lines.Count
  }
  Start-Sleep -Milliseconds 400
}
Write-Host "done"
