# Points bindgen at LLVM's libclang on a Windows runner (ADR 0014, amendment).
# whisper-rs-sys generates its bindings at build time: the bindings it bundles are
# laid out for an 8-byte C long and do not compile with MSVC. GitHub's Windows
# images install LLVM in C:\Program Files\LLVM. Every later step of the job gets
# LIBCLANG_PATH through GITHUB_ENV.
$ErrorActionPreference = 'Stop'

$dir = 'C:\Program Files\LLVM\bin'
if (-not (Test-Path -LiteralPath (Join-Path $dir 'libclang.dll'))) {
    Write-Output "::error::no libclang.dll in $dir; bindgen needs LLVM to build whisper.cpp's bindings (ADR 0014)"
    exit 1
}
Write-Output "LIBCLANG_PATH=$dir"
"LIBCLANG_PATH=$dir" | Out-File -Append -Encoding utf8 $env:GITHUB_ENV
