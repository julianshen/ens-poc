#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Remove the TNS notification agent service and its AUMID registration.
#>
param(
    [switch]$RemoveFiles,
    [string]$InstallDir = "C:\Program Files\YourCo"
)

$ErrorActionPreference = 'Stop'

$ServiceName = 'YourCoNotificationAgent'
$Aumid       = 'YourCo.NotificationAgent'
$DisplayName = 'Acme Notification Agent'

if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    Write-Host "Stopping and deleting $ServiceName ..."
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    & sc.exe delete $ServiceName | Out-Null
} else {
    Write-Host "$ServiceName is not installed."
}

Remove-Item -Path "HKLM:\SOFTWARE\Classes\AppUserModelId\$Aumid" -Recurse -Force -ErrorAction SilentlyContinue

$StartMenuLnk = Join-Path ([Environment]::GetFolderPath('CommonPrograms')) "$DisplayName.lnk"
Remove-Item -Path $StartMenuLnk -Force -ErrorAction SilentlyContinue

if ([System.Diagnostics.EventLog]::SourceExists($ServiceName)) {
    Remove-EventLog -Source $ServiceName
}

if ($RemoveFiles -and (Test-Path $InstallDir)) {
    Remove-Item -Path $InstallDir -Recurse -Force
    Write-Host "Removed $InstallDir."
}

Write-Host "Uninstalled $ServiceName."
