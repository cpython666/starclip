#!/usr/bin/env python3
"""Real updater UI on a disposable GitHub macOS ARM runner, never a user Mac.

Current source is packaged with only Tauri comparison metadata set to 0.2.1.
The genuine, immutable public 0.2.2 is the target. This is not an upgrade from
historical 0.2.1 source, nor a release or acceptance of a future target build.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import plistlib
import signal
import sqlite3
import subprocess
import tarfile
import tempfile
import time


IDENTIFIER = "com.starclip.desktop"
COMPARISON = "0.2.1"
TARGET = "0.2.2"
ENDPOINT = "https://github.com/cpython666/starclip/releases/latest/download/latest.json"
MANIFEST_HASH = "10d0e5646027d4bd0804446b477bd6fafc10f232b0d3135b44b46be3595a706d"
TARGET_URL = "https://github.com/cpython666/starclip/releases/download/v0.2.2/Starclip_0.2.2_aarch64.app.tar.gz"
TARGET_HASH = "83113a8c37739335cbc11d905c5b3bc089bba4aa0ad9b380a5d39199b63d224e"
PUBLIC_KEY = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDVEQjQ4MUM1NTgxMDBGRjEKUldUeER4Qll4WUcwWFpqaHJOVDZvRGxTcXNZK3FVdGVpTnFxMVNJbmgyRmFNeUVtVFRJcCtVaGEK"
SETTINGS = dict(paused=True, pauseRevision=0, uiLanguage="system", theme="forest", layout="list",
                maxHistory=0, shortcut="CommandOrControl+Shift+V", alwaysOnTop=False,
                retentionDays=0, showStartup=False, stickerPack="none")
NOTE, TITLE, GROUP = "Starclip CI original note: a quiet orbit.", "CI original note", "CI original group"


def command(*args, **kwargs):
    return subprocess.run(args, check=True, timeout=120, **kwargs)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fresh_runner():
    if (platform.system() != "Darwin" or platform.machine() != "arm64"
            or os.environ.get("GITHUB_ACTIONS") != "true"
            or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or os.environ.get("GITHUB_REPOSITORY") != "cpython666/starclip"):
        raise RuntimeError("This experiment requires a disposable GitHub-hosted macOS ARM runner")
    data = Path.home() / "Library/Application Support" / IDENTIFIER
    for path in (data, Path.home() / "Library/Caches" / IDENTIFIER,
                 Path.home() / "Library/WebKit" / IDENTIFIER,
                 Path.home() / "Library/Preferences" / (IDENTIFIER + ".plist"),
                 Path("/Applications/Starclip.app"), Path.home() / "Applications/Starclip.app"):
        if path.exists() or path.is_symlink():
            raise RuntimeError("Existing Starclip installation or profile; refusing this experiment")
    if subprocess.run(["pgrep", "-ix", "starclip"], capture_output=True).returncode != 1:
        raise RuntimeError("A Starclip process exists, or process discovery failed")
    config = json.loads((Path(__file__).resolve().parent.parent / "src-tauri/tauri.conf.json").read_text())
    updater = config["plugins"]["updater"]
    if (config["identifier"] != IDENTIFIER or updater["pubkey"] != PUBLIC_KEY
            or updater["endpoints"] != [ENDPOINT] or updater["requireSignedVersion"] is not True):
        raise RuntimeError("Production identity, key, endpoint or signed-version enforcement changed")
    return Path(os.environ["RUNNER_TEMP"]).resolve(strict=True), data


def verify_database(path):
    with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True, timeout=5) as db:
        assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
        assert db.execute("PRAGMA user_version").fetchone()[0] == 2
        assert json.loads(db.execute("SELECT value FROM preferences WHERE key='settings'").fetchone()[0]) == SETTINGS
        assert db.execute("SELECT content,title,kind,pinned,keep_forever,group_id,copy_count FROM clips").fetchall() == [(NOTE, TITLE, "text", 1, 0, 1001, 7)], "Fixed record changed or canary captured"
        assert db.execute("SELECT id,name,color FROM groups").fetchall() == [(1001, GROUP, "#7BBEB0")]


def seed_database(path):
    assert path.is_file(), "Only the application may create its synthetic database"
    with sqlite3.connect(path.as_uri() + "?mode=rw", uri=True, timeout=5) as db:
        assert db.execute("PRAGMA user_version").fetchone()[0] == 2
        assert db.execute("SELECT count(*) FROM clips").fetchone()[0] == 0
        assert db.execute("SELECT count(*) FROM groups").fetchone()[0] == 0
        now = int(time.time() * 1000)
        with db:
            db.execute("INSERT INTO groups(id,name,color) VALUES(1001,?,?)", (GROUP, "#7BBEB0"))
            db.execute("INSERT INTO clips(id,content,title,kind,pinned,group_id,created_at,updated_at,copy_count) VALUES(1001,?,?,?,1,1001,?,?,7)", (NOTE, TITLE, "text", now, now))
            db.execute("INSERT INTO preferences(key,value) VALUES('settings',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", (json.dumps(SETTINGS),))
            db.execute("INSERT INTO preferences(key,value) VALUES('onboarding_v1','true') ON CONFLICT(key) DO UPDATE SET value=excluded.value")
        db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    verify_database(path)


def verify_app(app, version):
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    assert (info.get("CFBundleIdentifier"), info.get("CFBundleShortVersionString"), info.get("CFBundleExecutable")) == (IDENTIFIER, version, "starclip")
    binary = app / "Contents/MacOS/starclip"
    assert binary.resolve(strict=True).is_relative_to(app.resolve(strict=True))
    assert command("lipo", "-archs", str(binary), capture_output=True, text=True).stdout.split() == ["arm64"]
    command("codesign", "--verify", "--deep", "--strict", str(app))
    return digest(binary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preflight", action="store_true")
    parser.add_argument("--app", type=Path)
    parser.add_argument("--inspector", type=Path)
    args = parser.parse_args()
    runner_temp, data = fresh_runner()
    if args.preflight:
        print("Disposable runner and absent Starclip profile verified")
        return
    if not args.app or not args.inspector:
        parser.error("--app and --inspector are required outside preflight")
    inspector = args.inspector.resolve(strict=True)
    assert inspector.is_relative_to(runner_temp), "UI helper must be runner-owned"
    command(str(inspector), "preflight")
    evidence = runner_temp / "starclip-macos-updater-evidence"
    evidence.mkdir(exist_ok=False)
    scratch = Path(tempfile.mkdtemp(prefix="starclip-macos-updater-", suffix=".noindex", dir=runner_temp))
    installation = scratch / "Starclip.app"
    database = data / "starclip.sqlite3"
    binary = installation / "Contents/MacOS/starclip"
    process, active_pid = None, None
    stage = "preflight"
    baseline_hash = ""

    def write(name, result):
        (evidence / name).write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")

    def pinned_manifest():
        raw = command("curl", "--fail", "--silent", "--show-error", "--location", "--retry", "3", "--max-time", "60", ENDPOINT, capture_output=True).stdout
        assert hashlib.sha256(raw).hexdigest() == MANIFEST_HASH, "Public latest changed; experiment is pinned to 0.2.2"
        manifest = json.loads(raw)
        assert manifest["version"] == TARGET
        assert manifest["platforms"]["darwin-aarch64"]["url"] == TARGET_URL
        assert manifest["platforms"]["darwin-aarch64"]["signature"]
        write("latest.json", manifest)

    def ui(operation, *arguments):
        return json.loads(command(str(inspector), operation, *map(str, arguments), capture_output=True, text=True).stdout)

    def wait_ui(pattern, *, seconds=60, press=False, text=False, confirmation=False):
        flags = (["--text"] if text else []) + (["--confirmation"] if confirmation else [])
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if ui("find", active_pid, pattern, *flags)["found"]:
                if press:
                    assert ui("press", active_pid, pattern, *flags)["found"]
                return
            time.sleep(0.5)
        raise RuntimeError("Expected UI was not ready: " + pattern)

    def wait_window():
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if ui("inspect", active_pid)["window"] and database.is_file():
                return
            time.sleep(0.5)
        raise RuntimeError("The owned native main window did not appear")

    def start():
        nonlocal process, active_pid
        # Only write original synthetic canaries; do not inspect a clipboard.
        command("pbcopy", input=b"Starclip CI original clipboard canary: a quiet orbit.")
        process = subprocess.Popen([str(binary)], cwd=scratch, start_new_session=True,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        active_pid = process.pid
        wait_window()
        write("accessibility-prepare-" + str(active_pid) + ".json", ui("prepare", active_pid))
        command("pbcopy", input=b"Starclip CI second original canary: a bright orbit.")

    def stop():
        nonlocal active_pid
        owned = ui("processes", installation)["pids"] if installation.exists() else []
        if process is not None and process.poll() is None and process.pid not in owned:
            owned.append(process.pid)
        for pid in owned:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline and ui("processes", installation)["pids"]:
            time.sleep(0.2)
        for pid in ui("processes", installation)["pids"]:
            os.kill(pid, signal.SIGKILL)
        if process is not None:
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=10)
        active_pid = None

    def fixture(mode):
        verify_database(database)
        points = [(p, json.loads(p.read_text())) for p in (data / "recovery").glob("*/snapshot.json")]
        points = [(p, m) for p, m in points if m["reason"] == "before-update"]
        if mode == "after-install":
            assert len(points) == 1, "Expected one pre-update recovery point"
            path, manifest = points[0]
            assert manifest["schemaVersion"] == 2
            snapshot = path.parent / "starclip.sqlite3"
            assert digest(snapshot) == manifest["files"]["starclip.sqlite3"]["sha256"]
            verify_database(snapshot)
            backup = data / "application-backups" / manifest["id"] / "Starclip.app"
            assert verify_app(backup, COMPARISON) == baseline_hash, "Original application backup changed"
        else:
            assert not points, "Recovery point created before user confirmation"
            assert not (data / "application-backups").exists(), "Application backup created before confirmation"
        result = dict(stage=mode, schema=2, paused=True, fixedRecords=1, fixedGroups=1, recoveryPoints=len(points))
        write(mode + ".json", result)
        print(json.dumps(result), flush=True)

    try:
        pinned_manifest()
        # Independently pin the expected installed executable. This does not
        # replace the updater's real HTTPS download or its signature checks.
        archive = command("curl", "--fail", "--silent", "--show-error", "--location", "--retry", "3", "--max-time", "60", TARGET_URL, capture_output=True).stdout
        assert hashlib.sha256(archive).hexdigest() == TARGET_HASH
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
            members = [m for m in bundle if m.name.endswith("/Contents/MacOS/starclip") and m.isfile()]
            assert len(members) == 1
            target_executable_hash = hashlib.sha256(bundle.extractfile(members[0]).read()).hexdigest()
        stage = "fresh-launch-and-seed"
        verify_app(args.app.resolve(strict=True), COMPARISON)
        command("ditto", str(args.app), str(installation))
        baseline_hash = verify_app(installation, COMPARISON)
        start()
        time.sleep(2)
        command("python3", str(Path(__file__).with_name("installer-fixture.py")), str(database), "fresh", "2")
        stop()
        seed_database(database)
        start()
        original_pid = active_pid
        wait_ui(r"^(?:开启剪贴记录|Start recording)$")
        fixture("before-check")
        stage = "check"
        wait_ui(r"^(?:偏好设置|Preferences)(?:\s|$)", press=True)
        wait_ui(r"^(?:帮助|Help)$", press=True)
        wait_ui(r"^(?:检查更新|Check for updates)$", press=True)
        wait_ui(r"^(?:新版本|Version)\s*0\.2\.2(?:\s|$)", text=True)
        pinned_manifest()
        stage = "download-and-verify"
        wait_ui(r"^(?:下载更新|Download update)$", press=True)
        wait_ui(r"^(?:稍后再说|Later)$", seconds=180)
        # Ready is reached only after the genuine updater verifies the public
        # signature and bound version; no IPC or network result is injected.
        fixture("ready-before-confirmation")
        stage = "later-keeps-current-app"
        wait_ui(r"^(?:稍后再说|Later)$", press=True)
        time.sleep(2)
        assert not ui("find", active_pid, r"^(?:稍后再说|Later)$")["found"]
        assert digest(binary) == baseline_hash
        assert process.poll() is None and active_pid == original_pid
        fixture("before-install")
        stage = "confirm-install"
        pinned_manifest()
        wait_ui(r"^(?:安装并重启|Install and restart)$", press=True)
        wait_ui(r"^(?:稍后再说|Later)$")
        wait_ui(r"^(?:安装并重启|Install and restart)$", press=True, confirmation=True)
        stage = "automatic-restart"
        deadline = time.monotonic() + 180
        new_pids = []
        while time.monotonic() < deadline:
            new_pids = [pid for pid in ui("processes", installation)["pids"] if pid != original_pid]
            if process.poll() is not None and len(new_pids) == 1:
                break
            time.sleep(0.5)
        assert process.poll() is not None and len(new_pids) == 1, "No unique automatic restart at original install path"
        active_pid = new_pids[0]
        installed_hash = verify_app(installation, TARGET)
        assert installed_hash == target_executable_hash and installed_hash != baseline_hash
        stage = "restarted-paused-with-fixed-data"
        wait_window()
        write("accessibility-prepare-" + str(active_pid) + ".json", ui("prepare", active_pid))
        wait_ui(r"^(?:开启剪贴记录|Start recording)$")
        command("pbcopy", input=b"Starclip CI post-update original canary: a new orbit.")
        for _ in range(3):
            time.sleep(1)
            fixture("after-install")
        restarted_pid = active_pid
        stop()
        fixture("after-install")
        write("result.json", dict(result="passed", sourceCommit=os.environ["GITHUB_SHA"], comparisonVersion=COMPARISON,
                                  targetVersion=TARGET, manifestSha256=MANIFEST_HASH, targetArchiveSha256=TARGET_HASH,
                                  originalPid=original_pid, restartedPid=restarted_pid,
                                  originalExecutableSha256=baseline_hash, installedExecutableSha256=installed_hash))
        with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as file:
            file.write("- macOS native updater passed: check → signed download → Later → confirmation → data/application recovery copies → bundle replacement → automatic restart → paused UI and fixed data.\n")
            file.write("- Only Tauri comparison/package metadata was 0.2.1. The source is this run's commit; target is pinned public 0.2.2. This is neither historical 0.2.1-source migration nor acceptance/publication of a future target.\n")
    except Exception as error:
        write("result.json", dict(result="failed", stage=stage, message=str(error)))
        if active_pid:
            try:
                write("owned-window-ax.json", ui("dump", active_pid))
            except Exception:
                pass
        raise
    finally:
        stop()
        # VM disposal removes the test app and profile. Evidence deliberately
        # excludes databases, application backups, clipboard and credentials.


if __name__ == "__main__":
    main()
