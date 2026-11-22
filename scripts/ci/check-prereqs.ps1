# Weekly drift check of ADR 0013's command tables, read-only: nothing is installed.
#  1. Each winget id Acta installs with still resolves, with the flags Acta passes
#     (`winget show`, exact id, winget source, user scope).
#  2. Claude Code's setup page still documents the two native-installer one-liners
#     Acta runs.
# The ids, the one-liners and the page are read from
# crates/acta-core/src/prereqs/commands.rs, where Acta takes them from. A failure fails
# the job for the lead to read; it opens no issue. Writes a table to the job summary
# when GITHUB_STEP_SUMMARY is set. Windows only (winget).
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path (Split-Path $PSScriptRoot)
$source = Get-Content -Raw (Join-Path $root 'crates/acta-core/src/prereqs/commands.rs')
$names = 'WINGET_CLAUDE', 'WINGET_GIT', 'WINGET_GH', 'CLAUDE_INSTALL_SH', 'CLAUDE_INSTALL_PS1', 'PAGE_CLAUDE'
$values = @{}
foreach ($name in $names) {
    $match = [regex]::Match($source, "pub const ${name}: &str = `"([^`"]+)`";")
    if (-not $match.Success) { throw "could not read $name in commands.rs" }
    $values[$name] = $match.Groups[1].Value
}

$rows = [System.Collections.Generic.List[string]]::new()
$failed = $false

# winget ships with Windows Server 2025 but is registered for a user only at their
# first sign-in; a runner may need that registration (Microsoft's documented step).
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Output 'winget not found; registering App Installer for this user'
    Add-AppxPackage -RegisterByFamilyName -MainPackage Microsoft.DesktopAppInstaller_8wekyb3d8bbwe
    $env:PATH = "$env:LOCALAPPDATA\Microsoft\WindowsApps;$env:PATH"
}
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    # Microsoft's own bootstrap module, pinned; it installs winget on this runner only.
    Write-Output 'winget still not found; bootstrapping it with Microsoft.WinGet.Client'
    Install-PackageProvider -Name NuGet -Force | Out-Null
    Install-Module -Name Microsoft.WinGet.Client -RequiredVersion 1.29.380 -Force -Repository PSGallery
    Repair-WinGetPackageManager -AllUsers -Force
    $env:PATH = "$env:LOCALAPPDATA\Microsoft\WindowsApps;$env:PATH"
}
$version = winget --version
Write-Output "winget $version"

foreach ($name in 'WINGET_CLAUDE', 'WINGET_GIT', 'WINGET_GH') {
    $id = $values[$name]
    $output = winget show --id $id -e --source winget --scope user --accept-source-agreements --disable-interactivity 2>&1
    $code = $LASTEXITCODE
    if ($code -eq 0) {
        $output | Select-String -Pattern '^(Found|\s*Version:|\s*Installer (Type|Url):)' |
            ForEach-Object { $_.Line } | Write-Output
        $found = ($output | Select-String -Pattern '^\s*Version:\s*(\S+)' | Select-Object -First 1)
        $type = ($output | Select-String -Pattern '^\s*Installer Type:\s*(\S+)' | Select-Object -First 1)
        $detail = 'resolves'
        if ($found) { $detail += ", version $($found.Matches[0].Groups[1].Value)" }
        if ($type) { $detail += ", installer $($type.Matches[0].Groups[1].Value)" }
        $rows.Add("| winget ``$id`` (user scope) | ok: $detail |")
    } else {
        $output | Out-String | Write-Output
        $rows.Add("| winget ``$id`` (user scope) | failed: exit code $code |")
        Write-Output "::error::winget show --id $id --scope user failed with exit code $code; check ADR 0013 and commands.rs"
        $failed = $true
    }
}

$page = $values['PAGE_CLAUDE']
try {
    $html = (Invoke-WebRequest -Uri $page -UseBasicParsing -MaximumRedirection 5).Content
} catch {
    Write-Output "::error::could not read ${page}: $($_.Exception.Message)"
    $html = ''
    $failed = $true
}
# Code blocks are highlighted token by token: compare the text without the tags.
$text = [System.Net.WebUtility]::HtmlDecode(($html -replace '<[^>]*>', ''))
foreach ($name in 'CLAUDE_INSTALL_SH', 'CLAUDE_INSTALL_PS1') {
    $command = $values[$name]
    # A pipe inside a Markdown table cell must be escaped.
    $cell = $command -replace '\|', '\|'
    if ($html -and $text.Contains($command)) {
        $rows.Add("| ``$cell`` on $page | ok |")
    } else {
        $rows.Add("| ``$cell`` on $page | not found |")
        Write-Output "::error::$page no longer shows ``$command``; check ADR 0013 and commands.rs"
        $failed = $true
    }
}

$table = @('### Claude Code prerequisites (ADR 0013)', '', '| Check | Result |', '|-------|--------|') + $rows
$table | Write-Output
if ($env:GITHUB_STEP_SUMMARY) {
    $table | Out-File -Append -Encoding utf8 $env:GITHUB_STEP_SUMMARY
}
if ($failed) { exit 1 }
