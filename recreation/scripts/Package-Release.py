"""Prepare a local Jawjack Windows x64 release. Never uploads or publishes.

Ported from Haiwire's .local-release/Package-Release.py. Builds (unless
--no-build) the release `chat-workbench.exe` and `jawjack-setup.exe`, lays out
one app folder (Jawjack.exe, README.txt, notes.md, THIRD_PARTY_NOTICES.txt)
and produces under release-out/<version>/:

  Jawjack-<v>-setup.exe          (+ .sha256)  per-user installer with the
                                              verified app folder appended
  Jawjack-<v>-x64-portable.zip   (+ .sha256)
  latest.json                    feed read by crates/desktop/src/updates.rs
  manifest.json, notes.md, PROVENANCE.txt, app/Jawjack/

The output folder for a version is never overwritten: delete it yourself or
bump the workspace version. Optional Authenticode signing runs only with
--sign-thumbprint. Python 3.11+, Windows.
"""
import argparse
import datetime
import hashlib
import json
import os
import pathlib
import re
import shutil
import struct
import subprocess
import tomllib
import zipfile
import zlib

ROOT = pathlib.Path(__file__).resolve().parents[1]  # recreation/
# Build-Release.ps1 -TargetDir builds elsewhere so a running dev build is untouched.
RELEASE = pathlib.Path(os.environ.get("JAWJACK_TARGET_DIR") or ROOT / "target") / "release"
OUTPUT_ROOT = ROOT / "release-out"
TOOLCHAIN = os.environ.get("JAWJACK_TOOLCHAIN", "1.99.0")
TRAILER_MAGIC = b"JJSETUP1"
PAYLOAD_MAGIC = b"JJPAYLD1"
MAX_ARTIFACT = 128 * 1024 * 1024
APP_EXE = "Jawjack.exe"
PACKAGES = ["chat-workbench", "jawjack-setup"]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: pathlib.Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def cargo(*args: str) -> list[str]:
    rustup = pathlib.Path(os.environ.get("USERPROFILE", pathlib.Path.home())) / ".cargo/bin/rustup.exe"
    if not rustup.is_file():
        raise ValueError(f"rustup was not found at {rustup}")
    return [str(rustup), "run", TOOLCHAIN, "cargo", *args]


def run_text(args, timeout=15, cwd=None) -> str:
    return subprocess.run([str(a) for a in args], check=True, capture_output=True,
                          text=True, timeout=timeout, cwd=cwd).stdout.strip()


def workspace_version() -> str:
    data = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return data["workspace"]["package"]["version"]


def build():
    """Release build of the app and setup into the normal target directory."""
    command = cargo("build", "--release", "--locked", *[f"-p={p}" for p in PACKAGES])
    print("Building:", " ".join(command[3:]))
    subprocess.run(command, check=True, cwd=ROOT, timeout=3 * 60 * 60)


def resolved_packages(roots: list[str]) -> list[dict]:
    """Packages reachable (normal + build edges) from `roots` in the locked
    Windows x64 graph of the workspace."""
    result = subprocess.run(cargo("metadata", "--locked", "--offline", "--format-version", "1",
                                  "--filter-platform", "x86_64-pc-windows-msvc",
                                  "--manifest-path", str(ROOT / "Cargo.toml")),
                            check=True, capture_output=True, encoding="utf-8", timeout=300)
    metadata = json.loads(result.stdout)
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    start = [pid for pid, package in packages.items() if package["name"] in roots and package["source"] is None]
    if len(start) != len(roots):
        raise ValueError(f"Could not find all root packages {roots}")
    seen, stack = set(), list(start)
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid]["deps"]:
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    return [packages[pid] for pid in seen]


