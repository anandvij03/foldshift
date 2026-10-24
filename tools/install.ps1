<#
.SYNOPSIS
    Installs the latest FoldShift binary on Windows (x86_64).
.DESCRIPTION
    Downloads the precompiled FoldShift distribution from GitHub Releases,
    extracts it to %LOCALAPPDATA%\foldshift\bin, and optionally adds it to PATH.
#>
param(
    [string]$Profile = "web", # "core", "images", or "web"
    [string]$Version = "latest",
    [string]$InstallDir = "$env:LOCALAPPDATA\foldshift\bin"
)

$ErrorActionPreference = "Stop"

$repo = "anandvij03/foldshift"
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host " Foldshift Installer for Windows" -ForegroundColor Cyan
Write-Host " Profile: $Profile | Version: $Version" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

if ($Version -eq "latest") {
    $apiUri = "https://api.github.com/repos/$repo/releases/latest"
    try {
        $releaseInfo = Invoke-RestMethod -Uri $apiUri -Headers @{ "User-Agent" = "FoldShift-Installer" }
        $tag = $releaseInfo.tag_name
        $cleanVer = $tag.TrimStart("v")
    } catch {
        Write-Warning "Could not query GitHub API for latest release ($($_.Exception.Message)). Falling back to v0.1.0."
        $cleanVer = "0.1.0"
        $tag = "v0.1.0"
    }
} else {
    $cleanVer = $Version.TrimStart("v")
    $tag = "v$cleanVer"
}

$zipName = "foldshift-$Profile-$cleanVer-windows-x86_64.zip"
$downloadUrl = "https://github.com/$repo/releases/download/$tag/$zipName"

$tempZip = Join-Path ([System.IO.Path]::GetTempPath()) $zipName

Write-Host "Downloading $zipName..." -ForegroundColor Yellow
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $downloadUrl -OutFile $tempZip -UseBasicParsing
} catch {
    Write-Error "Failed to download $downloadUrl. Ensure the release exists or try a manual download from https://github.com/$repo/releases"
    exit 1
}

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

Write-Host "Extracting to $InstallDir..." -ForegroundColor Yellow
Expand-Archive -Path $tempZip -DestinationPath $InstallDir -Force
Remove-Item -Force $tempZip -ErrorAction SilentlyContinue

# Ensure directory is on User PATH
$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($userPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Green
    [Environment]::SetEnvironmentVariable("PATH", "$userPath;$InstallDir", "User")
    $env:PATH = "$env:PATH;$InstallDir"
}

Write-Host "`nFoldshift installation successful!" -ForegroundColor Green
Write-Host "Run:" -ForegroundColor White
if ($Profile -eq "web") {
    Write-Host "  foldshift-web" -ForegroundColor Cyan
    Write-Host "  Then open http://localhost:3000 in your browser." -ForegroundColor Gray
} else {
    Write-Host "  foldshift --help" -ForegroundColor Cyan
}
