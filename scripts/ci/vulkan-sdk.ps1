# Provides the Vulkan SDK pieces whisper.cpp's Vulkan backend is built with (ADR 0014,
# amendment "Vulkan in Acta.exe"): the headers, the loader's import library and glslc,
# from LunarG's SDK at a pinned version, checked against LunarG's published SHA-256.
# Free; nothing of it ships (Acta.exe does not even import the loader).
#
# The installer is run silently into a temporary folder; only Include, Lib and
# glslc.exe are kept in -Dir, which a workflow may cache (keyed on the version).
# Every later step of the job gets VULKAN_SDK through GITHUB_ENV.
# Usage: scripts/ci/vulkan-sdk.ps1 -Dir <folder>
param(
    [Parameter(Mandatory = $true)][string]$Dir
)
$ErrorActionPreference = 'Stop'

$version = '1.4.350.0'
$sha256 = '855b27ba05d2d8119c5114c5d4ff870ca38f2c632b11e1bb9923b9b7e6ecfe7b'

$glslc = Join-Path $Dir 'Bin\glslc.exe'
if (-not (Test-Path -LiteralPath $glslc)) {
    $file = "vulkansdk-windows-X64-$version.exe"
    $installer = Join-Path $env:RUNNER_TEMP $file
    $full = Join-Path $env:RUNNER_TEMP 'vulkan-sdk-full'
    Write-Output "Downloading $file"
    Invoke-WebRequest -UseBasicParsing -Uri "https://sdk.lunarg.com/sdk/download/$version/windows/$file" -OutFile $installer
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $installer).Hash.ToLowerInvariant()
    if ($actual -ne $sha256) {
        Write-Output "::error::$file has SHA-256 $actual, expected $sha256"
        exit 1
    }
    # Qt Installer Framework, unattended.
    $p = Start-Process -FilePath $installer -Wait -PassThru -NoNewWindow -ArgumentList @(
        '--root', $full, '--accept-licenses', '--default-answer', '--confirm-command', 'install')
    if ($p.ExitCode -ne 0) {
        Write-Output "::error::the Vulkan SDK installer exited with $($p.ExitCode)"
        exit 1
    }
    New-Item -ItemType Directory -Force (Join-Path $Dir 'Bin') | Out-Null
    Copy-Item -Recurse -Force (Join-Path $full 'Include') $Dir
    Copy-Item -Recurse -Force (Join-Path $full 'Lib') $Dir
    Copy-Item -Force (Join-Path $full 'Bin\glslc.exe') $glslc
    Remove-Item -Recurse -Force $full, $installer
}

foreach ($need in 'Include\vulkan\vulkan.hpp', 'Lib\vulkan-1.lib', 'Bin\glslc.exe') {
    if (-not (Test-Path -LiteralPath (Join-Path $Dir $need))) {
        Write-Output "::error::no $need in $Dir"
        exit 1
    }
}
# The whole output is collected before anything is printed: `Select-Object -First`
# in the pipeline stops glslc early, and its exit code is then not 0 although it ran.
$glslcOut = & $glslc --version 2>&1
$code = $LASTEXITCODE
if ($code -ne 0) {
    $glslcOut | ForEach-Object { Write-Output "$_" }
    Write-Output "::error::glslc does not run (exit $code)"
    exit 1
}
Write-Output "$($glslcOut | Select-Object -First 1)"
Write-Output "VULKAN_SDK=$Dir"
"VULKAN_SDK=$Dir" | Out-File -Append -Encoding utf8 $env:GITHUB_ENV
