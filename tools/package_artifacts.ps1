param(
    [string]$OutputDir = "$([System.IO.Path]::GetTempPath())foldshift_dist",
    [string]$Version = "0.1.0",
    [string]$Target = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"

Write-Host "===================================================="
Write-Host " Foldshift Artifact Packager & Manifest Generator"
Write-Host " Target: $Target | Version: $Version"
Write-Host " Output: $OutputDir"
Write-Host "===================================================="

if (Test-Path $OutputDir) {
    Remove-Item -Recurse -Force $OutputDir -ErrorAction SilentlyContinue
}
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$env:CARGO_INCREMENTAL = "0"
$repoRoot = (Get-Item $PSScriptRoot).Parent.FullName

function Get-FileSha256 {
    param([string]$FilePath)
    $hash = (Get-FileHash -Path $FilePath -Algorithm SHA256).Hash.ToLower()
    return $hash
}

$profiles = @(
    @{
        Id = "core"
        Name = "Foldshift Core"
        CargoArgs = @("build", "-p", "foldshift-cli", "--release", "--no-default-features", "--locked")
        BinarySource = "target/release/foldshift.exe"
        BinaryTargetName = "foldshift.exe"
        ArchiveName = "foldshift-core-$Version-windows-x86_64.zip"
        Description = "Minimal base CLI without conversion codecs; capability discovery and inspection contracts"
        Capabilities = @("core")
        Notices = "MIT License. Permissive dependencies: clap, anyhow, serde_json, foldshift-core."
    },
    @{
        Id = "images"
        Name = "Foldshift Images"
        CargoArgs = @("build", "-p", "foldshift-cli", "--release", "--features", "image", "--locked")
        BinarySource = "target/release/foldshift.exe"
        BinaryTargetName = "foldshift.exe"
        ArchiveName = "foldshift-images-$Version-windows-x86_64.zip"
        Description = "Core CLI plus still-image inspection, conversion, and resize (PNG, JPEG, WebP, GIF, BMP, TIFF)"
        Capabilities = @("core", "image")
        Notices = "MIT License. Permissive dependencies: image, gif, tiff, png, clap, anyhow, serde_json."
    },
    @{
        Id = "web"
        Name = "Foldshift Web"
        CargoArgs = @("build", "-p", "foldshift-web", "--release", "--locked")
        BinarySource = "target/release/foldshift-web.exe"
        BinaryTargetName = "foldshift-web.exe"
        ArchiveName = "foldshift-web-$Version-windows-x86_64.zip"
        Description = "Local browser UI and HTTP API server with embedded single-page client"
        Capabilities = @("core", "image", "web")
        Notices = "MIT License. Permissive dependencies: axum, tokio, image, gif, tiff, png, serde_json."
    }
)

$artifactRecords = @()

foreach ($prof in $profiles) {
    Write-Host "`nBuilding $($prof.Name) ($($prof.Id))..."
    $p = Start-Process -FilePath "cargo" -ArgumentList $prof.CargoArgs -WorkingDirectory $repoRoot -NoNewWindow -Wait -PassThru
    if ($p.ExitCode -ne 0) {
        throw "Failed to build $($prof.Name) (exit code $($p.ExitCode))"
    }

    $binPath = Join-Path $repoRoot $prof.BinarySource
    if (-not (Test-Path $binPath)) {
        throw "Binary not found at $binPath"
    }

    $stageDir = Join-Path $OutputDir "stage_$($prof.Id)"
    New-Item -ItemType Directory -Force -Path $stageDir | Out-Null

    # Copy binary
    $stagedBin = Join-Path $stageDir $prof.BinaryTargetName
    Copy-Item $binPath $stagedBin

    # Copy LICENSE and README
    $licensePath = Join-Path $repoRoot "LICENSE"
    if (Test-Path $licensePath) { Copy-Item $licensePath (Join-Path $stageDir "LICENSE") }
    $readmePath = Join-Path $repoRoot "README.md"
    if (Test-Path $readmePath) { Copy-Item $readmePath (Join-Path $stageDir "README.md") }

    $binItem = Get-Item $stagedBin
    $binSize = $binItem.Length
    $binSha = Get-FileSha256 $stagedBin

    # Create zip archive
    $zipPath = Join-Path $OutputDir $prof.ArchiveName
    if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
    Compress-Archive -Path "$stageDir/*" -DestinationPath $zipPath -CompressionLevel Optimal

    $zipItem = Get-Item $zipPath
    $zipSize = $zipItem.Length
    $zipSha = Get-FileSha256 $zipPath

    Remove-Item -Recurse -Force $stageDir

    Write-Host "  Binary size:  $binSize bytes ($([math]::Round($binSize / 1MB, 2)) MB)"
    Write-Host "  Archive size: $zipSize bytes ($([math]::Round($zipSize / 1MB, 2)) MB)"
    Write-Host "  Archive SHA:  $zipSha"

    $artifactRecords += [PSCustomObject]@{
        id = $prof.Id
        name = $prof.Name
        description = $prof.Description
        profile = $prof.Id
        capabilities = $prof.Capabilities
        archive_filename = $prof.ArchiveName
        download_bytes = $zipSize
        download_sha256 = $zipSha
        installed_bytes = $binSize
        binary_filename = $prof.BinaryTargetName
        binary_sha256 = $binSha
        notices = $prof.Notices
    }
}

$manifest = [PSCustomObject]@{
    schema_version = "1.0.0"
    foldshift_version = $Version
    generated_at = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
    platform = [PSCustomObject]@{
        os = "windows"
        arch = "x86_64"
        target_triple = $Target
    }
    artifacts = $artifactRecords
}

$manifestJson = $manifest | ConvertTo-Json -Depth 5
$manifestPath = Join-Path $OutputDir "manifest.json"
$manifestJson | Set-Content -Path $manifestPath -Encoding UTF8

$repoDist = Join-Path $repoRoot "dist"
if (-not (Test-Path $repoDist)) { New-Item -ItemType Directory -Force -Path $repoDist | Out-Null }
$manifestJson | Set-Content -Path (Join-Path $repoDist "manifest.json") -Encoding UTF8

Write-Host "`nManifest written to: $manifestPath and $(Join-Path $repoDist 'manifest.json')"

# Markdown download chooser table
Write-Host "`nDownload Chooser Markdown:"
Write-Host "----------------------------------------------------"
$mdChooser = @"
### Selectable Downloads (v$Version - Windows x86_64)

| Selection | Capabilities | Executable Size | Download Size (ZIP) | SHA-256 Checksum |
|---|---|---|---|---|
"@

foreach ($art in $artifactRecords) {
    $instMb = [math]::Round($art.installed_bytes / 1MB, 2)
    $dlMb = [math]::Round($art.download_bytes / 1MB, 2)
    $caps = $art.capabilities -join ", "
    $shortSha = $art.download_sha256.Substring(0, 12) + "..."
    $mdChooser += "`n| **$($art.name)** | ``$caps`` | **$instMb MB** ($($art.installed_bytes) B) | **$dlMb MB** ($($art.download_bytes) B) | ``$($art.download_sha256)`` |"
}

Write-Host $mdChooser
Write-Host "----------------------------------------------------"
