param(
    [string]$OutputDir = "$([System.IO.Path]::GetTempPath())foldshift_bench_fixtures"
)

$ErrorActionPreference = "Stop"

$bin = Resolve-Path "target/release/foldshift.exe" -ErrorAction SilentlyContinue
if (-not $bin) {
    Write-Host "Building release binary..."
    $env:CARGO_INCREMENTAL = "0"
    cargo build --release --bin foldshift --locked
    $bin = Resolve-Path "target/release/foldshift.exe"
}

Write-Host "Foldshift binary: $bin"

# 1. Measure binary sizes
$exeSize = (Get-Item $bin).Length
Write-Host "Executable size: $exeSize bytes ($([math]::Round($exeSize / 1MB, 2)) MB)"

# 2. Setup fixtures
if (Test-Path $OutputDir) { Remove-Item -Recurse -Force $OutputDir -ErrorAction SilentlyContinue }
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$env:CARGO_INCREMENTAL = "0"
cargo run --bin gen_fixtures --release --quiet -- $OutputDir

Write-Host "Fixtures generated:"
Get-ChildItem $OutputDir -Filter "*.*" | Where-Object { $_.Extension -ne ".exe" -and $_.Extension -ne ".rs" } | ForEach-Object {
    Write-Host "  $($_.Name): $($_.Length) bytes"
}

function Measure-ProcessStats {
    param([string]$FilePath, [string[]]$Arguments)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath
    $psi.Arguments = $Arguments -join " "
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $p = [System.Diagnostics.Process]::Start($psi)
    $peakMemory = 0
    while (-not $p.HasExited) {
        $p.Refresh()
        $ws = $p.WorkingSet64
        if ($ws -gt $peakMemory) { $peakMemory = $ws }
        Start-Sleep -Milliseconds 2
    }
    $sw.Stop()
    $stdout = $p.StandardOutput.ReadToEnd()
    $stderr = $p.StandardError.ReadToEnd()
    
    if ($p.PeakWorkingSet64 -gt $peakMemory) { $peakMemory = $p.PeakWorkingSet64 }

    return [PSCustomObject]@{
        ElapsedMs = [math]::Round($sw.Elapsed.TotalMilliseconds, 2)
        PeakMemoryMB = [math]::Round($peakMemory / 1MB, 2)
        ExitCode = $p.ExitCode
        StdOut = $stdout
        StdErr = $stderr
    }
}

Write-Host "`nMeasuring baselines..."

# 3. Startup benchmarks
$startupTimes = @()
for ($i=0; $i -lt 5; $i++) {
    $res = Measure-ProcessStats -FilePath $bin -Arguments @("--version")
    $startupTimes += $res.ElapsedMs
}
$avgStartup = [math]::Round(($startupTimes | Measure-Object -Average).Average, 2)
Write-Host "Startup (--version): avg $avgStartup ms"

# 4. Capabilities benchmark
$resCap = Measure-ProcessStats -FilePath $bin -Arguments @("capabilities", "--json")
Write-Host "Capabilities: $($resCap.ElapsedMs) ms, Peak Mem: $($resCap.PeakMemoryMB) MB"

# 5. Inspect benchmarks
$resInspSmall = Measure-ProcessStats -FilePath $bin -Arguments @("inspect", "$OutputDir/small_100x100.png", "--json")
Write-Host "Inspect Small (100x100): $($resInspSmall.ElapsedMs) ms, Peak Mem: $($resInspSmall.PeakMemoryMB) MB"

$resInspMed = Measure-ProcessStats -FilePath $bin -Arguments @("inspect", "$OutputDir/medium_1920x1080.jpg", "--json")
Write-Host "Inspect Medium (1920x1080): $($resInspMed.ElapsedMs) ms, Peak Mem: $($resInspMed.PeakMemoryMB) MB"

$resInspLarge = Measure-ProcessStats -FilePath $bin -Arguments @("inspect", "$OutputDir/large_4000x3000_orient6.jpg", "--json")
Write-Host "Inspect Large (4000x3000): $($resInspLarge.ElapsedMs) ms, Peak Mem: $($resInspLarge.PeakMemoryMB) MB"

# 6. Convert benchmarks
$resConvSmall = Measure-ProcessStats -FilePath $bin -Arguments @("convert", "$OutputDir/small_100x100.png", "$OutputDir/small.jpg", "--force")
Write-Host "Convert Small PNG->JPG: $($resConvSmall.ElapsedMs) ms, Peak Mem: $($resConvSmall.PeakMemoryMB) MB"

$resConvMed = Measure-ProcessStats -FilePath $bin -Arguments @("convert", "$OutputDir/medium_1920x1080.jpg", "$OutputDir/medium.webp", "--force")
Write-Host "Convert Medium JPG->WebP: $($resConvMed.ElapsedMs) ms, Peak Mem: $($resConvMed.PeakMemoryMB) MB"

$resConvLarge = Measure-ProcessStats -FilePath $bin -Arguments @("convert", "$OutputDir/large_4000x3000_orient6.jpg", "$OutputDir/large_upright.png", "--force")
Write-Host "Convert Large 12MP JPG->PNG: $($resConvLarge.ElapsedMs) ms, Peak Mem: $($resConvLarge.PeakMemoryMB) MB"

# 7. Resize benchmark
$resResize = Measure-ProcessStats -FilePath $bin -Arguments @("image", "resize", "$OutputDir/large_4000x3000_orient6.jpg", "$OutputDir/resized_1920.webp", "--width", "1920", "--force")
Write-Host "Resize 12MP to 1920px WebP: $($resResize.ElapsedMs) ms, Peak Mem: $($resResize.PeakMemoryMB) MB"

# Clean up temporary fixtures
Remove-Item -Recurse -Force $OutputDir -ErrorAction SilentlyContinue

Write-Host "`nMeasurements complete!"
