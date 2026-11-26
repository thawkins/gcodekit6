# Windows Release Build Script
# Builds GCodeKit5 for the MSVC target and creates the MSI installer and the
# portable zip. No GTK runtime or MSYS2 is needed: the egui front end links
# only against system DLLs.
#
# Requires: Rust (MSVC toolchain), WiX Toolset v3, cargo-wix
#   (cargo install cargo-wix)

param(
    [switch]$SkipBuild = $false
)

$ErrorActionPreference = "Stop"

$Target = "x86_64-pc-windows-msvc"
$BinDir = "target\$Target\release"

# Get version from Cargo.toml
$CargoToml = Get-Content "Cargo.toml" -Raw
if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    Write-Host "Error: Could not determine version from Cargo.toml" -ForegroundColor Red
    exit 1
}
Write-Host "Building GCodeKit5 $Version for $Target" -ForegroundColor Cyan

# Step 1: Build release binary
if (-not $SkipBuild) {
    cargo build --release --target $Target
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Build failed" -ForegroundColor Red
        exit 1
    }
}

if (-not (Test-Path "$BinDir\gcodekit5.exe")) {
    Write-Host "Error: gcodekit5.exe not found in $BinDir" -ForegroundColor Red
    exit 1
}

# Step 2: Build the MSI
$MsiFile = "target\wix\gcodekit5-$Version-x64.msi"
cargo wix --package gcodekit5-design --target $Target --no-build --nocapture -o $MsiFile
if ($LASTEXITCODE -ne 0) {
    Write-Host "MSI build failed" -ForegroundColor Red
    exit 1
}

# Step 3: Portable zip (executable + licence)
$Dist = "target\dist\gcodekit5"
New-Item -ItemType Directory -Force -Path $Dist | Out-Null
Copy-Item "$BinDir\gcodekit5.exe" $Dist
Copy-Item "wix\License.rtf" $Dist
$ZipFile = "target\gcodekit5-$Version-windows-x86_64-portable.zip"
Compress-Archive -Path "$Dist\*" -DestinationPath $ZipFile -Force

Write-Host "MSI:      $MsiFile" -ForegroundColor Green
Write-Host "Portable: $ZipFile" -ForegroundColor Green
