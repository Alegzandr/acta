# Proves that Acta.exe starts on a processor without AVX2 (ADR 0014, amendment):
# whisper.cpp is compiled in with AVX2, FMA and F16C, and only the dictation worker
# may run that code. Intel SDE emulates a Sandy Bridge processor (AVX, no AVX2) and
# runs the executable through its C runtime start, static initialisers and the
# start of `main`: the `--voice-worker` mode with its input closed exits at once
# with 0 without calling into whisper.cpp. An AVX2 instruction anywhere on that
# path stops the emulated run with an illegal-instruction error. whisper.cpp's Vulkan
# backend is linked too (ADR 0014, amendment "Vulkan in Acta.exe"): the executable
# imports no Vulkan loader and that path calls no Vulkan function, so the run also
# proves a start without the GPU code (the runner has no graphics card).
# Usage: windows-sde-start.ps1 -Exe <path to Acta.exe>   (SDE_PATH from setup-sde)
param([Parameter(Mandatory = $true)][string]$Exe)
$ErrorActionPreference = 'Stop'

$sde = Get-ChildItem -LiteralPath $env:SDE_PATH -Recurse -Filter 'sde.exe' | Select-Object -First 1
if (-not $sde) {
    Write-Output "::error::no sde.exe under $env:SDE_PATH"
    exit 1
}
# Nothing on its input: the worker reads the end of the pipe and exits.
$out = $null | & $sde.FullName -snb -- $Exe --voice-worker 2>&1
$code = $LASTEXITCODE
$out | ForEach-Object { Write-Output $_ }
if ($code -ne 0) {
    Write-Output "::error::Acta.exe did not start on an emulated processor without AVX2 (exit $code)"
    exit 1
}
Write-Output 'Acta.exe starts without AVX2'
