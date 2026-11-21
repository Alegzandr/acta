# Selects a native Perl for the vendored OpenSSL build on a Windows runner (ADR 0003).
# The bash steps of a workflow put Git's MSYS Perl first on PATH, and its ./Configure
# fails with "Can't locate Locale/Maketext/Simple.pm". GitHub's Windows images install
# Strawberry Perl through Chocolatey (toolset strawberryperl), in C:\Strawberry.
# openssl-src runs the program named by OPENSSL_SRC_PERL, so every later step of the
# job gets that variable through GITHUB_ENV.
$ErrorActionPreference = 'Stop'

$perl = 'C:\Strawberry\perl\bin\perl.exe'
if (-not (Test-Path -LiteralPath $perl)) {
    Write-Output "::error::no Strawberry Perl at $perl; the vendored OpenSSL build needs a native Perl (ADR 0003)"
    exit 1
}
# The module that Git's Perl lacks, and the core modules ./Configure loads.
# No double quotes in -e: Windows PowerShell and pwsh pass them to native programs
# differently.
& $perl -MLocale::Maketext::Simple -MFile::Spec -MIPC::Cmd -e 'print $^V, chr 10'
if ($LASTEXITCODE -ne 0) {
    Write-Output "::error::$perl cannot load the modules OpenSSL's Configure needs"
    exit 1
}
Write-Output "OPENSSL_SRC_PERL=$perl"
"OPENSSL_SRC_PERL=$perl" | Out-File -Append -Encoding utf8 $env:GITHUB_ENV
