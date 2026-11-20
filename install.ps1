# Acta installer for Windows: silent, per-user, no administrator rights.
#
#   irm https://github.com/Alegzandr/acta/releases/latest/download/install.ps1 | iex
#
# A specific version: set $env:ACTA_VERSION = '0.1.0' first.
# Do not start Acta after installing: set $env:ACTA_NO_LAUNCH = '1' first.
#
# It downloads Acta-win-x64-Setup.exe from the GitHub release, checks its SHA-256
# against the release's SHA256SUMS, runs it with --silent and starts Acta. Acta then
# lives in %LocalAppData%\Acta and keeps itself up to date.
# Works in Windows PowerShell 5.1 and PowerShell 7. Never calls `exit`, so it is safe
# to pipe into iex.

function Install-Acta {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'

    $repo = 'https://github.com/Alegzandr/acta'
    $asset = 'Acta-win-x64-Setup.exe'

    if ($env:OS -ne 'Windows_NT') {
        throw 'install.ps1 is for Windows; on macOS and Linux use install.sh'
    }
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    if ($arch -ne 'AMD64' -and $arch -ne 'ARM64') {
        throw "Acta needs 64-bit Windows (found $arch)"
    }
    # Windows on ARM runs the x64 build under emulation; there is no native build yet.

    # Windows PowerShell 5.1 does not always enable TLS 1.2 by default.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $version = $env:ACTA_VERSION
    if ($version) {
        $version = $version -replace '^v', ''
    } else {
        # Resolve "latest" once through the redirect of /releases/latest (no API call, so
        # no rate limit), then download everything from that one release.
        $request = [Net.HttpWebRequest]::Create("$repo/releases/latest")
        $request.Method = 'HEAD'
        $request.AllowAutoRedirect = $false
        $request.UserAgent = 'acta-install'
        try {
            $response = $request.GetResponse()
        } catch {
            throw "could not reach $repo/releases/latest: $($_.Exception.Message)"
        }
        $location = $response.Headers['Location']
        $response.Close()
        if ($location -notmatch '/releases/tag/v([^/]+)$') {
            throw "no published Acta release found at $repo/releases"
        }
        $version = $Matches[1]
    }
    $base = "$repo/releases/download/v$version"

    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("acta-install-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        $setup = Join-Path $tmp $asset
        $sums = Join-Path $tmp 'SHA256SUMS'
        Write-Host "Downloading Acta $version..."
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $sums
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile $setup
        } catch {
            throw "download from $base failed: $($_.Exception.Message)"
        }

        $expected = $null
        foreach ($line in Get-Content $sums) {
            if ($line -match '^([0-9a-fA-F]{64})\s+\*?(.+)$' -and $Matches[2].Trim() -eq $asset) {
                $expected = $Matches[1]
            }
        }
        if (-not $expected) { throw "SHA256SUMS of Acta $version has no entry for $asset" }
        $actual = (Get-FileHash -Algorithm SHA256 -Path $setup).Hash
        if ($actual -ne $expected) {
            throw "checksum mismatch for $asset (expected $expected, got $actual); nothing was installed"
        }

        Write-Host 'Installing...'
        $process = Start-Process -FilePath $setup -ArgumentList '--silent' -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "the installer exited with code $($process.ExitCode); see $env:LOCALAPPDATA\velopack\velopack_Acta.log"
        }
    } finally {
        Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }

    $exe = Join-Path $env:LOCALAPPDATA 'Acta\current\Acta.exe'
    if (-not (Test-Path $exe)) { throw "the installer finished but $exe is missing" }
    Write-Host "Acta $version is installed in $env:LOCALAPPDATA\Acta."
    if ($env:ACTA_NO_LAUNCH -ne '1') {
        Start-Process -FilePath $exe
    }
    Write-Host 'Open a new terminal to use the acta command.'
}

Install-Acta
