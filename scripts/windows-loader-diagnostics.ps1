param([Parameter(Mandatory)][string]$OutputDirectory)

# Failure-only CI diagnostics. Do not execute the test/app, change the runner's
# installed runtimes, or hide the original cargo failure.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (!$IsWindows -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
  throw 'Loader diagnostics require a GitHub-hosted Windows runner.'
}
$null = New-Item -ItemType Directory -Path $OutputDirectory -Force
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vsRoot = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$dumpbin = Get-ChildItem "$vsRoot\VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe" |
  Sort-Object FullName -Descending | Select-Object -First 1
if (!$dumpbin) { throw 'Visual Studio dumpbin is unavailable.' }
$manifestTool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\mt.exe" |
  Sort-Object FullName -Descending | Select-Object -First 1
$binaries = @(
  Get-ChildItem 'src-tauri/target/debug/deps/starclip_lib-*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  Get-ChildItem 'src-tauri/target/debug/deps/ai_stdio-*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  Get-Item 'src-tauri/target/debug/starclip.exe' -ErrorAction SilentlyContinue
)
if (!$binaries.Count) { throw 'No compiled test executable is available.' }

Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class StarclipLoaderProbe {
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  public static extern IntPtr LoadLibraryEx(string name, IntPtr reserved, uint flags);
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Ansi, ExactSpelling=true)]
  public static extern IntPtr GetProcAddress(IntPtr module, string name);
  [DllImport("kernel32.dll", SetLastError=true, EntryPoint="GetProcAddress", ExactSpelling=true)]
  public static extern IntPtr GetOrdinal(IntPtr module, IntPtr ordinal);
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  public static extern uint GetModuleFileName(IntPtr module, StringBuilder name, int size);
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  public static extern bool SetDllDirectory(string path);
  [DllImport("kernel32.dll")] public static extern bool FreeLibrary(IntPtr module);
  [DllImport("kernel32.dll")] public static extern uint SetErrorMode(uint mode);
}
'@
$null = [StarclipLoaderProbe]::SetErrorMode(3)
$results = [Collections.Generic.List[object]]::new()
foreach ($binary in $binaries) {
  Copy-Item -LiteralPath $binary.FullName -Destination $OutputDirectory
  $imports = & $dumpbin.FullName /nologo /imports $binary.FullName
  if ($LASTEXITCODE -ne 0) { throw "dumpbin failed for $($binary.Name)." }
  $imports | Set-Content (Join-Path $OutputDirectory "$($binary.Name).imports.txt")
  if ($manifestTool) {
    $manifestOutput = Join-Path $OutputDirectory "$($binary.Name).manifest.xml"
    $manifestLog = & $manifestTool.FullName -nologo "-inputresource:$($binary.FullName);#1" "-out:$manifestOutput" 2>&1
    @("Manifest extraction exit code: $LASTEXITCODE", $manifestLog) |
      Set-Content (Join-Path $OutputDirectory "$($binary.Name).manifest-status.txt")
  }
  $null = [StarclipLoaderProbe]::SetDllDirectory($binary.DirectoryName)
  $modules = @{}
  $current = $null
  try {
    foreach ($line in $imports) {
      if ($line -match '^\s+([^\s]+\.dll)\s*$') {
        $current = $Matches[1]
        if (!$modules.ContainsKey($current)) {
          $handle = [StarclipLoaderProbe]::LoadLibraryEx($current, [IntPtr]::Zero, 0)
          $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
          $name = [Text.StringBuilder]::new(32768)
          if ($handle -ne [IntPtr]::Zero) { $null = [StarclipLoaderProbe]::GetModuleFileName($handle, $name, $name.Capacity) }
          $loadedPath = $name.ToString()
          $version = if ($loadedPath) { [Diagnostics.FileVersionInfo]::GetVersionInfo($loadedPath).FileVersion } else { $null }
          $modules[$current] = $handle
          $results.Add([pscustomobject]@{ binary=$binary.Name; dll=$current; probePath=$loadedPath; version=$version; loadError=$(if ($handle -eq [IntPtr]::Zero) { $errorCode } else { 0 }) })
        }
      } elseif ($current -and $modules[$current] -ne [IntPtr]::Zero) {
        $symbol = $null
        if ($line -match '^\s+[0-9A-Fa-f]+\s+([A-Za-z_?@][^\s]*)\s*$') {
          $symbol = $Matches[1]
          $address = [StarclipLoaderProbe]::GetProcAddress($modules[$current], $symbol)
        } elseif ($line -match '^\s+[0-9A-Fa-f]+\s+Ordinal\s+(\d+)\s*$') {
          $symbol = "ordinal:$($Matches[1])"
          $address = [StarclipLoaderProbe]::GetOrdinal($modules[$current], [IntPtr][int]$Matches[1])
        }
        if ($symbol -and $address -eq [IntPtr]::Zero) {
          $results.Add([pscustomobject]@{ binary=$binary.Name; dll=$current; missingExport=$symbol; error=[Runtime.InteropServices.Marshal]::GetLastWin32Error() })
          Write-Host "MISSING EXPORT: $($binary.Name) -> $current!$symbol"
        }
      }
    }
  } finally {
    foreach ($handle in $modules.Values) { if ($handle -ne [IntPtr]::Zero) { $null = [StarclipLoaderProbe]::FreeLibrary($handle) } }
    $null = [StarclipLoaderProbe]::SetDllDirectory($null)
  }
}
$results | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutputDirectory 'dll-probe.json')
Write-Host 'DLL paths below belong to the diagnostic process; import tables and binaries are retained for independent loader analysis.'
$results | Format-Table -AutoSize | Out-String -Width 240 | Write-Host
$events = @(Get-WinEvent -FilterHashtable @{ LogName='Application'; StartTime=(Get-Date).AddMinutes(-20) } -ErrorAction SilentlyContinue |
  Where-Object { $_.Message -match 'starclip(?:_lib)?(?:-[a-f0-9]+)?\.exe' } |
  Select-Object TimeCreated, Id, ProviderName, Message)
$events | ConvertTo-Json -Depth 3 | Set-Content (Join-Path $OutputDirectory 'application-events.json')
$events | Format-List | Out-String -Width 200 | Write-Host
