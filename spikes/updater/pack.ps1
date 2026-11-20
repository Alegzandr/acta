# Build and package one version of the spike. Usage: .\pack.ps1 0.0.1
# Requires: cargo, dotnet SDK >= 8, vpk 1.2.161 (dotnet tool install -g vpk --version 1.2.161)
param([Parameter(Mandatory)][string]$Version)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$toml = (Get-Content Cargo.toml) -replace '^version = ".*"$', "version = `"$Version`""
[IO.File]::WriteAllLines("$PSScriptRoot\Cargo.toml", $toml, (New-Object Text.UTF8Encoding $false))
cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

if (Test-Path stage) { Remove-Item stage -Recurse -Force }
New-Item -ItemType Directory stage | Out-Null
Copy-Item target\release\Acta.exe, target\release\acta-cli.exe stage\

# Same directory for every version: vpk finds the previous full package there
# and emits a delta. In CI this directory is filled by `vpk download github`.
vpk pack --packId ActaSpike --packTitle 'Acta Spike' --packVersion $Version `
    --packDir stage --mainExe Acta.exe --runtime win-x64 --channel win-x64 --outputDir releases
if ($LASTEXITCODE -ne 0) { throw 'vpk pack failed' }
Get-ChildItem releases | Select-Object Name, Length
