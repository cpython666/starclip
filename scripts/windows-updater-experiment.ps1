param(
  [Parameter(Mandatory)][string]$Installer,
  [Parameter(Mandatory)][string]$RepositoryRoot
)

# One-off acceptance only: current source compares as 0.2.1, then installs the
# real signed public 0.2.2 on a disposable Windows VM. Never run on a user PC.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (!$IsWindows -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or
    $env:GITHUB_REPOSITORY -ne 'cpython666/starclip' -or $env:GITHUB_REF_NAME -ne 'codex/windows-updater-acceptance-20261029') {
  throw 'This experiment requires its exact branch on a fresh GitHub-hosted Windows runner.'
}
$RepositoryRoot = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$evidence = Join-Path $env:RUNNER_TEMP 'starclip-updater-evidence'
$null = New-Item -ItemType Directory -Path $evidence -Force
$stage = 'preflight'
$baselineHash = ''
$targetVersion = '0.2.2'
$manifestHash = '10d0e5646027d4bd0804446b477bd6fafc10f232b0d3135b44b46be3595a706d'
$endpoint = 'https://github.com/cpython666/starclip/releases/latest/download/latest.json'
$targetUrl = 'https://github.com/cpython666/starclip/releases/download/v0.2.2/Starclip_0.2.2_x64-setup.exe'
$publicKey = 'dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDVEQjQ4MUM1NTgxMDBGRjEKUldUeER4Qll4WUcwWFpqaHJOVDZvRGxTcXNZK3FVdGVpTnFxMVNJbmgyRmFNeUVtVFRJcCtVaGEK'
$config = Get-Content (Join-Path $RepositoryRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
if ($config.identifier -ne 'com.starclip.desktop' -or $config.plugins.updater.pubkey -ne $publicKey -or
    $config.plugins.updater.requireSignedVersion -ne $true -or @($config.plugins.updater.endpoints).Count -ne 1 -or
    $config.plugins.updater.endpoints[0] -ne $endpoint) {
  throw 'Production updater identity, endpoint, key or signed-version enforcement changed.'
}

function Assert-PinnedManifest {
  $path = Join-Path $evidence 'latest.json'
  Invoke-WebRequest -Uri $endpoint -OutFile $path -TimeoutSec 60
  if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifestHash) {
    throw 'The public latest manifest changed. This experiment only accepts the pinned 0.2.2 manifest.'
  }
  $manifest = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
  if ($manifest.version -ne $targetVersion -or $manifest.platforms.'windows-x86_64'.url -ne $targetUrl -or
      !$manifest.platforms.'windows-x86_64'.signature) { throw 'The pinned Windows update target is not 0.2.2.' }
}

function Get-MainRoot {
  $script:appProcess.Refresh()
  if ($script:appProcess.HasExited) { throw 'Application exited before the installation confirmation.' }
  return [System.Windows.Automation.AutomationElement]::FromHandle($script:appProcess.MainWindowHandle)
}

function Find-Ui([string]$NamePattern, [bool]$Action = $true, $Scope = $null) {
  if ($null -eq $Scope) { $Scope = Get-MainRoot }
  $nodes = $Scope.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
  $candidates = @($nodes | Where-Object {
    $_.Current.Name -match $NamePattern -and (!$Action -or ($_.Current.IsEnabled -and
      $_.Current.ControlType -in @([System.Windows.Automation.ControlType]::Button, [System.Windows.Automation.ControlType]::TabItem)))
  })
  $visible = @($candidates | Where-Object { !$_.Current.IsOffscreen })
  if ($visible.Count -gt 1 -and $Action) { throw "Ambiguous visible UI action: $NamePattern" }
  if ($visible.Count) { return $visible[0] }
  if ($Action -and $candidates.Count -eq 1) { return $candidates[0] }
  return $null
}

function Wait-Ui([string]$NamePattern, [bool]$Action = $true, [int]$Seconds = 60) {
  $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
  do {
    try { $node = Find-Ui $NamePattern $Action; if ($null -ne $node) { return $node } }
    catch [System.Windows.Automation.ElementNotAvailableException] { }
    Start-Sleep -Milliseconds 500
  } while ([DateTime]::UtcNow -lt $deadline)
  throw "Expected UI was not ready: $NamePattern"
}

function Invoke-Ui($Node) {
  if ($Node.Current.IsOffscreen) {
    $scroll = $null
    if ($Node.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern, [ref]$scroll)) { $scroll.ScrollIntoView() }
    $Node.SetFocus()
  }
  $pattern = $null
  if ($Node.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) { $pattern.Invoke(); return }
  if ($Node.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) { $pattern.Select(); return }
  throw 'The visible control exposes no native accessibility action.'
}

