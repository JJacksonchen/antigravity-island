#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$PurgeConfig,
    [string]$InstallDir = "$env:LOCALAPPDATA\AntigravityIsland"
)

$ErrorActionPreference = "SilentlyContinue"

Get-Process -Name "antigravity-island" | Stop-Process -Force
Start-Sleep -Milliseconds 300

$RunRegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
Remove-ItemProperty -Path $RunRegKey -Name "AntigravityIsland"

$ShortcutPath = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Antigravity Island.lnk"
if (Test-Path $ShortcutPath) {
    Remove-Item -Path $ShortcutPath -Force
    Write-Host "Removed Start Menu shortcut: $ShortcutPath"
}

if (Test-Path $InstallDir) {
    Remove-Item -Path $InstallDir -Recurse -Force
    Write-Host "Removed install directory: $InstallDir"
}

if ($PurgeConfig) {
    $ConfigDir = Join-Path $env:USERPROFILE ".config\antigravity-island"
    if (Test-Path $ConfigDir) {
        Remove-Item -Path $ConfigDir -Recurse -Force
        Write-Host "Removed config directory: $ConfigDir"
    }
}

Write-Host "Antigravity Island uninstalled." -ForegroundColor Green