def dependency_notices() -> str:
    own = {"chat-core", "chat-workbench", "jawjack-setup"}
    unique = {}
    for package in resolved_packages(PACKAGES):
        if package["name"] in own:
            continue  # Jawjack's own crates. Vendored path crates (gpui-pre) are third-party.
        unique[(package["name"], package["version"])] = package
    chunks = ["Third-party notices for Jawjack\n\nResolved Windows x64 build graphs of Jawjack.exe "
              "(crates/desktop) and the setup program\n(crates/setup), including build-time dependencies. "
              "License text is reproduced from the\ndistributed dependency sources. These licenses do not "
              "select a license for Jawjack.\n"]
    # Repository notices (adapted code, bundled dictionary).
    extras = sorted((ROOT / "licenses").glob("*")) + [ROOT / "crates/desktop/assets/dict/LICENSE-en_US.txt"]
    for path in extras:
        if path.is_file():
            chunks.append(f"\n{'=' * 72}\n{path.relative_to(ROOT).as_posix()}\n\n"
                          + path.read_text(encoding="utf-8", errors="replace") + "\n")
    for key in sorted(unique):
        package = unique[key]
        directory = pathlib.Path(package["manifest_path"]).parent
        chunks.append(f"\n{'=' * 72}\n{package['name']} {package['version']}\n"
                      f"Declared license: {package['license'] or 'unspecified'}\n")
        sources = sorted(p for p in directory.iterdir() if p.is_file()
                         and p.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "UNLICENSE")))
        for source in sources:
            if source.stat().st_size > 256 * 1024:
                raise ValueError(f"A dependency notice exceeded its size limit: {source}")
            chunks.append(f"\n--- {source.name} ---\n" + source.read_text(encoding="utf-8", errors="replace") + "\n")
        if not sources:
            chunks.append("The crate distribution provides the declared license expression without a root notice file.\n")
    notices = "".join(chunks)
    if len(notices.encode("utf-8")) > 16 * 1024 * 1024:
        raise ValueError("Dependency notices exceeded the bundle limit")
    return notices


def verify_inputs() -> dict[str, pathlib.Path]:
    files = {"app": RELEASE / "chat-workbench.exe", "setup": RELEASE / "jawjack-setup.exe"}
    for name, path in files.items():
        if not path.is_file():
            raise ValueError(f"Missing release input {name}: {path} (build without --no-build)")
        if path.read_bytes()[:2] != b"MZ":
            raise ValueError(f"{path} is not a Windows executable")
    return files


def find_signtool() -> pathlib.Path | None:
    configured = os.environ.get("JAWJACK_SIGNTOOL")
    if configured:
        return pathlib.Path(configured)
    kits = pathlib.Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "Windows Kits/10/bin"
    candidates = sorted(kits.glob("10.*/x64/signtool.exe"), reverse=True) if kits.is_dir() else []
    return candidates[0] if candidates else None


def sign(paths: list[pathlib.Path], thumbprint: str | None) -> bool:
    """Optional Authenticode hook. Skipped (False) without a configured certificate."""
    if not thumbprint:
        return False
    if not re.fullmatch(r"[0-9A-Fa-f]{40}", thumbprint):
        raise ValueError("--sign-thumbprint must be a 40-hex SHA-1 certificate thumbprint")
    signtool = find_signtool()
    if signtool is None or not signtool.is_file():
        raise ValueError("Signing was requested but signtool.exe was not found (set JAWJACK_SIGNTOOL)")
    timestamp = os.environ.get("JAWJACK_SIGN_TIMESTAMP_URL", "http://timestamp.digicert.com")
    for path in paths:
        subprocess.run([str(signtool), "sign", "/sha1", thumbprint, "/fd", "sha256",
                        "/tr", timestamp, "/td", "sha256", str(path)], check=True, timeout=180)
        subprocess.run([str(signtool), "verify", "/pa", str(path)], check=True, timeout=60)
    return True


