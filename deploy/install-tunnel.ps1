# One-time setup: starts tunnel.ps1 hidden every time you log into Windows.
#   powershell -ExecutionPolicy Bypass -File deploy\install-tunnel.ps1 -Server user@your-server
# Remove it again with:  Unregister-ScheduledTask -TaskName "Aeon Explorer tunnel" -Confirm:$false
param(
    [Parameter(Mandatory = $true)]
    [string]$Server,
    [int]$Port = 5050
)

$name = "Aeon Explorer tunnel"
$script = Join-Path $PSScriptRoot "tunnel.ps1"

$action = New-ScheduledTaskAction -Execute "powershell.exe" `
    -Argument "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File `"$script`" -Server `"$Server`" -Port $Port"
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
$settings = New-ScheduledTaskSettingsSet `
    -ExecutionTimeLimit ([TimeSpan]::Zero) `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Settings $settings -Force | Out-Null
Start-ScheduledTask -TaskName $name

Write-Host "Installed. Open http://localhost:$Port (give it a few seconds the first time)."
