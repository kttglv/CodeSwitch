"""Stage version-bound updater assets and prepare a reviewable GitHub draft release."""
import argparse
import base64
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "aarch64-apple-darwin": ("darwin-aarch64", "macos", "CodeSwitch.app.tar.gz", "aarch64.app.tar.gz"),
    "x86_64-apple-darwin": ("darwin-x86_64", "macos", "CodeSwitch.app.tar.gz", "x86_64.app.tar.gz"),
    "x86_64-pc-windows-msvc": ("windows-x86_64", "nsis", "CodeSwitch_{version}_x64-setup.exe", "x64-setup.exe"),
}


def valid_version(version):
    identifier = r"(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)"
    pattern = (r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)"
               + rf"(?:-{identifier}(?:\.{identifier})*)?"
               + r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?")
    if not re.fullmatch(pattern, version):
        raise ValueError("Invalid release version")
    return version


def version():
    value = valid_version(json.loads((ROOT / "src-tauri/tauri.conf.json").read_text())["version"])
    npm = json.loads((ROOT / "package.json").read_text())["version"]
    cargo = re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M).group(1)
    if npm != value or cargo != value:
        raise ValueError("Update the versions in tauri.conf.json, Cargo.toml and package.json together")
    return value


def signature(path, release_version):
    value = path.read_text().strip()
    try:
        lines = base64.b64decode(value, validate=True).decode("utf-8").splitlines()
        if len(lines) != 4 or not lines[2].startswith("trusted comment: "):
            raise ValueError("Invalid minisign structure")
        fields = lines[2].split("\t")
        if f"version:{release_version}" not in fields:
            raise ValueError("Signature is not bound to the release version")
        base64.b64decode(lines[1], validate=True)
        base64.b64decode(lines[3], validate=True)
    except (ValueError, UnicodeError) as error:
        raise ValueError(f"Invalid or unversioned updater signature: {path.name}") from error
    return value


def asset_name(target, release_version):
    valid_version(release_version)
    return f"CodeSwitch_{release_version}_{TARGETS[target][3]}"


def stage(target, output):
    value = version()
    _, folder, original, _ = TARGETS[target]
    bundle = ROOT / "target" / target / "release/bundle"
    source = bundle / folder / original.format(version=value)
    signature(source.with_name(source.name + ".sig"), value)
    output.mkdir(parents=True, exist_ok=True)
    dest = output / asset_name(target, value)
    shutil.copy2(source, dest)
    shutil.copy2(source.with_name(source.name + ".sig"), dest.with_name(dest.name + ".sig"))
    if folder == "macos":
        for dmg in (bundle / "dmg").glob("*.dmg"):
            shutil.copy2(dmg, output / dmg.name)
    notices = ROOT / "artifacts/dependency-notices.json"
    shutil.copy2(notices, output / f"dependency-notices-{target}.json")


def manifest(directory, release_version, repository):
    valid_version(release_version)
    if repository != "kttglv/CodeSwitch":
        raise ValueError("The repository must match the updater's pinned download origin")
    platforms = {}
    for target, (platform, _, _, _) in TARGETS.items():
        name = asset_name(target, release_version)
        if not (directory / name).is_file():
            raise ValueError(f"Missing updater package: {name}")
        platforms[platform] = {
            "signature": signature(directory / (name + ".sig"), release_version),
            "url": f"https://github.com/{repository}/releases/download/v{quote(release_version, safe='')}/{quote(name, safe='')}",
        }
    return {"version": release_version,
            "notes": (ROOT / "release/notes.md").read_text().strip(),
            "pub_date": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
            "platforms": platforms}


def create_draft(directory, release_version, repository, commit):
    valid_version(release_version)
    if repository != "kttglv/CodeSwitch":
        raise ValueError("Untrusted release repository")
    if not re.fullmatch(r"[0-9a-f]{40}", commit or ""):
        raise ValueError("A reviewed source commit is required for a draft release")
    tag = f"v{release_version}"
    existing = subprocess.run(["gh", "api", f"repos/{repository}/git/ref/tags/{tag}", "--silent"],
                              capture_output=True, text=True)
    if existing.returncode == 0:
        raise ValueError("Release tag already exists; use a new version instead of replacing assets")
    if "(HTTP 404)" not in existing.stderr:
        raise ValueError("Could not verify that the release tag is unused")
    subprocess.run(["gh", "release", "create", tag, "--repo", repository,
                    "--target", commit, "--draft", "--title", f"CodeSwitch {release_version}",
                    "--notes-file", str(ROOT / "release/notes.md"),
                    *map(str, sorted(directory.iterdir()))], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["stage", "manifest", "draft"])
    parser.add_argument("--target", choices=TARGETS)
    parser.add_argument("--directory", type=Path, default=ROOT / "artifacts/updater")
    parser.add_argument("--repository", default="kttglv/CodeSwitch")
    parser.add_argument("--commit", default=os.environ.get("GITHUB_SHA"))
    args = parser.parse_args()
    if args.action == "stage":
        if not args.target: parser.error("stage requires --target")
        stage(args.target, args.directory)
        return
    value = version()
    data = manifest(args.directory, value, args.repository)
    output = args.directory / "latest.json"
    output.write_text(json.dumps(data, indent=2) + "\n")
    if args.action == "draft":
        create_draft(args.directory, value, args.repository, args.commit)


if __name__ == "__main__":
    main()