def readme(version: str, signed: bool) -> str:
    signature = ("Its executables are Authenticode-signed." if signed else
                 "This build is unsigned: Windows SmartScreen may warn before it runs\n"
                 "(More info -> Run anyway). Compare the file's SHA-256 with the\n"
                 "published .sha256 file first.")
    return f"""Jawjack {version} - Windows x64

Install: run Jawjack-{version}-setup.exe. It installs for the current user
only (no administrator rights) to %LOCALAPPDATA%\\Programs\\Jawjack, adds a
Start Menu shortcut (Desktop optional) and an Apps & Features entry with an
uninstaller. Updating keeps your settings.

Portable: extract the ZIP and run Jawjack.exe.

Settings live in %LOCALAPPDATA%\\ChatWorkbench, not the program folder. Twitch
sign-ins are kept in Windows Credential Manager. If Windows reports missing
MSVC runtime DLLs, install Microsoft's Visual C++ Redistributable (x64).

{signature}

Release information: https://apps.zombie.digital/jawjack/releases
"""


def payload_bytes(version: str, entries: list[tuple[str, bytes]]) -> bytes:
    raw = bytearray(PAYLOAD_MAGIC)
    encoded = version.encode("ascii")
    raw += struct.pack("<H", len(encoded)) + encoded + struct.pack("<I", len(entries))
    for name, data in entries:
        name_bytes = name.encode("ascii")
        raw += struct.pack("<H", len(name_bytes)) + name_bytes + struct.pack("<Q", len(data))
        raw += hashlib.sha256(data).digest() + data
    return bytes(raw)


def build_setup(base: bytes, version: str, entries: list[tuple[str, bytes]]) -> bytes:
    """Layout read by crates/setup/src/payload.rs:
    [zlib stream] [0..=7 zero padding] [64-byte trailer]
    trailer = JJSETUP1 | u64 offset | u64 compressed_len | sha256(compressed) | u64 raw_len"""
    if base[:2] != b"MZ":
        raise ValueError("The setup base is not a Windows executable")
    raw = payload_bytes(version, entries)
    compressed = zlib.compress(raw, 9)
    offset = len(base)
    padding = (-(offset + len(compressed) + 64)) % 8
    trailer = TRAILER_MAGIC + struct.pack("<QQ", offset, len(compressed)) \
        + hashlib.sha256(compressed).digest() + struct.pack("<Q", len(raw))
    assert len(trailer) == 64
    return base + compressed + b"\0" * padding + trailer


def summary_of(notes: str) -> str:
    """The first prose paragraph of the notes (headings skipped), as plain text."""
    paragraph = []
    for line in notes.strip().splitlines():
        text = line.strip()
        if text.startswith("#") or not text:
            if paragraph:
                break
            continue
        paragraph.append(text.lstrip("-* ").replace("`", ""))
    return " ".join(paragraph).encode("utf-8")[:240].decode("utf-8", errors="ignore").strip()


