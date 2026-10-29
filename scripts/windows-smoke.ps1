param(
  [Parameter(Mandatory)][string]$Installer,
  [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
  [Parameter(Mandatory)][ValidateSet('fresh', 'upgrade')][string]$Scenario,
  [string]$BaselineInstaller
)

# Only disposable GitHub-hosted runners. Windows Known Folders do not follow an
# APPDATA environment override, so refuse any pre-existing app state instead.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (!$IsWindows -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
  throw 'Windows installation smoke tests require a fresh GitHub-hosted runner.'
}
if (!$env:RUNNER_TEMP -or !(Test-Path -LiteralPath $env:RUNNER_TEMP -PathType Container)) {
  throw 'Runner temporary directory is unavailable.'
}
$dataDir = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'com.starclip.desktop'
$webviewDir = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'com.starclip.desktop'
$uninstallKeys = @(
  'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Starclip',
  'HKCU:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Starclip',
  'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Starclip',
  'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Starclip'
)
foreach ($path in @($dataDir, $webviewDir) + $uninstallKeys) {
  if (Test-Path -LiteralPath $path) { throw 'Existing Starclip data or installation found; refusing to run.' }
}
if (Get-Process -Name starclip -ErrorAction SilentlyContinue) { throw 'Starclip is already running.' }
$Installer = (Resolve-Path -LiteralPath $Installer).Path
if ($Scenario -eq 'upgrade') {
  if (!$BaselineInstaller) { throw 'A verified 0.1.0 installer is required for a real upgrade test.' }
  $BaselineInstaller = (Resolve-Path -LiteralPath $BaselineInstaller).Path
  if ([version]$Version -le [version]'0.1.0') { throw 'Upgrade target must be newer than 0.1.0.' }
}
$installDir = Join-Path $env:RUNNER_TEMP "starclip-smoke-$Scenario-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT"
if (Test-Path -LiteralPath $installDir) { throw 'Smoke installation directory already exists.' }
$null = New-Item -ItemType Directory -Path $installDir
$exe = Join-Path $installDir 'starclip.exe'
$database = Join-Path $dataDir 'starclip.sqlite3'
$script:appProcess = $null
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class StarclipSmokeWindow {
  [DllImport("user32.dll")]
  [return: MarshalAs(UnmanagedType.Bool)]
  public static extern bool IsWindow(IntPtr handle);
}
'@

# This script never reads the clipboard. A fixed canary replaces runner contents
# before any application launch, making a pause regression safe and detectable.
function Set-FixedClipboard([bool]$AfterStartup = $false) {
  $canary = if ($AfterStartup) { 'Starclip CI original clipboard canary: a second orbit.' } else { 'Starclip CI original clipboard canary: a quiet orbit.' }
  Set-Clipboard -Value $canary
}

