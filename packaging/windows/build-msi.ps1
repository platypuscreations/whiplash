# Builds the Windows installer: target\release\bundle\whiplash-<version>-x86_64.msi
# (under CARGO_TARGET_DIR instead of target, if that's set).
#
# Needs: Rust (x86_64-pc-windows-msvc), Visual Studio 2022 or its Build
# Tools with the C++ workload, CMake, LLVM (for projectM's bindings), vcpkg
# with VCPKG_INSTALLATION_ROOT set to it, and WiX 5:
#   dotnet tool install -g wix --version 5.*
# (WiX 6 and later ask you to accept a license first; this uses 5.)
#
# Any arguments are passed to `cargo build`, for example
# `--no-default-features` to leave out MilkDrop.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..\..')

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
$bundle = Join-Path $target 'release\bundle'
$msi = Join-Path $bundle "whiplash-$version-x86_64.msi"

Write-Host "Building Whiplash $version..."
cargo build --release @args
if ($LASTEXITCODE) { exit $LASTEXITCODE }

# Windows Installer versions are plain numbers: 0.4.0 for a release
# candidate 0.4.0-rc1 (the file name keeps the full version).
Write-Host "Building $msi..."
New-Item -ItemType Directory -Force $bundle | Out-Null
wix build packaging\windows\whiplash.wxs -arch x64 `
    -d "Version=$($version -replace '-.*')" -d "Exe=$(Join-Path $target 'release\whiplash.exe')" `
    -o $msi
if ($LASTEXITCODE) { exit $LASTEXITCODE }
Write-Host "Wrote $msi"