def prepare(version: str, notes: str, notices: str, inputs: dict[str, pathlib.Path],
            output: pathlib.Path, thumbprint: str | None, provenance: str) -> dict:
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-(?:preview|beta)\.\d+)?", version):
        raise ValueError("Expected a stable, preview.N, or beta.N product version")
    if not notes.strip() or len(notes.encode("utf-8")) > 65536:
        raise ValueError("Supply nonempty release notes of at most 64 KiB")
    # Refuse overwrites, including a partly prepared version. Never clear a folder.
    output.mkdir(parents=True, exist_ok=False)
    app = output / "app" / "Jawjack"
    app.mkdir(parents=True)
    shutil.copyfile(inputs["app"], app / APP_EXE)
    signed = sign([app / APP_EXE], thumbprint)
    (app / "README.txt").write_text(readme(version, signed), encoding="utf-8", newline="\r\n")
    (app / "notes.md").write_text(notes, encoding="utf-8", newline="\n")
    (app / "THIRD_PARTY_NOTICES.txt").write_text(notices, encoding="utf-8", newline="\n")
    order = [APP_EXE, "README.txt", "notes.md", "THIRD_PARTY_NOTICES.txt"]
    entries = [(name, (app / name).read_bytes()) for name in order]

    portable = f"Jawjack-{version}-x64-portable.zip"
    with zipfile.ZipFile(output / portable, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, data in entries:
            archive.writestr(name, data)

    # Must equal installer_file_name() in crates/desktop/src/updates.rs.
    setup_name = f"Jawjack-{version}-setup.exe"
    base = inputs["setup"].read_bytes()
    (output / setup_name).write_bytes(build_setup(base, version, entries))
    sign([output / setup_name], thumbprint)
    # Read-only self-check: --payload-info prints the decoded payload and exits.
    info = json.loads(run_text([output / setup_name, "--payload-info"], timeout=60))
    expected = [{"name": name, "size": len(data), "sha256": sha256_bytes(data)} for name, data in entries]
    if info.get("version") != version or info.get("files") != expected or info.get("image_len") != len(base):
        raise ValueError("The assembled setup did not report the expected payload")

    artifacts = []
    for kind, name in (("setup", setup_name), ("portable", portable)):
        path = output / name
        size = path.stat().st_size
        if not 0 < size <= MAX_ARTIFACT:
            raise ValueError(f"{name} exceeds the 128 MiB artifact limit")
        digest = sha256_file(path)
        (output / (name + ".sha256")).write_text(f"{digest} {name}\n", encoding="utf-8", newline="\n")
        artifacts.append({"kind": kind, "file": name, "size": size, "sha256": digest})
    (output / "notes.md").write_text(notes, encoding="utf-8", newline="\n")
    generated = datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z")
    channel = "preview" if "-preview." in version else "beta" if "-beta." in version else "stable"
    setup = artifacts[0]
    # Schema read by crates/desktop/src/updates.rs (parse_release/parse_installer):
    # version (SemVer), channel (preview|beta|stable), setup {file, size, sha256}.
    # The app derives the download URL itself and ignores any URL here.
    latest = {"version": version, "channel": channel, "publishedAt": generated,
              "file": setup_name, "notes": summary_of(notes), "notesFile": "notes.md",
              "files": {"setup": setup_name, "portable": portable},
              "setup": {"file": setup_name, "size": setup["size"], "sha256": setup["sha256"]}}
    manifest = {"version": version, "generatedAt": generated, "artifacts": artifacts}
    for filename, data in (("latest.json", latest), ("manifest.json", manifest)):
        (output / filename).write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8", newline="\n")
    (output / "PROVENANCE.txt").write_text(
        provenance + f"\nSigned: {'yes' if signed else 'no (no certificate configured)'}\n",
        encoding="utf-8", newline="\n")
    return manifest


def source_description(source_commit: str | None) -> str:
    if source_commit:
        return f"committed HEAD {source_commit} (exported by Build-Release.ps1)"
    commit = run_text(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT)
    dirty = run_text(["git", "status", "--porcelain", "--", "."], cwd=ROOT)
    if dirty:
        print("WARNING: the working tree has uncommitted changes; this package includes them.\n"
              "         Use scripts/Build-Release.ps1 for a build of committed HEAD only.")
        return f"working tree at {commit} WITH UNCOMMITTED CHANGES"
    return f"clean working tree at {commit}"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--notes-file", required=True, type=pathlib.Path,
                        help="reviewed user-facing Markdown release notes")
    parser.add_argument("--no-build", action="store_true",
                        help="package the existing target/release binaries")
    parser.add_argument("--source-commit", help="set by Build-Release.ps1 for a HEAD export")
    parser.add_argument("--sign-thumbprint", default=os.environ.get("JAWJACK_SIGN_THUMBPRINT"))
    args = parser.parse_args()
    notes = args.notes_file.read_text(encoding="utf-8")
    version = workspace_version()
    output = OUTPUT_ROOT / version
    if output.exists():
        raise SystemExit(f"{output} already exists; delete it or bump the workspace version.")
    provenance = f"Jawjack {version}\nSource: {source_description(args.source_commit)}\nToolchain: {TOOLCHAIN}"
    if not args.no_build:
        build()
    inputs = verify_inputs()
    manifest = prepare(version, notes, dependency_notices(), inputs, output, args.sign_thumbprint, provenance)
    for artifact in manifest["artifacts"]:
        print(f"{artifact['file']}  {artifact['size']} bytes  sha256 {artifact['sha256']}")
    print(f"Prepared {output}; not uploaded or published.")


if __name__ == "__main__":
    main()
