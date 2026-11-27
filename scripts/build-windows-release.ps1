# Windows Release Build Script
# Builds GCodeKit6 for the MSVC target and creates the MSI installer and the
# portable zip. No GTK runtime or MSYS2 is needed: the egui front end links
# only against system DLLs, and .cargo/config.toml links the C runtime
# statically (+crt-static).
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
Write-Host "Building GCodeKit6 $Version for $Target" -ForegroundColor Cyan

# Step 1: Build release binary
if (-not $SkipBuild) {
    cargo build --release --target $Target
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Build failed" -ForegroundColor Red
        exit 1
    }
}

if (-not (Test-Path "$BinDir\gcodekit6.exe")) {
    Write-Host "Error: gcodekit6.exe not found in $BinDir" -ForegroundColor Red
    exit 1
}

# Step 2: Build the MSI (WiX needs a numeric x.y.z: 1.2.0-beta.2 -> 1.2.0)
$InstallVersion = ($Version -split '[-+]')[0]
$MsiFile = "target\wix\gcodekit6-$Version-x64.msi"
cargo wix --package gcodekit6-design --target $Target --no-build --nocapture `
    --install-version $InstallVersion -o $MsiFile
if ($LASTEXITCODE -ne 0) {
    Write-Host "MSI build failed" -ForegroundColor Red
    exit 1
}

# Step 3: Portable zip (executable + licence)
$Dist = "target\dist\gcodekit6"
New-Item -ItemType Directory -Force -Path $Dist | Out-Null
Copy-Item "$BinDir\gcodekit6.exe" $Dist
Copy-Item "wix\License.rtf" $Dist
$ZipFile = "target\gcodekit6-$Version-windows-x86_64-portable.zip"
Compress-Archive -Path "$Dist\*" -DestinationPath $ZipFile -Force

Write-Host "MSI:      $MsiFile" -ForegroundColor Green
Write-Host "Portable: $ZipFile" -ForegroundColor Green
