"""Sandboxed installer smoke test for a prepared release. Local only; never publishes.

Ported from Haiwire's .local-release/Test-InstallerSandbox.py. Runs
release-out/<v>/Jawjack-<v>-setup.exe with the test-only --test-root flag, so
every location (install folder, data folder, shortcuts, and HKCU keys below
Software\\JawjackTest\\) is disposable. Checks payload info, damaged-payload
refusal, install, upgrade with preserved data, uninstall via the registered
uninstaller (data kept) and uninstall with data removal. With --with-app it
also launches the installed Jawjack.exe (with LOCALAPPDATA pointed at the
sandbox) and checks that setup refuses to touch a running app; that step opens
a real Jawjack window and ends it afterwards (only the test's own process).

The owner's real install, shortcuts, Apps & Features entry and data folder are
snapshotted and must be unchanged. Python 3.11+, Windows.
"""
import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import time
import tomllib
import winreg

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
SETUP = ROOT / "release-out" / VERSION / f"Jawjack-{VERSION}-setup.exe"
UNINSTALL_KEY = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Jawjack"
TEST_ROOT_KEY = r"Software\JawjackTest"


def reg_value(key, name):
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key) as handle:
            return winreg.QueryValueEx(handle, name)[0]
    except FileNotFoundError:
        return None


def reg_exists(key):
    try:
        winreg.OpenKey(winreg.HKEY_CURRENT_USER, key).Close()
        return True
    except FileNotFoundError:
        return False


def known_folder(name):
    out = subprocess.run(["powershell", "-NoProfile", "-Command",
                          f"[Environment]::GetFolderPath('{name}')"],
                         capture_output=True, text=True, check=True).stdout.strip()
    return pathlib.Path(out)


def real_state():
    local = pathlib.Path(os.environ["LOCALAPPDATA"])
    programs = known_folder("Programs")
    desktop = known_folder("Desktop")

    def stamp(path):
        return path.stat().st_mtime_ns if path.exists() else None
    return {
        "uninstall_key": reg_exists(UNINSTALL_KEY),
        "uninstall_version": reg_value(UNINSTALL_KEY, "DisplayVersion"),
        "install_dir": stamp(local / "Programs/Jawjack"),
        "start_menu_link": stamp(programs / "Jawjack.lnk"),
        "desktop_link": stamp(desktop / "Jawjack.lnk"),
        "data_dir_exists": (local / "ChatWorkbench").exists(),
    }


def setup(*args, exe=SETUP, expect=None):
    result = subprocess.run([str(exe), *map(str, args)], capture_output=True, text=True, timeout=300)
    print(f"  setup {' '.join(map(str, args))} -> {result.returncode} {result.stdout.strip()[:300]}")
    if expect is not None:
        assert result.returncode == expect, (expect, result.returncode, result.stdout, result.stderr)
    return result


