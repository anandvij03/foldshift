param()

$ErrorActionPreference = "Stop"

Write-Host "===================================================="
Write-Host " Foldshift Dependency & Profile Minimality Check"
Write-Host "===================================================="

# 1. Check runtime dependency tree for Core-only profile
Write-Host "`nInspecting Core-only runtime dependency tree..."
$treeOutput = cargo tree -p foldshift-cli --no-default-features --prefix none --edges normal
$treeLines = $treeOutput -split "`r?`n" | Where-Object { $_.Trim() -ne "" } | ForEach-Object {
    ($_ -split " v\d+")[0].Trim()
} | Select-Object -Unique

Write-Host "Unique runtime crates in Core-only profile:"
$treeLines | ForEach-Object { Write-Host "  - $_" }

$forbiddenCodecs = @("image", "gif", "png", "tiff", "zune-jpeg", "zune-core", "moxcms", "image-webp", "bytemuck", "foldshift-image")
$foundForbidden = @()

foreach ($forbidden in $forbiddenCodecs) {
    if ($treeLines -contains $forbidden) {
        $foundForbidden += $forbidden
    }
}

if ($foundForbidden.Count -gt 0) {
    throw "Minimality check FAILED: Core profile contains forbidden image crates: $($foundForbidden -join ', ')"
}

Write-Host "`n[PASS] Core-only profile is 100% free of image codecs."

# 2. Check that Images profile includes foldshift-image
Write-Host "`nInspecting Images runtime dependency tree..."
$imgTree = (cargo tree -p foldshift-cli --features image --prefix none --edges normal) -join "`n"
if ($imgTree -notmatch "foldshift-image") {
    throw "Images profile check FAILED: foldshift-image crate not found in dependency tree"
}
Write-Host "[PASS] Images profile successfully links foldshift-image."

# 3. Verify dist/manifest.json integrity
$repoRoot = (Get-Item $PSScriptRoot).Parent.FullName
$manifestPath = Join-Path $repoRoot "dist\manifest.json"
if (-not (Test-Path $manifestPath)) {
    throw "Distribution manifest not found at $manifestPath"
}

$manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json
if ($manifest.schema_version -ne "1.0.0") {
    throw "Unexpected manifest schema version: $($manifest.schema_version)"
}
if ($manifest.artifacts.Count -ne 3) {
    throw "Expected 3 artifacts in manifest, found $($manifest.artifacts.Count)"
}

Write-Host "`n[PASS] Distribution manifest validated with $($manifest.artifacts.Count) artifacts."

# 4. Confirm build/test profile separation
Write-Host "`nVerifying CI checks locally..."
$env:CARGO_INCREMENTAL = "0"
Write-Host "  - Running cargo test (Core-only)..."
cargo test -p foldshift-cli --no-default-features --quiet
Write-Host "  - Running cargo test (Images / Workspace)..."
cargo test --workspace --quiet

Write-Host "`n===================================================="
Write-Host " All minimality and profile checks PASSED!"
Write-Host "===================================================="