function Check-UpdaterFixture([string]$Mode) {
  # Only the runner-owned schema-2 database established by windows-smoke.ps1.
  # Reuse its original fixed note/group; the old seed helper requires schema 0.
  $result = @'
import hashlib, json, pathlib, sqlite3, sys, time
path, mode, old_hash = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
assert mode in ('seed', 'before-install', 'after-install')
assert path.is_file()
settings = dict(paused=True, pauseRevision=0, uiLanguage='system', theme='forest', layout='list', maxHistory=0,
    shortcut='CommandOrControl+Shift+V', alwaysOnTop=False, retentionDays=0, showStartup=False, stickerPack='none')
note, title, group = 'Starclip CI original note: a quiet orbit.', 'CI original note', 'CI original group'
def verify(db):
    assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok', 'Database integrity failed'
    assert db.execute('PRAGMA user_version').fetchone()[0] == 2, 'Schema changed unexpectedly'
    assert json.loads(db.execute("SELECT value FROM preferences WHERE key='settings'").fetchone()[0]) == settings, 'Fixed settings or pause changed'
    assert db.execute('SELECT content,title,kind,pinned,keep_forever,group_id,copy_count FROM clips').fetchall() == [(note,title,'text',1,0,1001,7)], 'Fixed record changed or clipboard canary was captured'
    assert db.execute('SELECT id,name,color FROM groups').fetchall() == [(1001,group,'#7BBEB0')], 'Fixed group changed'
with sqlite3.connect(path.as_uri() + ('?mode=rw' if mode == 'seed' else '?mode=ro'), uri=True, timeout=5) as db:
    if mode == 'seed':
        assert db.execute('PRAGMA user_version').fetchone()[0] == 2
        assert db.execute('SELECT count(*) FROM clips').fetchone()[0] == 0
        assert db.execute('SELECT count(*) FROM groups').fetchone()[0] == 0
        now = int(time.time() * 1000)
        with db:
            db.execute('INSERT INTO groups(id,name,color) VALUES(1001,?,?)', (group, '#7BBEB0'))
            db.execute('INSERT INTO clips(id,content,title,kind,pinned,group_id,created_at,updated_at,copy_count) VALUES(1001,?,?,?,1,1001,?,?,7)', (note,title,'text',now,now))
            db.execute("INSERT INTO preferences(key,value) VALUES('settings',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", (json.dumps(settings),))
        db.execute('PRAGMA wal_checkpoint(TRUNCATE)')
    verify(db)
points = [(p, json.loads(p.read_text())) for p in (path.parent / 'recovery').glob('*/snapshot.json')]
points = [(p,m) for p,m in points if m['reason'] == 'before-update']
if mode == 'after-install':
    assert len(points) == 1, 'Expected exactly one pre-update recovery point'
    p, manifest = points[0]
    assert manifest['schemaVersion'] == 2
    snapshot = p.parent / 'starclip.sqlite3'
    assert hashlib.sha256(snapshot.read_bytes()).hexdigest() == manifest['files']['starclip.sqlite3']['sha256'], 'Recovery database digest mismatch'
    with sqlite3.connect(snapshot.as_uri() + '?mode=ro', uri=True) as backup: verify(backup)
    old_app = path.parent / 'application-backups' / manifest['id'] / 'starclip.exe'
    assert old_app.is_file() and hashlib.sha256(old_app.read_bytes()).hexdigest() == old_hash, 'Original application backup missing or changed'
else:
    assert not points, 'Recovery point created before the user confirmed installation'
print(json.dumps(dict(stage=mode, schema=2, paused=True, fixedRecords=1, fixedGroups=1, recoveryPoints=len(points))))
'@ | python - $database $Mode $baselineHash
  if ($LASTEXITCODE -ne 0) { throw "Updater fixture verification failed: $Mode" }
  Write-Host $result
  $result | Set-Content -LiteralPath (Join-Path $evidence "$Mode.json") -Encoding utf8
}

