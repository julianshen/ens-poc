#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Install the TNS notification agent as a Windows service.

.DESCRIPTION
    Implements spec section 7 "Windows Service details":
      - Start type Automatic  -> agent returns after a reboot
      - Runs as Local System  -> default account for New-Service
      - Restart on failure: Yes, after 10 seconds (SCM relaunches on a crash)
    Also registers the AUMID (spec section 7) so toasts/badges are attributed
    to the agent.

    The restart-recovery values here match `ServiceSpec::sc_failure_args` in
    src/service.rs (reset= 86400, actions= restart/10000) -- keep them in sync.
#>
param(
    [string]$BinaryPath  = "$PSScriptRoot\tns.exe",
    [string]$InstallDir  = "C:\Program Files\YourCo",
    [string]$ConfigSource = "$PSScriptRoot\agent.toml"
)

$ErrorActionPreference = 'Stop'

$ServiceName = 'YourCoNotificationAgent'
$DisplayName = 'Acme Notification Agent'
$Aumid       = 'YourCo.NotificationAgent'
$RestartMs   = 10000   # spec section 7: restart 10s after a failure
$ResetSecs   = 86400   # window after which the failure counter resets (1 day)

Write-Host "Installing $ServiceName ..."

# 1. Lay down binary + config.
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -Path $BinaryPath -Destination (Join-Path $InstallDir 'tns.exe') -Force
$IconSource = Join-Path $PSScriptRoot 'agent.ico'
if (Test-Path $IconSource) {
    Copy-Item -Path $IconSource -Destination (Join-Path $InstallDir 'agent.ico') -Force
}
if (Test-Path $ConfigSource) {
    Copy-Item -Path $ConfigSource -Destination (Join-Path $InstallDir 'agent.toml') -Force
} else {
    Write-Warning "No agent.toml found at $ConfigSource; copy one to $InstallDir before starting."
}
$ExePath = Join-Path $InstallDir 'tns.exe'

# 2. Register the AUMID (spec section 7).
$AumidKey = "HKLM:\SOFTWARE\Classes\AppUserModelId\$Aumid"
New-Item -Path $AumidKey -Force | Out-Null
New-ItemProperty -Path $AumidKey -Name 'DisplayName' -Value $DisplayName -PropertyType String -Force | Out-Null
New-ItemProperty -Path $AumidKey -Name 'IconUri' -Value (Join-Path $InstallDir 'agent.ico') -PropertyType String -Force | Out-Null

# 2b. Create a Start Menu shortcut carrying the AUMID. REQUIRED: an unpackaged
#     Win32 app cannot raise toasts on the registry key alone — without this
#     shortcut, ToastNotification.Show() succeeds but the toast is dropped.
#     (Verified by smoke test: the toast only reached the notification center
#     once this shortcut existed.)
$ShortcutHelper = Join-Path $PSScriptRoot 'New-AumidShortcut.ps1'
if (-not (Test-Path $ShortcutHelper)) {
    $ShortcutHelper = Join-Path $PSScriptRoot '..\tools\New-AumidShortcut.ps1'
}
$StartMenuLnk = Join-Path ([Environment]::GetFolderPath('CommonPrograms')) "$DisplayName.lnk"
& $ShortcutHelper -ShortcutPath $StartMenuLnk -TargetPath $ExePath -Aumid $Aumid `
                  -IconPath (Join-Path $InstallDir 'agent.ico')

# 3. Register the Event Log source so entries are attributed (spec section 9 #7).
#    The source name matches EVENT_SOURCE in src/eventlog.rs.
if (-not [System.Diagnostics.EventLog]::SourceExists($ServiceName)) {
    New-EventLog -LogName Application -Source $ServiceName
}

# 4. (Re)create the service: Automatic start, Local System (New-Service default).
if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    Write-Host "  Existing service found; removing it first."
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    & sc.exe delete $ServiceName | Out-Null
    Start-Sleep -Seconds 2
}
New-Service -Name $ServiceName `
            -DisplayName $DisplayName `
            -BinaryPathName "`"$ExePath`" --service" `
            -StartupType Automatic | Out-Null

# 5. Configure restart-on-failure recovery (spec section 7). The failureflag
#    makes the SCM run recovery actions when the service stops with a non-zero
#    exit code too (not only on a hard crash) — the agent reports exit code 1
#    when run_agent fails (see service_runtime.rs).
& sc.exe failure $ServiceName reset= $ResetSecs actions= restart/$RestartMs | Out-Null
& sc.exe failureflag $ServiceName 1 | Out-Null

# 6. Start it.
Start-Service -Name $ServiceName
Write-Host "Done. $ServiceName is installed (Automatic) and running, and will"
Write-Host "restart automatically $([int]($RestartMs/1000))s after an unexpected exit."
