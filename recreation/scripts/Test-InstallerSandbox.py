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

Then, in a second sandbox, it checks the in-app updater contract
(--app-pid <pid> --quiet --relaunch --result-file <file>) with setups built
from the fresh target/release/jawjack-setup.exe and a tiny stub "Jawjack.exe"
(compiled here with rustc) instead of the real app, so relaunching never opens
Jawjack: waiting for a dummy app process that exits by itself, the result file
on success and on forced failures, the relaunch after both, and the one-setup
lock. --fresh-setup also runs the first part with the fresh setup program and
the packaged app folder instead of the prepared release-out setup.

The owner's real install, shortcuts, Apps & Features entry and data folder are
snapshotted and must be unchanged. Python 3.11+, Windows.
"""
import argparse
import hashlib
import importlib.util
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
FRESH_BASE = ROOT / "target/release/jawjack-setup.exe"
APP_FOLDER = ROOT / "release-out" / VERSION / "app/Jawjack"
UNINSTALL_KEY = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Jawjack"
TEST_ROOT_KEY = r"Software\JawjackTest"
TOOLCHAIN = os.environ.get("JAWJACK_TOOLCHAIN", "1.99.0")
RUSTUP = pathlib.Path(os.environ.get("USERPROFILE", pathlib.Path.home())) / ".cargo/bin/rustup.exe"

# Stands in for Jawjack.exe in the update-flow sandbox. "--sleep N" plays the
# app that closes itself after starting setup; started without arguments (a
# relaunch) it logs the version file installed beside it and exits.
STUB_SOURCE = r'''
#![windows_subsystem = "windows"]
use std::io::Write;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "--sleep" {
        let seconds = args[1].parse().unwrap_or(1);
        std::thread::sleep(std::time::Duration::from_secs(seconds));
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let folder = exe.parent().unwrap();
    let version = std::fs::read_to_string(folder.join("stub-version.txt")).unwrap_or_default();
    let log = folder.parent().unwrap().join("relaunch-log.txt");
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap();
    writeln!(file, "{} args={}", version.trim(), args.len()).unwrap();
}
'''


def packager():
    """scripts/Package-Release.py, for its build_setup (its main() does not run)."""
    spec = importlib.util.spec_from_file_location("package_release", ROOT / "scripts/Package-Release.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


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


def setup(*args, exe=None, expect=None):
    exe = SETUP if exe is None else exe
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
    global SETUP
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--with-app", action="store_true",
                        help="also launch the sandbox-installed Jawjack.exe to test the running-app refusal")
    parser.add_argument("--fresh-setup", action="store_true",
                        help="run the release checks with target/release/jawjack-setup.exe and the packaged "
                             "app folder instead of the prepared release-out setup")
    parser.add_argument("--app-folder", type=pathlib.Path, default=APP_FOLDER,
                        help="with --fresh-setup: the app folder to pack (default: the packaged one for "
                             "this version)")
    parser.add_argument("--skip-update-flow", action="store_true",
                        help="skip the in-app updater checks (wait, result file, relaunch, lock)")
    args = parser.parse_args()
    keys_before = set(test_keys())
    before = real_state()
    print("Real-profile snapshot:", before)
    work = pathlib.Path(tempfile.mkdtemp(prefix="jawjack-setup-work-")).resolve()
    try:
        if args.fresh_setup:
            folder = args.app_folder.resolve()
            assert FRESH_BASE.is_file() and (folder / "Jawjack.exe").is_file(), (FRESH_BASE, folder)
            entries = [(path.name, path.read_bytes()) for path in sorted(folder.iterdir()) if path.is_file()]
            SETUP = work / f"Jawjack-{VERSION}-setup.exe"
            SETUP.write_bytes(packager().build_setup(FRESH_BASE.read_bytes(), VERSION, entries))
            print("Using a fresh setup:", SETUP)
        assert SETUP.is_file(), SETUP
        release_flow(args, keys_before)
        if args.skip_update_flow:
            print("(skipped: in-app updater checks)")
        else:
            update_flow(work)
        after = real_state()
        assert after == before, ("Real profile changed!", before, after)
        print("PASS all sandboxed installer checks; real profile unchanged")
    finally:
        for name in set(test_keys()) - keys_before:
            subprocess.run(["reg", "delete", rf"HKCU\{TEST_ROOT_KEY}\{name}", "/f"], capture_output=True)
        if not test_keys() and reg_exists(TEST_ROOT_KEY):
            subprocess.run(["reg", "delete", rf"HKCU\{TEST_ROOT_KEY}", "/f"], capture_output=True)
        shutil.rmtree(work, ignore_errors=True)


def release_flow(args, keys_before):
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

        print("PASS sandboxed install/upgrade/uninstall")
    finally:
        if app is not None and app.poll() is None:
            app.terminate()
        shutil.rmtree(root, ignore_errors=True)


def build_stub(work):
    source = work / "stub.rs"
    source.write_text(STUB_SOURCE, encoding="utf-8")
    exe = work / "stub.exe"
    built = subprocess.run([str(RUSTUP), "run", TOOLCHAIN, "rustc", "-O", "--edition", "2021",
                            "-o", str(exe), str(source)], capture_output=True, text=True, cwd=work, timeout=600)
    assert built.returncode == 0, built.stderr[-2000:]
    return exe.read_bytes()


def read_result(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    assert set(data) == {"version", "ok", "message"}, data
    return data


def log_lines(log):
    return log.read_text(encoding="utf-8").splitlines() if log.exists() else []


def wait_relaunch(log, count, timeout=30):
    deadline = time.time() + timeout
    while time.time() < deadline:
        lines = log_lines(log)
        if len(lines) >= count:
            assert len(lines) == count, lines
            return lines
        time.sleep(.25)
    raise AssertionError(f"no relaunch seen in {log}: {log_lines(log)}")


def update_flow(work):
    """The in-app updater contract, with a stub app (never the real Jawjack)."""
    assert FRESH_BASE.is_file(), FRESH_BASE
    base = FRESH_BASE.read_bytes()
    package = packager()
    stub = build_stub(work)

    def stub_setup(version):
        path = work / f"stub-{version}-setup.exe"
        path.write_bytes(package.build_setup(base, version, [("Jawjack.exe", stub),
                                                             ("stub-version.txt", version.encode())]))
        return path
    first, second, third = stub_setup("0.0.1"), stub_setup("0.0.2"), stub_setup("0.0.3")

    root = pathlib.Path(tempfile.mkdtemp(prefix="jawjack-setup-update-")).resolve()
    (root / ".jawjack-setup-test-root").write_text("disposable\n", encoding="utf-8")
    install = root / "LocalAppData/Programs/Jawjack"
    data = root / "LocalAppData/ChatWorkbench"
    log = install.parent / "relaunch-log.txt"  # appended to by a relaunched stub
    results = root / "results"
    results.mkdir()
    portable = root / "portable"
    portable.mkdir()
    (portable / "chat-workbench.exe").write_bytes(stub)
    dummies = []

    def installed():
        return (install / "stub-version.txt").read_text(encoding="utf-8")

    def dummy(exe, seconds):
        """A stand-in for Jawjack closing itself after starting setup."""
        process = subprocess.Popen([str(exe), "--sleep", str(seconds)], cwd=str(root))
        dummies.append(process)
        time.sleep(.5)
        assert process.poll() is None, "the dummy app exited early"
        return process

    try:
        print("U1. Quiet install of stub 0.0.1 (starts nothing)")
        setup("--quiet", "--test-root", root, exe=first, expect=0)
        assert installed() == "0.0.1"
        data.mkdir(parents=True, exist_ok=True)
        (data / "sentinel.txt").write_text("user data", encoding="utf-8")
        time.sleep(1)
        assert not log.exists()

        print("U2. Updater handoff: waits for the app to exit by itself, upgrades, writes the result, relaunches")
        app = dummy(install / "Jawjack.exe", 4)
        result = results / "update-result.json"
        started = time.time()
        setup("--app-pid", app.pid, "--quiet", "--relaunch", "--result-file", result,
              "--test-root", root, exe=second, expect=0)
        waited = time.time() - started
        assert app.poll() == 0, "the app must exit by itself, never be ended"
        assert waited >= 3, f"setup did not wait for the app ({waited:.1f} s)"
        assert read_result(result) == {"version": "0.0.2", "ok": True, "message": ""}
        assert sorted(p.name for p in results.iterdir()) == [result.name], "temp result left behind"
        assert installed() == "0.0.2"
        assert (data / "sentinel.txt").read_text(encoding="utf-8") == "user data"
        assert not any(p.name.startswith((".Jawjack-setup-", ".Jawjack-old-")) for p in install.parent.iterdir())
        assert wait_relaunch(log, 1)[-1] == "0.0.2 args=0"
        print(f"  waited {waited:.1f} s; result {result.read_text(encoding='utf-8')}")

        print("U3. Forced failure (damaged setup, portable app pid): result says why, kept version relaunched")
        app = dummy(portable / "chat-workbench.exe", 3)
        info = json.loads(setup("--payload-info", exe=third, expect=0).stdout)
        damaged = work / "damaged-setup.exe"
        blob = bytearray(third.read_bytes())
        blob[info["image_len"] + 64] ^= 0xFF
        damaged.write_bytes(blob)
        result = results / "damaged-result.json"
        setup("--app-pid", app.pid, "--quiet", "--relaunch", "--result-file", result,
              "--test-root", root, exe=damaged, expect=4)
        assert app.poll() == 0, "relaunch must wait for the app to exit"
        outcome = read_result(result)
        assert outcome["ok"] is False and outcome["version"] == "" and "damaged" in outcome["message"], outcome
        assert installed() == "0.0.2"
        assert wait_relaunch(log, 2)[-1] == "0.0.2 args=0"
        print(f"  result {result.read_text(encoding='utf-8')}")

        print("U4. Forced failure (install folder in use): result names the version, kept version relaunched")
        app = dummy(portable / "chat-workbench.exe", 2)
        result = results / "in-use-result.json"
        # Opened without FILE_SHARE_DELETE, so the install folder cannot be renamed.
        with open(install / "stub-version.txt", "rb"):
            setup("--app-pid", app.pid, "--quiet", "--relaunch", "--result-file", result,
                  "--test-root", root, exe=third, expect=1)
        outcome = read_result(result)
        assert outcome["ok"] is False and outcome["version"] == "0.0.3", outcome
        assert "previous installation was kept" in outcome["message"], outcome
        assert installed() == "0.0.2"
        assert not any(p.name.startswith((".Jawjack-setup-", ".Jawjack-old-")) for p in install.parent.iterdir())
        assert wait_relaunch(log, 3)[-1] == "0.0.2 args=0"
        print(f"  result {result.read_text(encoding='utf-8')}")

        print("U5. One setup at a time: a second setup and an uninstall are refused (exit 5)")
        app = dummy(install / "Jawjack.exe", 6)
        holder = subprocess.Popen([str(third), "--app-pid", str(app.pid), "--quiet", "--test-root", str(root)],
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        time.sleep(1.5)
        assert holder.poll() is None, "the first setup should still be waiting for the app"
        result = results / "busy-result.json"
        setup("--app-pid", app.pid, "--quiet", "--relaunch", "--result-file", result,
              "--test-root", root, exe=second, expect=5)
        outcome = read_result(result)
        assert outcome["ok"] is False and "Another Jawjack setup is running" in outcome["message"], outcome
        setup("--uninstall", "--quiet", "--test-root", root, exe=second, expect=5)
        assert holder.wait(timeout=180) == 0, holder.communicate()
        assert app.poll() == 0
        assert installed() == "0.0.3"
        time.sleep(1)
        assert len(log_lines(log)) == 3, "a refused setup (or one without --relaunch) must start nothing"
        print(f"  result {result.read_text(encoding='utf-8')}")

        print("U6. An invalid --result-file stops the install (nothing changed, nothing written), still relaunches")
        app = dummy(install / "Jawjack.exe", 2)
        setup("--app-pid", app.pid, "--quiet", "--relaunch", "--result-file", "relative.json",
              "--test-root", root, exe=second, expect=1)
        assert app.poll() == 0
        assert wait_relaunch(log, 4)[-1] == "0.0.3 args=0"
        setup("--quiet", "--result-file", root / "missing" / "result.json", "--test-root", root,
              exe=second, expect=1)
        assert installed() == "0.0.3" and not (root / "missing").exists()
        assert not (root / "relative.json").exists() and not (pathlib.Path.cwd() / "relative.json").exists()

        print("U7. Uninstall with data removal")
        setup("--uninstall", "--quiet", "--remove-data", "--test-root", root, exe=second, expect=0)
        assert not install.exists() and not data.exists()
        print("PASS in-app updater contract (wait, result file, relaunch, lock)")
    finally:
        for process in dummies:  # They exit by themselves within seconds; never ended here.
            process.wait(timeout=60)
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    main()