Assert-PinnedManifest
# Dot-source the existing smoke to reuse its fresh-runner checks, install path,
# UIA pause assertion and process ownership. It ends with its app stopped.
. (Join-Path $RepositoryRoot 'scripts/windows-smoke.ps1') -Installer $Installer -Version '0.2.1' -Scenario 'fresh'
$baselineHash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
try {
  $stage = 'seed-and-launch'
  Check-UpdaterFixture 'seed'
  Start-And-AssertPaused
  $originalPid = $script:appProcess.Id
  $stage = 'check'
  Invoke-Ui (Wait-Ui '^(?:偏好设置|Preferences)(?:\s|$)')
  Invoke-Ui (Wait-Ui '^(?:帮助|Help)$')
  Invoke-Ui (Wait-Ui '^(?:检查更新|Check for updates)$')
  $null = Wait-Ui '^(?:新版本|Version)\s*0\.2\.2(?:\s|$)' $false
  Assert-PinnedManifest
  $stage = 'download-and-verify'
  Invoke-Ui (Wait-Ui '^(?:下载更新|Download update)$')
  $later = Wait-Ui '^(?:稍后再说|Later)$' $true 180
  # This dialog is reachable only after download() verifies the official key
  # and the signed version. Do not invoke IPC or replace the download result.
  $stage = 'later-keeps-current-app'
  Invoke-Ui $later
  Start-Sleep -Seconds 3
  if (Find-Ui '^(?:稍后再说|Later)$') { throw 'The Later action did not dismiss the installation dialog.' }
  if ((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $baselineHash) { throw 'Application changed before installation confirmation.' }
  if ($script:appProcess.Id -ne $originalPid -or $script:appProcess.HasExited) { throw 'Later unexpectedly restarted the application.' }
  Check-UpdaterFixture 'before-install'
  $stage = 'confirm-install'
  Assert-PinnedManifest
  Invoke-Ui (Wait-Ui '^(?:安装并重启|Install and restart)$')
  $later = Wait-Ui '^(?:稍后再说|Later)$'
  # Limit the confirming action to the dialog button row containing Later,
  # so the identically named underlying Help button cannot count as consent.
  $row = [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($later)
  $confirm = Find-Ui '^(?:安装并重启|Install and restart)$' $true $row
  if ($null -eq $confirm) { throw 'The real confirmation dialog action could not be identified.' }
  Invoke-Ui $confirm
  $stage = 'automatic-restart'
  $deadline = [DateTime]::UtcNow.AddSeconds(180)
  do {
    $script:appProcess.Refresh()
    $restarted = @(Get-Process -Name starclip -ErrorAction SilentlyContinue | Where-Object { $_.Id -ne $originalPid -and $_.Path -eq $exe })
    if ($script:appProcess.HasExited -and $restarted.Count -eq 1) { break }
    Start-Sleep -Milliseconds 500
  } while ([DateTime]::UtcNow -lt $deadline)
  if (!$script:appProcess.HasExited -or $restarted.Count -ne 1) { throw 'No unique automatic restart at the original install path was observed.' }
  $script:appProcess = $restarted[0]
  if ([Diagnostics.FileVersionInfo]::GetVersionInfo($exe).ProductVersion -ne $targetVersion) { throw 'The restarted executable is not public 0.2.2.' }
  $entries = @($uninstallKeys | Where-Object { Test-Path -LiteralPath $_ } | ForEach-Object { Get-ItemProperty -LiteralPath $_ })
  if ($entries.Count -ne 1 -or $entries[0].DisplayVersion -ne $targetVersion) { throw 'Updated application registration does not identify 0.2.2.' }
  $installedHash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($installedHash -eq $baselineHash) { throw 'The executable was not replaced.' }
  $stage = 'restarted-paused-with-fixed-data'
  Wait-ForPausedWindow
  Set-FixedClipboard $true
  for ($i = 0; $i -lt 3; $i++) { Start-Sleep -Seconds 1; Check-UpdaterFixture 'after-install' }
  Stop-OwnedApplication
  Check-UpdaterFixture 'after-install'
  @{ result='passed'; sourceCommit='7fbbae542b89b9ea245e703330594cb1d78ffc76'; comparisonVersion='0.2.1'; displayedSourceVersion='0.2.2'; targetVersion=$targetVersion; manifestSha256=$manifestHash; originalPid=$originalPid; originalExeSha256=$baselineHash; installedExeSha256=$installedHash } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'result.json') -Encoding utf8
  '- Windows one-off updater acceptance passed: real check → signed download → Later → explicit install confirmation → automatic restart → paused UI, fixed data and before-update data/application recovery copies.' >> $env:GITHUB_STEP_SUMMARY
  '- Source code is the candidate; only Tauri comparison/package metadata was 0.2.1. The UI still displayed 0.2.2. Target is the pinned public 0.2.2, not a release of the candidate source.' >> $env:GITHUB_STEP_SUMMARY
} catch {
  @{ result='failed'; stage=$stage; message=$_.Exception.Message } | ConvertTo-Json |
    Set-Content -LiteralPath (Join-Path $evidence 'result.json') -Encoding utf8
  try {
    $root = Get-MainRoot
    $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
      ForEach-Object { '{0} | enabled={1} | offscreen={2} | {3}' -f $_.Current.ControlType.ProgrammaticName, $_.Current.IsEnabled, $_.Current.IsOffscreen, $_.Current.Name } |
      Set-Content -LiteralPath (Join-Path $evidence 'owned-window-uia.txt') -Encoding utf8
  } catch { Write-Host 'The owned window was unavailable for UIA failure evidence.' }
  throw
} finally {
  Stop-OwnedApplication
  # VM disposal removes the synthetic app/profile. Upload only this evidence
  # folder, never the database, clipboard contents, profile or app backups.
}
