#Requires -Version 5.1
<#
.SYNOPSIS
    Antigravity Island One-Click Installer for Windows 10 / 11
.EXAMPLE
    irm https://raw.githubusercontent.com/JJacksonchen/antigravity-island/main/scripts/install.ps1 | iex
.EXAMPLE
    & ([scriptblock]::Create((irm https://raw.githubusercontent.com/JJacksonchen/antigravity-island/main/scripts/install.ps1))) -Autostart
#>
[CmdletBinding()]
param(
    [switch]$Autostart,
    [switch]$NoLaunch,
    [switch]$FromSource,
    [string]$InstallDir = "$env:LOCALAPPDATA\AntigravityIsland"
)

$ErrorActionPreference = "Stop"

$RepoOwner = "JJacksonchen"
$RepoName = "antigravity-island"
$RepoUrl = "https://github.com/$RepoOwner/$RepoName.git"
$ReleaseZipUrl = "https://github.com/$RepoOwner/$RepoName/releases/latest/download/antigravity-island-windows-x86_64.zip"
$ExeName = "antigravity-island.exe"
$RunRegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$RunValueName = "AntigravityIsland"

function Write-Step([string]$Msg) { Write-Host "==> $Msg" -ForegroundColor Cyan }
function Write-Ok([string]$Msg)   { Write-Host "[OK] $Msg" -ForegroundColor Green }
function Write-Warn([string]$Msg) { Write-Host "[!] $Msg" -ForegroundColor Yellow }

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("agy-island-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null

try {
    $BinSrc = $null

    if (-not $FromSource) {
        Write-Step "Downloading prebuilt Windows x64 release from GitHub Releases..."
        $ZipPath = Join-Path $TempDir "release.zip"
        try {
            Invoke-WebRequest -Uri $ReleaseZipUrl -OutFile $ZipPath -UseBasicParsing
            $ExtractDir = Join-Path $TempDir "extract"
            Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force
            $Candidate = Join-Path $ExtractDir $ExeName
            if (Test-Path $Candidate) {
                $BinSrc = $Candidate
                Write-Ok "Downloaded prebuilt binary"
            }
        } catch {
            Write-Warn "Prebuilt Windows release unavailable; falling back to cargo source build."
        }
    }

    if (-not $BinSrc) {
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            throw "Rust (cargo) is required to build from source. Install from https://rustup.rs and retry."
        }
        if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
            throw "git is required to clone $RepoUrl"
        }
        $SrcDir = Join-Path $TempDir "src"
        Write-Step "Cloning $RepoUrl..."
        git clone --depth 1 $RepoUrl $SrcDir | Out-Null
        Write-Step "Building optimized release binary with cargo..."
        cargo build --release --manifest-path (Join-Path $SrcDir "Cargo.toml")
        $BinSrc = Join-Path $SrcDir "target\release\$ExeName"
    }

    if (-not (Test-Path $BinSrc)) {
        throw "Compiled binary not found at $BinSrc"
    }

    # Stop any running instance cleanly before overwriting
    Get-Process -Name "antigravity-island" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $DestExe = Join-Path $InstallDir $ExeName
    Copy-Item -Path $BinSrc -Destination $DestExe -Force
    Write-Ok "Installed executable: $DestExe"

    # Add InstallDir to user PATH if missing
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($null -eq $UserPath) { $UserPath = "" }
    $PathEntries = $UserPath -split ";" | Where-Object { $_ -ne "" }
    if ($PathEntries -notcontains $InstallDir) {
        $NewPath = ($PathEntries + $InstallDir) -join ";"
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
        Write-Ok "Added $InstallDir to user PATH"
    }

    # Create Start Menu shortcut
    try {
        $StartMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
        $ShortcutPath = Join-Path $StartMenu "Antigravity Island.lnk"
        $Wsh = New-Object -ComObject WScript.Shell
        $Shortcut = $Wsh.CreateShortcut($ShortcutPath)
        $Shortcut.TargetPath = $DestExe
        $Shortcut.WorkingDirectory = $InstallDir
        $Shortcut.Description = "Google Antigravity Dynamic Island & Quota Widget"
        $Shortcut.Save()
        Write-Ok "Created Start Menu shortcut: $ShortcutPath"
    } catch {
        Write-Warn "Could not create Start Menu shortcut: $_"
    }

    # Initialize default skins folder
    & $DestExe --list-skins | Out-Null
    Write-Ok "Initialized skins directory"

    if ($Autostart) {
        New-Item -Path $RunRegKey -Force | Out-Null
        Set-ItemProperty -Path $RunRegKey -Name $RunValueName -Value "`"$DestExe`""
        Write-Ok "Enabled Windows login auto-start ($RunRegKey\$RunValueName)"
    }

    Write-Host ""
    Write-Host "Antigravity Island installed successfully!" -ForegroundColor Green
    Write-Host "  Live quota probe : antigravity-island --probe"
    Write-Host "  Preview demo UI  : antigravity-island --demo"
    Write-Host "  Executable path  : $DestExe"

    if (-not $NoLaunch) {
        Start-Process -FilePath $DestExe
        Write-Ok "Launched Antigravity Island"
    }
} finally {
    if (Test-Path $TempDir) {
        Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
