#!/usr/bin/env python3
"""Write the CycloneDX 1.5 SBOM of a Linux release (OU-SEC-016, part of OU-DIST-005).

It lists the Rust crates of the CLI and the desktop app for one target, the production
npm packages that Vite bundles into the renderer, and the pinned `ccusage` and Codex
binaries. A crate that only runs at build time gets the scope `excluded`. The output is
deterministic: the time comes from SOURCE_DATE_EPOCH or the HEAD commit, and the serial
number comes from the components. It reads only lockfiles, the local Cargo cache and
`node_modules`, and downloads nothing.
"""

import argparse
import base64
import json
import os
import re
import subprocess
import sys
import tomllib
import uuid
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-unknown-linux-gnu"
PACKAGES = ("openusage-desktop", "openusage-cli")
# A build dependency whose C source is compiled into the binary, not only run at build time.
LINKED_BUILD_SOURCES = {"openssl-src"}
PINS = {
    "ccusage": ROOT / "crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json",
    "codex": ROOT / "crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json",
}
TREE_LINE = re.compile(r"^(?P<name>[A-Za-z0-9_-]+) v(?P<version>[^ |]+)(?P<marks>[^|]*)\|(?P<license>.*)$")


class SbomError(Exception):
    pass


def spdx(license_text: str) -> list[dict]:
    """Cargo still accepts the old `MIT/Apache-2.0` form. SPDX writes it with OR."""
    text = license_text.strip()
    if not text:
        return []
    return [{"expression": " OR ".join(part.strip() for part in text.split("/"))}]


def cargo_components(linked: list[str], everything: list[str], lock: dict) -> list[dict]:
    checksums = {(p["name"], p["version"]): p.get("checksum") for p in lock["package"]}
    linked_keys = set()
    for line in linked:
        match = TREE_LINE.match(line)
        if match:
            linked_keys.add((match["name"], match["version"]))
    components = {}
    for line in everything:
        match = TREE_LINE.match(line)
        if not match:
            raise SbomError(f"unexpected cargo tree line: {line[:80]}")
        name, version = match["name"], match["version"]
        paths = [mark for mark in re.findall(r"\(([^)]+)\)", match["marks"]) if mark.startswith("/")]
        properties = []
        if paths:
            path = Path(paths[0])
            if path.is_relative_to(ROOT) and not path.is_relative_to(ROOT / "vendor"):
                continue  # A workspace member is part of OpenUsage itself.
            properties.append({"name": "openusage:source", "value": f"local patch in {path.relative_to(ROOT)}"})
        component = {
            "type": "library",
            "bom-ref": f"pkg:cargo/{name}@{version}",
            "name": name,
            "version": version,
            "purl": f"pkg:cargo/{name}@{version}",
            "scope": "required" if (name, version) in linked_keys or name in LINKED_BUILD_SOURCES else "excluded",
        }
        if checksum := checksums.get((name, version)):
            component["hashes"] = [{"alg": "SHA-256", "content": checksum}]
        if licenses := spdx(match["license"]):
            component["licenses"] = licenses
        if properties:
            component["properties"] = properties
        components[component["bom-ref"]] = component
    return list(components.values())


def parse_bun_lock(text: str) -> dict:
    # bun.lock is JSON with trailing commas.
    return json.loads(re.sub(r",(\s*[}\]])", r"\1", text))


def npm_purl(name: str, version: str) -> str:
    return f"pkg:npm/{name.replace('@', '%40', 1) if name.startswith('@') else name}@{version}"


def npm_components(lock: dict, modules: Path) -> list[dict]:
    """Walk the production dependencies the way Node resolves them: the nearest nested copy first."""
    packages = lock["packages"]

    def resolve(path: list[str], name: str) -> list[str] | None:
        for depth in range(len(path), -1, -1):
            candidate = path[:depth] + [name]
            if "/".join(candidate) in packages:
                return candidate
        return None

    components, seen = {}, set()
    pending = [[name] for name in lock["workspaces"][""].get("dependencies", {})]
    while pending:
        path = pending.pop()
        key = "/".join(path)
        if key in seen:
            continue
        seen.add(key)
        if key not in packages:
            raise SbomError(f"bun.lock has no entry for {key}")
        ident, _, meta, integrity = packages[key][:4]
        name, _, version = ident.rpartition("@")
        component = {
            "type": "library",
            "bom-ref": npm_purl(name, version),
            "name": name,
            "version": version,
            "purl": npm_purl(name, version),
            "scope": "required",
        }
        algorithm, _, digest = integrity.partition("-")
        if algorithm == "sha512":
            component["hashes"] = [{"alg": "SHA-512", "content": base64.b64decode(digest).hex()}]
        nested = [part for package in path for part in ("node_modules", package)][1:]
        manifest = modules.joinpath(*nested) / "package.json"
        if manifest.is_file():
            license_text = json.loads(manifest.read_text()).get("license")
            if isinstance(license_text, str):
                component["licenses"] = spdx(license_text)
        components[component["bom-ref"]] = component
        for field in ("dependencies", "optionalDependencies", "peerDependencies"):
            for dependency in meta.get(field, {}):
                found = resolve(path, dependency)
                if found is None and field == "dependencies":
                    raise SbomError(f"{key} needs {dependency}, which bun.lock does not hold")
                if found is not None:
                    pending.append(found)
    return list(components.values())


