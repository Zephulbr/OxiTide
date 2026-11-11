# Elevated network helper for the engine smokes (network resilience rows;
# first used for the 2026-10-24 v1 release-gate run).
#
# Run once in an elevated PowerShell:
#   powershell -ExecutionPolicy Bypass -File scripts/smoke/net-toggle.ps1
# Leave it open for the session; Ctrl+C when done.
#
# It watches a command file and applies one action per request:
#   down | up      - disable / enable the Ethernet adapter ("network down")
#   block | unblock - block / allow only the app's exe (link up, no internet)
#   status         - print adapter + firewall state
#
# Actions are applied with Windows' own tools and reported on the console.
param(
  [string]$CommandFile = "$env:TEMP\oxitide-net.cmd",
  # The release exe of this checkout; pass -Exe for an installed copy
  # (%LOCALAPPDATA%\OxiTide\oxitide-app.exe).
  [string]$Exe = (Join-Path $PSScriptRoot "..\..\target\release\oxitide-app.exe"),
  [string]$Adapter = "Ethernet",
  [string]$RuleName = "OxiTide smoke block"
)

$ErrorActionPreference = "Stop"
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Write-Host "This helper must run elevated." -ForegroundColor Red
  exit 1
}

Remove-Item $CommandFile -ErrorAction SilentlyContinue
Write-Host "Watching $CommandFile"
Write-Host "Adapter: $Adapter   Exe: $Exe"
Write-Host "Ctrl+C to stop."

while ($true) {
  if (Test-Path $CommandFile) {
    $cmd = (Get-Content $CommandFile -Raw -ErrorAction SilentlyContinue)
    if ($cmd) { $cmd = $cmd.Trim().ToLowerInvariant() }
    Remove-Item $CommandFile -Force -ErrorAction SilentlyContinue
    try {
      switch ($cmd) {
        "down"    { Disable-NetAdapter -Name $Adapter -Confirm:$false; Write-Host "[$(Get-Date -Format HH:mm:ss)] adapter down" }
        "up"      { Enable-NetAdapter  -Name $Adapter -Confirm:$false; Write-Host "[$(Get-Date -Format HH:mm:ss)] adapter up" }
        "block"   {
          Remove-NetFirewallRule -DisplayName $RuleName -ErrorAction SilentlyContinue
          New-NetFirewallRule -DisplayName $RuleName -Direction Outbound -Program $Exe -Action Block -Profile Any | Out-Null
          Write-Host "[$(Get-Date -Format HH:mm:ss)] app blocked (link stays up)"
        }
        "unblock" {
          Remove-NetFirewallRule -DisplayName $RuleName -ErrorAction SilentlyContinue
          Write-Host "[$(Get-Date -Format HH:mm:ss)] app unblocked"
        }
        "status"  {
          (Get-NetAdapter -Name $Adapter | Select-Object Name, Status | Out-String).Trim() | Write-Host
          $rule = Get-NetFirewallRule -DisplayName $RuleName -ErrorAction SilentlyContinue
          if ($rule) { Write-Host "firewall block: present" } else { Write-Host "firewall block: absent" }
        }
        default   { Write-Host "unknown command: '$cmd'" }
      }
    } catch {
      Write-Host "[$(Get-Date -Format HH:mm:ss)] FAILED '$cmd': $($_.Exception.Message)" -ForegroundColor Red
    }
  }
  Start-Sleep -Milliseconds 400
}
