# Stage the release binaries and build the Velopack packages for Windows.
# The release workflow calls this script, so CI and local packaging cannot drift.
# The macOS and Linux counterpart is scripts/package.sh.
#
# Usage: scripts\package.ps1 [-Target <triple>] [-BinDir <dir>] [-Version <x.y.z>]
#                            [-OutputDir <dir>] [-FetchPrevious]
#
#   -Target         Rust target triple the binaries were built for; binaries are then
#                   read from target\<triple>\release instead of target\release
#   -BinDir         directory holding Acta.exe and acta-cli.exe (overrides -Target)
#   -Version        default: the workspace version from `cargo metadata`
#   -OutputDir      default: releases\win-x64
#   -FetchPrevious  download the latest published release of the channel first, so that
#                   vpk also emits a delta package (set VPK_TOKEN to avoid the anonymous
#                   GitHub API rate limit)
#
# Requires: the binaries already built (`cargo build --release --locked`), the licence
# notices collected in dist\licenses (`sh scripts/licenses.sh` from Git Bash), .NET SDK >= 8
# and vpk 1.2.161 (`dotnet tool install -g vpk --version 1.2.161`).
# Works in Windows PowerShell 5.1 and PowerShell 7.
[CmdletBinding()]
param(
    [string]$Target,
    [string]$BinDir,
    [string]$Version,
    [string]$OutputDir,
    [switch]$FetchPrevious
)

$ErrorActionPreference = 'Stop'

$PackId = 'Acta'
$Channel = 'win-x64'
$RepoUrl = 'https://github.com/Alegzandr/acta'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $Version) {
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $Version = ($metadata.packages | Where-Object { $_.name -eq 'acta-app' }).version
    if (-not $Version) { throw 'could not read the acta-app version from cargo metadata' }
}
if (-not $BinDir) {
    if ($Target) { $BinDir = "target\$Target\release" } else { $BinDir = 'target\release' }
}
if (-not $OutputDir) { $OutputDir = "releases\$Channel" }

foreach ($bin in 'Acta.exe', 'acta-cli.exe') {
    if (-not (Test-Path (Join-Path $BinDir $bin) -PathType Leaf)) {
        throw "$BinDir\$bin not found; run cargo build --release --locked first"
    }
}
$icon = 'assets\icons\acta.ico'
if (-not (Test-Path $icon -PathType Leaf)) { throw "$icon is missing" }
# Every package carries the third-party notices; a package without them must not exist.
$licenses = 'dist\licenses'
if (-not (Test-Path (Join-Path $licenses 'THIRD-PARTY-RUST.txt') -PathType Leaf)) {
    throw "$licenses is missing or incomplete; run scripts/licenses.sh first (Git Bash)"
}
if (-not (Get-Command vpk -ErrorAction SilentlyContinue)) {
    throw 'vpk not found; run: dotnet tool install -g vpk --version 1.2.161'
}
# A vpk that cannot start (no .NET 10 runtime where its launcher looks: set DOTNET_ROOT)
# must stop here, not be mistaken below for "no previous release".
vpk --help | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'vpk is installed but does not start; see its message above' }

Write-Output "Packaging $PackId $Version for $Channel from $BinDir into $OutputDir"

$stage = "dist\stage\$Channel"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage, $OutputDir | Out-Null
Copy-Item (Join-Path $BinDir 'Acta.exe'), (Join-Path $BinDir 'acta-cli.exe') $stage
Copy-Item $licenses (Join-Path $stage 'licenses') -Recurse

if ($FetchPrevious) {
    # Fails when the channel has no published release yet: then there is simply no delta.
    vpk download github --repoUrl $RepoUrl --channel $Channel --outputDir $OutputDir
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "no previous $Channel release downloaded; no delta package will be built"
    }
}
# vpk refuses to pack a version that is already in the output directory (a local re-run,
# or a dry run of the version that is currently published).
foreach ($kind in 'full', 'delta') {
    $stale = Join-Path $OutputDir "$PackId-$Version-$Channel-$kind.nupkg"
    if (Test-Path $stale) {
        Write-Warning "removing existing $stale; no delta against the same version"
        Remove-Item $stale -Force
    }
}

vpk pack --packId $PackId --packTitle Acta --packVersion $Version `
    --packDir $stage --mainExe Acta.exe --icon $icon `
    --runtime $Channel --channel $Channel --outputDir $OutputDir
if ($LASTEXITCODE -ne 0) { throw 'vpk pack failed' }

foreach ($expected in "$PackId-$Channel-Setup.exe", "$PackId-$Version-$Channel-full.nupkg", "assets.$Channel.json") {
    if (-not (Test-Path (Join-Path $OutputDir $expected) -PathType Leaf)) {
        throw "vpk did not produce $expected"
    }
}

if ($FetchPrevious) {
    # Keep only what this build produced: the fetched packages are already published.
    Get-ChildItem $OutputDir -Filter *.nupkg |
        Where-Object { $_.Name -notlike "$PackId-$Version-$Channel-*" } |
        Remove-Item -Force
}

Write-Output "Packages in ${OutputDir}:"
Get-ChildItem $OutputDir | Select-Object Name, Length | Format-Table -AutoSize