def sha(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def test_keys():
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, TEST_ROOT_KEY) as handle:
            count = winreg.QueryInfoKey(handle)[0]
            return [name for name in (winreg.EnumKey(handle, index) for index in range(count))
                    if name.startswith("Setup-")]
    except FileNotFoundError:
        return []


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--with-app", action="store_true",
                        help="also launch the sandbox-installed Jawjack.exe to test the running-app refusal")
    args = parser.parse_args()
    assert SETUP.is_file(), SETUP
    keys_before = set(test_keys())
    before = real_state()
    print("Real-profile snapshot:", before)
    root = pathlib.Path(tempfile.mkdtemp(prefix="jawjack-setup-smoke-")).resolve()
    (root / ".jawjack-setup-test-root").write_text("disposable\n", encoding="utf-8")
    local = root / "LocalAppData"
    install = local / "Programs/Jawjack"
    data = local / "ChatWorkbench"
    start_link = root / "StartMenu/Programs/Jawjack.lnk"
    desktop_link = root / "Desktop/Jawjack.lnk"
    app = None
    try:
        info = json.loads(setup("--payload-info", expect=0).stdout)
        assert info["version"] == VERSION
        assert any(item["name"] == "Jawjack.exe" for item in info["files"])

        print("1. Damaged payload is refused")
        damaged = root / "damaged-setup.exe"
        blob = bytearray(SETUP.read_bytes())
        blob[info["image_len"] + 4096] ^= 0xFF
        damaged.write_bytes(blob)
        setup("--quiet", "--test-root", root, exe=damaged, expect=4)
        assert not install.exists()
        damaged.unlink()

        print("2. Fresh install")
        setup("--quiet", "--desktop-shortcut", "--test-root", root, expect=0)
        for item in info["files"]:
            assert sha(install / item["name"]) == item["sha256"], item["name"]
        assert (install / "uninstall.exe").read_bytes() == SETUP.read_bytes()[:info["image_len"]]
        assert start_link.is_file() and desktop_link.is_file()
        keys = sorted(set(test_keys()) - keys_before)
        assert len(keys) == 1, keys
        test_key = rf"{TEST_ROOT_KEY}\{keys[0]}"
        uninstall_key = rf"{test_key}\Uninstall\Jawjack"
        assert reg_value(uninstall_key, "DisplayVersion") == VERSION
        assert reg_value(uninstall_key, "DisplayName") == "Jawjack"
        command = reg_value(uninstall_key, "UninstallString")
        assert "--test-root" in command and str(install / "uninstall.exe") in command, command
        assert reg_value(uninstall_key, "DisplayIcon") == f"{install / 'Jawjack.exe'},0"
        data.mkdir(parents=True, exist_ok=True)
        (data / "sentinel.txt").write_text("user data", encoding="utf-8")

        if args.with_app:
            print("3. Setup refuses while the installed app runs (never ends it)")
            app = subprocess.Popen([str(install / "Jawjack.exe")], cwd=str(root),
                                   env=dict(os.environ, LOCALAPPDATA=str(local)))
            time.sleep(5)
            assert app.poll() is None, "Jawjack.exe exited early"
            setup("--quiet", "--test-root", root, expect=3)
            setup("--uninstall", "--quiet", "--test-root", root, expect=3)
            assert app.poll() is None and (install / "Jawjack.exe").is_file()
            app.terminate()  # The test's own process only.
            app.wait(timeout=30)
            app = None
        else:
            print("3. (skipped: running-app refusal; pass --with-app)")

        print("4. Upgrade/reinstall keeps user data and shortcuts")
        setup("--quiet", "--test-root", root, expect=0)
        assert (data / "sentinel.txt").read_text(encoding="utf-8") == "user data"
        assert desktop_link.is_file() and start_link.is_file()
        assert not any(p.name.startswith((".Jawjack-setup-", ".Jawjack-old-")) for p in install.parent.iterdir())

        print("5. Registered uninstaller (relaunches from %TEMP%), data kept")
        setup("--uninstall", "--quiet", "--test-root", root, exe=install / "uninstall.exe", expect=0)
        deadline = time.time() + 60
        while time.time() < deadline and (install.exists() or reg_exists(uninstall_key)):
            time.sleep(.5)
        assert not install.exists(), list(install.iterdir()) if install.exists() else None
        assert not reg_exists(uninstall_key) and not reg_exists(test_key)
        assert not start_link.exists() and not desktop_link.exists()
        assert (data / "sentinel.txt").is_file(), "user data must be kept"

        print("6. Reinstall, then uninstall with data removal")
        setup("--quiet", "--no-desktop-shortcut", "--test-root", root, expect=0)
        assert not desktop_link.exists()
        setup("--uninstall", "--quiet", "--remove-data", "--test-root", root, expect=0)
        assert not install.exists() and not data.exists()
        assert not reg_exists(test_key)

        after = real_state()
        assert after == before, ("Real profile changed!", before, after)
        print("PASS sandboxed install/upgrade/uninstall; real profile unchanged")
    finally:
        if app is not None and app.poll() is None:
            app.terminate()
        for name in set(test_keys()) - keys_before:
            subprocess.run(["reg", "delete", rf"HKCU\{TEST_ROOT_KEY}\{name}", "/f"], capture_output=True)
        if not test_keys() and reg_exists(TEST_ROOT_KEY):
            subprocess.run(["reg", "delete", rf"HKCU\{TEST_ROOT_KEY}", "/f"], capture_output=True)
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    main()
