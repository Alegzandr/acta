# End-to-end test of install, self-update and uninstall on Windows, run by
# .github/workflows/update-e2e.yml on a disposable runner. It installs Acta for the
# current user from the published GitHub releases, starts it, and uninstalls it:
# never run it on a machine that is in use.
#
#   scripts\ci\update-e2e.ps1 -Phase install   -Version 0.1.0   install.ps1 of that release
#   scripts\ci\update-e2e.ps1 -Phase verify    -Version 0.1.0   package, CLI, user PATH
#   scripts\ci\update-e2e.ps1 -Phase update    -From 0.1.0 -To 0.1.1
#   scripts\ci\update-e2e.ps1 -Phase uninstall
#   scripts\ci\update-e2e.ps1 -Phase collect   -Dir <dir>       copy the logs for an artifact
#   scripts\ci\update-e2e.ps1 -Phase cleanup                    stop every Acta process left
#
# Versions are given without the leading v. Paths are the ones of ADR 0004.
# Runs in Windows PowerShell 5.1, the shell a user pipes install.ps1 into.
[Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSAvoidUsingInvokeExpression', '',
    Justification = 'The documented install one-liner is irm | iex; the test runs exactly that.')]
param(
    [Parameter(Mandatory)]
    [ValidateSet('install', 'verify', 'update', 'uninstall', 'collect', 'cleanup')]
    [string] $Phase,
    [string] $Version,
    [string] $From,
    [string] $To,
    [string] $Dir,
    # Bound on the whole update phase, every launch included.
    [int] $UpdateTimeout = 180
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'https://github.com/Alegzandr/acta'
$Root = Join-Path $env:LOCALAPPDATA 'Acta'
$App = Join-Path $Root 'current\Acta.exe'
$CliRoot = Join-Path $env:LOCALAPPDATA 'ActaCli'
$CliDir = Join-Path $CliRoot 'bin'
$Cli = Join-Path $CliDir 'acta.exe'
$DataDir = Join-Path $env:LOCALAPPDATA 'ActaData'
$Log = Join-Path $DataDir 'logs\acta.log'
$VelopackLog = Join-Path $env:LOCALAPPDATA 'velopack\velopack_Acta.log'
$UninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Acta'

function Exit-Test([string] $Message) {
    Write-Output "::error::$Message"
    if (Test-Path $Log) {
        Write-Output "--- $Log ---"
        Get-Content $Log | Write-Output
    }
    exit 1
}

# Exact comparison; -IgnoreCase for paths, which Windows compares without case.
function Assert-Equal($Actual, $Expected, [string] $What, [switch] $IgnoreCase) {
    $same = if ($IgnoreCase) { "$Actual" -eq "$Expected" } else { "$Actual" -ceq "$Expected" }
    if (-not $same) { Exit-Test "${What}: expected '$Expected', got '$Actual'" }
    Write-Output "ok: $What is '$Expected'"
}

function Get-LogLine {
    if (Test-Path $Log) { @(Get-Content $Log) } else { @() }
}

# Lines of the app log from line $First on (1-based) written by Acta $LogVersion whose
# message contains $Text, as objects with Line and ProcessId. A line is
# "<ms> [<version> pid <pid>] <message>" (crates/acta-app/src/log.rs).
function Find-LogLine([int] $First, [string] $LogVersion, [string] $Text) {
    $lines = Get-LogLine
    for ($i = [Math]::Max($First, 1); $i -le $lines.Count; $i++) {
        if ($lines[$i - 1] -match '^\d+ \[(\S+) pid (\d+)\] (.*)$' -and
            $Matches[1] -eq $LogVersion -and $Matches[3].Contains($Text)) {
            [pscustomobject]@{ Line = $i; ProcessId = [int]$Matches[2] }
        }
    }
}

function Test-InstalledApp([int] $ProcessId) {
    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    $process -and $process.Path -and ($process.Path -eq $App)
}

function Get-ManifestVersion([string] $Path) {
    if ((Get-Content -Raw $Path) -match '<version>([^<]+)</version>') { $Matches[1] }
}

function Get-UserPathEntry {
    $value = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($value) { $value.Split(';') | Where-Object { $_ } } else { @() }
}

# The Start menu shortcut the installer created, or $null.
function Find-ActaShortcut {
    $programs = [Environment]::GetFolderPath('Programs')
    $shell = New-Object -ComObject WScript.Shell
    # Velopack points shortcuts at current\Acta.exe or at the stub in the root.
    $targets = $App, (Join-Path $Root 'Acta.exe')
    Get-ChildItem -Path $programs -Filter '*.lnk' -Recurse |
        Where-Object { $targets -contains $shell.CreateShortcut($_.FullName).TargetPath } |
        Select-Object -First 1
}

# What a user starts: the Start menu shortcut. Without one the install is broken.
function Open-Acta {
    $shortcut = Find-ActaShortcut
    if (-not $shortcut) { Exit-Test "no Start menu shortcut to $App" }
    Write-Output "Starting $($shortcut.FullName)"
    Start-Process -FilePath $shortcut.FullName
}

# Leftovers of a run, so that nothing of Acta outlives the job.
function Invoke-Cleanup {
    Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and ($_.Path.StartsWith("$Root\") -or $_.Path.StartsWith("$CliRoot\")) } |
        ForEach-Object {
            Write-Output "Stopping $($_.Id) $($_.Path)"
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        }
}

function Close-Acta([int] $ProcessId) {
    Stop-Process -Id $ProcessId -ErrorAction SilentlyContinue
    Wait-Process -Id $ProcessId -Timeout 15 -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}

function Invoke-Install([string] $Version) {
    $url = "$Repo/releases/download/v$Version/install.ps1"
    Write-Output "Installing Acta $Version with $url"
    # The documented one-liner, pinned to that release. ACTA_NO_LAUNCH: the test starts
    # the app itself, from the shortcut, in the update phase.
    $env:ACTA_VERSION = $Version
    $env:ACTA_NO_LAUNCH = '1'
    try {
        Invoke-RestMethod -UseBasicParsing -Uri $url | Invoke-Expression
    } catch {
        Exit-Test "install.ps1 of $Version failed: $($_.Exception.Message)"
    } finally {
        Remove-Item Env:ACTA_VERSION, Env:ACTA_NO_LAUNCH -ErrorAction SilentlyContinue
    }
    if (-not (Test-Path $App)) { Exit-Test "install.ps1 finished but $App is missing" }
}

function Invoke-Verify([string] $Version) {
    # The package: what Velopack and Windows think is installed.
    if (-not (Test-Path $App)) { Exit-Test "$App is missing" }
    Assert-Equal (Get-ManifestVersion (Join-Path $Root 'current\sq.version')) $Version 'version in current\sq.version'
    Assert-Equal (Get-ItemProperty $UninstallKey).DisplayVersion $Version 'DisplayVersion of the uninstall entry'
    Assert-Equal (& (Join-Path $Root 'current\acta-cli.exe') --version) "acta $Version" 'packaged acta-cli.exe'
    $licenses = Join-Path $Root 'current\licenses'
    if (-not (Test-Path $licenses)) { Exit-Test "$licenses is missing" }
    Write-Output 'ok: licenses directory next to Acta.exe'
    # Checked before and after the update: an update must keep the shortcut working.
    $shortcut = Find-ActaShortcut
    if (-not $shortcut) { Exit-Test "no Start menu shortcut to $App" }
    Write-Output "ok: Start menu shortcut $($shortcut.FullName)"

    # The CLI exported outside the Velopack root by the install or update hook.
    if (-not (Test-Path $Cli)) { Exit-Test "the exported CLI $Cli is missing" }
    Assert-Equal (& $Cli --version) "acta $Version" 'exported CLI'
    Assert-Equal (Get-Content -Raw (Join-Path $CliDir 'version.txt')).Trim() $Version 'export marker'

    # The user PATH, as a new terminal sees it.
    $entries = @(Get-UserPathEntry | Where-Object { $_.TrimEnd('\') -eq $CliDir })
    Assert-Equal $entries.Count 1 "occurrences of $CliDir in HKCU\Environment\Path"
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
        [Environment]::GetEnvironmentVariable('Path', 'User')
    $found = (Get-Command acta -CommandType Application | Select-Object -First 1).Source
    Assert-Equal $found $Cli 'acta found on PATH' -IgnoreCase
    Assert-Equal (& acta --version) "acta $Version" 'acta --version from PATH'
}

function Invoke-Update([string] $From, [string] $To, [int] $UpdateTimeout) {
    $started = Get-Date
    $deadline = $started.AddSeconds($UpdateTimeout)
    $launches = 0
    $relaunch = $true
    $first = 1
    $restarted = $null
    while ($true) {
        if ($relaunch) {
            $launches++
            $first = (Get-LogLine).Count + 1
            Write-Output ("Launch $launches of Acta $From ({0:N0} s)" -f ((Get-Date) - $started).TotalSeconds)
            Open-Acta
            $relaunch = $false
        }
        $restarted = Find-LogLine 1 $To 'update: started by the updater' | Select-Object -Last 1
        if ($restarted -and (Test-InstalledApp $restarted.ProcessId)) { break }
        # This launch opened its window before the update was applied: the download was
        # slower than the launch budget or the check failed. A user sees the update at
        # the next start, so close the app and start it again, at most twice.
        $deferred = (Find-LogLine $first $From 'not applying in this launch') -or
            (Find-LogLine $first $From 'retrying at next launch')
        if ($launches -lt 3 -and $deferred) {
            $previous = Find-LogLine $first $From 'update: ' | Select-Object -Last 1
            Write-Output "Launch $launches left the update to the next start; closing pid $($previous.ProcessId)"
            Close-Acta $previous.ProcessId
            $relaunch = $true
            continue
        }
        if ((Get-Date) -gt $deadline) {
            Exit-Test "Acta did not restart as $To within $UpdateTimeout s ($launches launch(es))"
        }
        Start-Sleep -Seconds 2
    }
    $elapsed = [int]((Get-Date) - $started).TotalSeconds

    # The update sequence, in this order, in the app log. On Windows the new version
    # exports its CLI from Velopack's after-update hook, before the restart.
    $available = Find-LogLine 1 $From "update: $To available, downloading" | Select-Object -First 1
    $downloaded = Find-LogLine 1 $From "update: $To downloaded" | Select-Object -First 1
    $hook = Find-LogLine 1 $To 'export (after-update): Export' | Select-Object -First 1
    if (-not $available) { Exit-Test "the log has no '${From}: update: $To available'" }
    if (-not $downloaded) { Exit-Test "the log has no '${From}: update: $To downloaded'" }
    if (-not $hook) { Exit-Test "the log has no '${To}: export (after-update): Export'" }
    if (-not ($available.Line -lt $downloaded.Line -and $downloaded.Line -lt $hook.Line -and
            $hook.Line -lt $restarted.Line)) {
        Exit-Test ("the update lines are out of order (available {0}, downloaded {1}, hook {2}, restart {3})" -f
            $available.Line, $downloaded.Line, $hook.Line, $restarted.Line)
    }
    Write-Output ("ok: log shows $From finding $To (line {0}), downloading it (line {1}), the after-update hook of $To (line {2}), $To restarted by the updater (line {3})" -f
        $available.Line, $downloaded.Line, $hook.Line, $restarted.Line)

    # Still up a little later: the new version did not crash after the restart.
    Start-Sleep -Seconds 10
    if (-not (Test-InstalledApp $restarted.ProcessId)) {
        Exit-Test "Acta $To (pid $($restarted.ProcessId)) did not stay up"
    }
    Write-Output "ok: Acta $To runs as pid $($restarted.ProcessId) from $App"
    $line = "Windows: $From updated itself to $To in $elapsed s, $launches launch(es)"
    Write-Output $line
    if ($env:GITHUB_STEP_SUMMARY) { $line | Out-File -Append -Encoding utf8 $env:GITHUB_STEP_SUMMARY }
    Write-Output "--- $Log ---"
    Get-LogLine | Write-Output
}

# What Settings > Apps > Uninstall runs, with Acta still open.
function Invoke-Uninstall {
    if (-not (Test-Path $DataDir)) { Exit-Test "$DataDir is missing before the uninstall" }
    $entry = Get-ItemProperty $UninstallKey
    $command = $entry.QuietUninstallString
    if (-not $command) { $command = "$($entry.UninstallString) --silent" }
    Write-Output "Running $command"
    $process = Start-Process -FilePath cmd.exe -ArgumentList "/d /s /c `"$command`"" -Wait -PassThru
    Write-Output "uninstaller exit code: $($process.ExitCode)"

    $deadline = (Get-Date).AddSeconds(60)
    while ((Test-Path $CliRoot) -or (Test-Path $UninstallKey) -or (Test-Path $Root)) {
        if ((Get-Date) -gt $deadline) {
            Exit-Test ("after 60 s: ActaCli present: {0}, uninstall entry present: {1}, install root present: {2}" -f
                (Test-Path $CliRoot), (Test-Path $UninstallKey), (Test-Path $Root))
        }
        Start-Sleep -Seconds 2
    }
    Write-Output "ok: $CliRoot, the uninstall entry and the install root $Root are gone"
    if (Find-ActaShortcut) { Exit-Test 'a Start menu shortcut to Acta is left' }
    Write-Output 'ok: no Start menu shortcut left'

    $left = @(Get-UserPathEntry | Where-Object { $_ -like '*ActaCli*' })
    if ($left.Count) { Exit-Test "the user PATH still has $($left -join ';')" }
    Write-Output 'ok: no ActaCli entry left in HKCU\Environment\Path'

    $running = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
            $_.Path -and ($_.Path.StartsWith("$Root\") -or $_.Path.StartsWith("$CliRoot\"))
        })
    if ($running.Count) { Exit-Test "Acta processes still running: $($running.Id -join ', ')" }
    Write-Output 'ok: no Acta process left'

    if (-not (Test-Path $DataDir)) { Exit-Test "the uninstall removed $DataDir" }
    if (-not (Test-Path $Log)) { Exit-Test "the uninstall removed $Log" }
    Write-Output "ok: $DataDir is kept"
}

function Invoke-Collect([string] $Dir) {
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    foreach ($file in $Log, $VelopackLog) {
        if (Test-Path $file) { Copy-Item $file $Dir } else { "missing: $file" | Out-File -Append (Join-Path $Dir 'missing.txt') }
    }
    & {
        "# $Root"
        Get-ChildItem -Recurse -Force $Root -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName
        "# $CliRoot"
        Get-ChildItem -Recurse -Force $CliRoot -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName
        '# HKCU\Environment\Path'
        [Environment]::GetEnvironmentVariable('Path', 'User')
        "# $UninstallKey"
        Get-ItemProperty $UninstallKey -ErrorAction SilentlyContinue | Out-String
        '# processes'
        Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -like '*Acta*' } |
            Select-Object Id, Path | Out-String
    } | Out-File -Encoding utf8 (Join-Path $Dir 'state.txt')
}

switch ($Phase) {
    'install' { Invoke-Install $Version }
    'verify' { Invoke-Verify $Version }
    'update' { Invoke-Update $From $To $UpdateTimeout }
    'uninstall' { Invoke-Uninstall }
    'collect' { Invoke-Collect $Dir }
    'cleanup' { Invoke-Cleanup }
}