function Install-Package([string]$Path, [string]$ExpectedVersion, [bool]$Update) {
  if ([Diagnostics.FileVersionInfo]::GetVersionInfo($Path).ProductVersion -ne $ExpectedVersion) {
    throw 'Installer version does not match the expected version.'
  }
  # NSIS /D must be last and unquoted, including when the directory has spaces.
  $arguments = if ($Update) { "/S /UPDATE /NS /D=$installDir" } else { "/S /NS /D=$installDir" }
  $process = Start-Process -FilePath $Path -ArgumentList $arguments -PassThru
  if (!$process.WaitForExit(180000)) { throw 'Installer did not finish within three minutes.' }
  if ($process.ExitCode -ne 0) { throw "Installer failed with exit code $($process.ExitCode)." }
  if (!(Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Installed executable is missing.' }
  if ([Diagnostics.FileVersionInfo]::GetVersionInfo($exe).ProductVersion -ne $ExpectedVersion) {
    throw 'Installed executable version does not match the installer.'
  }
  $entries = @($uninstallKeys | Where-Object { Test-Path -LiteralPath $_ } | ForEach-Object { Get-ItemProperty -LiteralPath $_ })
  if ($entries.Count -ne 1 -or $entries[0].DisplayVersion -ne $ExpectedVersion) {
    throw 'Installed application registration has an unexpected version or duplicate entries.'
  }
  Write-Host "NSIS installation verified: $ExpectedVersion"
}

function Start-And-AssertPaused {
  Set-FixedClipboard
  $script:appProcess = Start-Process -FilePath $exe -PassThru
  Wait-ForPausedWindow
}

function Wait-ForPausedWindow {
  $deadline = [DateTime]::UtcNow.AddSeconds(60)
  do {
    $script:appProcess.Refresh()
    if ($script:appProcess.HasExited) { throw 'Installed application exited during startup.' }
    $handle = $script:appProcess.MainWindowHandle
    if ($handle -ne [IntPtr]::Zero) {
      try {
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($handle)
        # The historical baseline is Chinese; the candidate follows the runner's
        # system language. Match the shipped labels without changing preferences.
        $skip = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
          [System.Windows.Automation.OrCondition]::new(
            [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, '跳过教程'),
            [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, 'Skip tour')))
        if ($skip -and !$skip.Current.IsOffscreen -and $skip.Current.IsEnabled) {
          $pattern = $null
          if ($skip.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
            $pattern.Invoke() # Never click the capture toggle.
          }
        }
        $nodes = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
        foreach ($node in $nodes) {
          if (!$node.Current.IsOffscreen -and $node.Current.Name -match '^(?:记录已暂停|Recording paused)(?:\s|/|$)') {
            if (Test-Path -LiteralPath $database -PathType Leaf) {
              Write-Host 'Native window rendered and reports recording paused.'
              return
            }
          }
        }
      } catch [System.Windows.Automation.ElementNotAvailableException] {
        # The tutorial can remove its accessibility nodes while we inspect them.
      }
    }
    Start-Sleep -Milliseconds 500
  } while ([DateTime]::UtcNow -lt $deadline)
  throw 'Native window, rendered pause status or database was not ready within one minute.'
}

function Check-WindowRecreation {
  # Only the fixed runner-owned database. An enabled pet changes the expected
  # window lifecycle, so verify the fixture before requesting a normal close.
  @'
import json, pathlib, sqlite3, sys
path = pathlib.Path(sys.argv[1])
with sqlite3.connect(path.as_uri() + '?mode=ro', uri=True) as db:
    saved = db.execute("SELECT value FROM preferences WHERE key='starpet_v1'").fetchone()
    assert saved is None or json.loads(saved[0])['enabled'] is False, 'Window fixture must have the pet disabled'
'@ | python - $database
  if ($LASTEXITCODE -ne 0) { throw 'Window lifecycle fixture is not isolated or has an enabled pet.' }
  $script:appProcess.Refresh()
  $previous = $script:appProcess.MainWindowHandle
  if ($previous -eq [IntPtr]::Zero -or !$script:appProcess.CloseMainWindow()) {
    throw 'Could not request a normal close of the candidate main window.'
  }
  $deadline = [DateTime]::UtcNow.AddSeconds(30)
  while ([StarclipSmokeWindow]::IsWindow($previous)) {
    $script:appProcess.Refresh()
    if ($script:appProcess.HasExited) { throw 'Closing the main window exited the application.' }
    if ([DateTime]::UtcNow -ge $deadline) { throw 'The main window was hidden but its native handle was not destroyed.' }
    Start-Sleep -Milliseconds 200
  }
  $script:appProcess.Refresh()
  if ($script:appProcess.HasExited) { throw 'The background application exited after releasing its window.' }
  # A second launch uses the shipped single-instance callback to rebuild/show
  # the original process's window. No JavaScript injection or product test hook.
  $second = Start-Process -FilePath $exe -PassThru
  if (!$second.WaitForExit(10000)) {
    & taskkill.exe /PID $second.Id /T /F | Out-Null
    throw 'The second instance did not hand control back to the background application.'
  }
  if ($second.ExitCode -ne 0) { throw 'The second instance failed to reopen the main window.' }
  Wait-ForPausedWindow
  Check-Database $Scenario 2
  Write-Host 'Native main window was destroyed and recreated; recording remains paused and fixed data is intact.'
}

function Stop-OwnedApplication {
  if ($null -ne $script:appProcess) {
    $script:appProcess.Refresh()
    if (!$script:appProcess.HasExited) {
      # A native close keeps the app in the tray. Stop only our process tree.
      & taskkill.exe /PID $script:appProcess.Id /T /F | Out-Null
      if (!$script:appProcess.WaitForExit(10000)) { throw 'Smoke application did not stop.' }
    }
    $script:appProcess = $null
  }
}

# SQLite access is limited to the new runner-owned app directory checked above.
# Python's sqlite3 is standard library; no driver, GUI framework or app hook.
function Check-Database([string]$Mode, [int]$Schema) {
  if ($null -ne $script:appProcess) {
    $script:appProcess.Refresh()
    if ($script:appProcess.HasExited) { throw 'Installed application exited during observation.' }
  }
  & python (Join-Path $PSScriptRoot 'installer-fixture.py') $database $Mode $Schema
  if ($LASTEXITCODE -ne 0) { throw "Fixed database verification failed: $Mode" }
}

try {
  Set-FixedClipboard
  if ($Scenario -eq 'upgrade') {
    Install-Package $BaselineInstaller '0.1.0' $false
    Start-And-AssertPaused
    Set-FixedClipboard $true
    for ($i = 0; $i -lt 3; $i++) { Start-Sleep -Seconds 1; Check-Database 'fresh' 0 }
    Stop-OwnedApplication
    Check-Database 'seed' 0
    Install-Package $Installer $Version $true
  } else {
    Install-Package $Installer $Version $false
  }
  Start-And-AssertPaused
  Set-FixedClipboard $true
  for ($i = 0; $i -lt 3; $i++) { Start-Sleep -Seconds 1; Check-Database $Scenario 2 }
  Check-WindowRecreation
  Stop-OwnedApplication
  Check-Database $Scenario 2
  if ($env:GITHUB_STEP_SUMMARY) {
    "- Windows native $Scenario smoke passed: NSIS $Version, rendered pause status, fixed database checks. No capture toggle was enabled." >> $env:GITHUB_STEP_SUMMARY
    '- Candidate main-window close destroyed the native HWND while the process stayed alive; a second launch recreated the window, restored paused UI and preserved fixed data.' >> $env:GITHUB_STEP_SUMMARY
    if ($Scenario -eq 'upgrade') { '- Real 0.1.0 → candidate upgrade preserved the fixed record, group, preferences and pre-migration recovery copy.' >> $env:GITHUB_STEP_SUMMARY }
    '- This does not verify the in-app update download/dialog flow, SmartScreen, or every Windows environment.' >> $env:GITHUB_STEP_SUMMARY
  }
} finally {
  Stop-OwnedApplication
  # Keep the synthetic state on this disposable VM for failure diagnosis. No
  # databases, clipboard data, screenshots or profile files are uploaded.
}
