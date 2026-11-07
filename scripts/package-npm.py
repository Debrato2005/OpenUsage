#!/usr/bin/env python3
"""Assemble the npm packages for one platform (SRS Section 16A, L-6b).

The main `openusage` package holds only a launcher. The platform package holds the CLI,
the desktop app, the pinned Codex companion, the pinned ccusage and their notices
(OU-DIST-003). Neither package has an install script, and nothing downloads at install
time or at run time. The script writes SHA256SUMS for every packed file (OU-DIST-005).
It never publishes: the owner publishes with npm and provenance.
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LAUNCHER = ROOT / "packaging/npm/openusage"
REPOSITORY = {"type": "git", "url": "git+https://github.com/Debrato2005/OpenUsage.git"}
# Rust target: npm suffix, npm os, npm cpu, executable suffix, ccusage pin file, Codex pin file.
TARGETS = {
    "x86_64-unknown-linux-gnu": (
        "linux-x64",
        "linux",
        "x64",
        "",
        "crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json",
        "crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json",
    ),
}


class PackageError(Exception):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def pins_for(target: str, pins: Path | None) -> dict:
    if pins is not None:
        return json.loads(pins.read_text())
    *_, ccusage, codex = TARGETS[target]
    return {
        "ccusage": json.loads((ROOT / ccusage).read_text())["executableSha256"],
        "codex": json.loads((ROOT / codex).read_text())["executableSha256"],
    }


def write_json(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n")


def pack(directory: Path, destination: Path, cache: Path) -> Path:
    # npm pack reads only the folder. A private cache keeps the user's npm cache untouched.
    result = subprocess.run(
        ["npm", "pack", "--json", "--ignore-scripts", "--offline", "--cache", str(cache), "--pack-destination", str(destination)],
        cwd=directory,
        capture_output=True,
        text=True,
        timeout=600,
        check=False,
    )
    if result.returncode != 0:
        raise PackageError(f"npm pack failed in {directory.name}")
    return destination / json.loads(result.stdout)[0]["filename"]


def check_tarball(tarball: Path, executables: set[str]) -> None:
    """A package must have no install script, and each binary must stay executable."""
    with tarfile.open(tarball) as archive:
        members = {member.name: member for member in archive.getmembers()}
        manifest = json.load(archive.extractfile("package/package.json"))
    if "scripts" in manifest:
        raise PackageError(f"{tarball.name} has scripts")
    for name in executables:
        member = members.get(f"package/{name}")
        if member is None or not member.mode & 0o111:
            raise PackageError(f"{tarball.name} lacks an executable {name}")


def assemble(args: argparse.Namespace) -> dict:
    if args.target not in TARGETS:
        raise PackageError(f"{args.target} has no package yet")
    npm_name, npm_os, npm_cpu, suffix, *_ = TARGETS[args.target]
    pins = pins_for(args.target, args.pins)
    for key, path in (("ccusage", args.ccusage), ("codex", args.codex)):
        if sha256(path) != pins[key]:
            raise PackageError(f"{key} does not match its pinned digest, so it is not packed")
    version = args.version or json.loads((ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text())["version"]
    out = args.out
    if out.exists() and any(out.iterdir()):
        raise PackageError(f"{out} is not empty")
    out.mkdir(parents=True, exist_ok=True)
    platform = out / npm_name
    (platform / "bin").mkdir(parents=True)
    binaries = {
        "openusage": args.cli,
        "openusage-desktop": args.desktop,
        "openusage-ccusage": args.ccusage,
        "openusage-codex": args.codex,
    }
    for name, source in binaries.items():
        destination = platform / "bin" / f"{name}{suffix}"
        shutil.copyfile(source, destination)
        os.chmod(destination, 0o755)
    for source, destination in (
        (ROOT / "LICENSE", platform / "LICENSE"),
        (ROOT / "apps/desktop/src-tauri/companion/licenses/LICENSE", platform / "licenses/codex/LICENSE"),
        (ROOT / "apps/desktop/src-tauri/companion/licenses/NOTICE", platform / "licenses/codex/NOTICE"),
        (ROOT / "packaging/licenses/ccusage/LICENSE", platform / "licenses/ccusage/LICENSE"),
    ):
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    write_json(platform / "package.json", {
        "name": f"@openusage/{npm_name}",
        "version": version,
        "description": f"OpenUsage binaries for {npm_os} {npm_cpu}",
        "license": "Apache-2.0",
        "repository": REPOSITORY,
        "os": [npm_os],
        "cpu": [npm_cpu],
        "files": ["bin", "licenses", "LICENSE"],
    })
    main = out / "openusage"
    (main / "bin").mkdir(parents=True)
    shutil.copyfile(LAUNCHER / "bin/openusage.js", main / "bin/openusage.js")
    os.chmod(main / "bin/openusage.js", 0o755)
    shutil.copyfile(LAUNCHER / "README.md", main / "README.md")
    shutil.copyfile(ROOT / "LICENSE", main / "LICENSE")
    write_json(main / "package.json", {
        "name": "openusage",
        "version": version,
        "description": "See what you used and what you have left across your AI coding tools",
        "license": "Apache-2.0",
        "repository": REPOSITORY,
        "bin": {"openusage": "bin/openusage.js"},
        "files": ["bin", "README.md", "LICENSE"],
        "engines": {"node": ">=18"},
        "optionalDependencies": {f"@openusage/{npm_name}": version},
    })
    with tempfile.TemporaryDirectory() as cache:
        tarballs = [pack(platform, out, Path(cache)), pack(main, out, Path(cache))]
    check_tarball(tarballs[0], {f"bin/{name}{suffix}" for name in binaries})
    check_tarball(tarballs[1], {"bin/openusage.js"})
    sums = [f"{sha256(path)}  {path.relative_to(out)}" for path in sorted(p for p in out.rglob("*") if p.is_file())]
    (out / "SHA256SUMS").write_text("\n".join(sums) + "\n")
    return {"version": version, "target": args.target, "tarballs": [t.name for t in tarballs], "files": len(sums)}


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--target", default="x86_64-unknown-linux-gnu")
    parser.add_argument("--version")
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--desktop", type=Path, required=True)
    parser.add_argument("--ccusage", type=Path, required=True)
    parser.add_argument("--codex", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--pins", type=Path, help="A JSON file of test digests. Releases use the pinned source records.")
    args = parser.parse_args(argv)
    try:
        print(json.dumps(assemble(args)))
    except (PackageError, OSError, KeyError) as error:
        print(f"package-npm: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