def binary_components(pins: dict[str, Path]) -> list[dict]:
    ccusage = json.loads(pins["ccusage"].read_text())
    codex = json.loads(pins["codex"].read_text())
    return [
        {
            "type": "application",
            "bom-ref": npm_purl(ccusage["package"], ccusage["version"]),
            "name": ccusage["package"],
            "version": ccusage["version"],
            "purl": npm_purl(ccusage["package"], ccusage["version"]),
            "scope": "required",
            "hashes": [{"alg": "SHA-256", "content": ccusage["executableSha256"]}],
            "licenses": spdx(ccusage["license"]),
            "properties": [{"name": "openusage:installed-as", "value": "openusage-ccusage"}],
        },
        {
            "type": "application",
            "bom-ref": f"pkg:github/openai/codex@rust-v{codex['version']}",
            "name": "codex",
            "version": codex["version"],
            "purl": f"pkg:github/openai/codex@rust-v{codex['version']}",
            "scope": "required",
            "hashes": [{"alg": "SHA-256", "content": codex["executableSha256"]}],
            "licenses": [{"expression": "Apache-2.0"}],
            "properties": [
                {"name": "openusage:installed-as", "value": "openusage-codex"},
                {"name": "openusage:note", "value": "The official binary. Its own dependencies are not listed here."},
            ],
        },
    ]


def document(version: str, timestamp: int, components: list[dict]) -> dict:
    components = sorted(components, key=lambda c: c["bom-ref"])
    serial = uuid.uuid5(uuid.NAMESPACE_URL, json.dumps([version, components], sort_keys=True))
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{serial}",
        "version": 1,
        "metadata": {
            "timestamp": datetime.fromtimestamp(timestamp, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "tools": {"components": [{"type": "application", "name": "openusage scripts/sbom.py"}]},
            "component": {
                "type": "application",
                "bom-ref": npm_purl("openusage", version),
                "name": "openusage",
                "version": version,
                "purl": npm_purl("openusage", version),
                "licenses": [{"expression": "Apache-2.0"}],
            },
            "properties": [{"name": "openusage:target", "value": TARGET}],
        },
        "components": components,
    }


def cargo_tree(edges: str) -> list[str]:
    command = ["cargo", "tree", "--locked", "--offline", "-e", edges, "--target", TARGET, "--prefix", "none", "--format", "{p}|{l}"]
    for package in PACKAGES:
        command += ["-p", package]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=600, check=False)
    if result.returncode != 0:
        raise SbomError("cargo tree failed")
    return sorted({line.removesuffix(" (*)") for line in result.stdout.splitlines() if line.strip()})


def source_time() -> int:
    if epoch := os.environ.get("SOURCE_DATE_EPOCH"):
        return int(epoch)
    result = subprocess.run(["git", "log", "-1", "--format=%ct"], cwd=ROOT, capture_output=True, text=True, timeout=60, check=True)
    return int(result.stdout.strip())


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        version = json.loads((ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text())["version"]
        modules = ROOT / "node_modules"
        if not modules.is_dir():
            raise SbomError("node_modules is missing. Install the frontend dependencies first")
        components = cargo_components(
            cargo_tree("normal,no-proc-macro"),
            cargo_tree("normal,build"),
            tomllib.loads((ROOT / "Cargo.lock").read_text()),
        )
        components += npm_components(parse_bun_lock((ROOT / "bun.lock").read_text()), modules)
        components += binary_components(PINS)
        bom = document(version, source_time(), components)
        args.out.write_text(json.dumps(bom, indent=2) + "\n")
    except (SbomError, OSError, KeyError, ValueError, subprocess.SubprocessError) as error:
        print(f"sbom: {error}", file=sys.stderr)
        return 1
    scopes = [c.get("scope") for c in bom["components"]]
    print(json.dumps({"components": len(scopes), "required": scopes.count("required"), "excluded": scopes.count("excluded")}))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
