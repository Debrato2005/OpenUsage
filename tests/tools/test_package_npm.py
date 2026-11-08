"""The npm assembly: pins, no install scripts, executables, SHA256SUMS and the launcher."""

import hashlib
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/package-npm.py"


def script(path: Path, body: str) -> Path:
    path.write_text(f"#!/bin/sh\n{body}\n")
    path.chmod(0o755)
    return path


class PackageNpmTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.dir = Path(self.temporary.name)
        bins = self.dir / "bins"
        bins.mkdir()
        self.cli = script(bins / "cli", 'echo "synthetic openusage $*"; [ "$1" = "fail" ] && exit 7; exit 0')
        self.desktop = script(bins / "desktop", "exit 0")
        self.ccusage = script(bins / "ccusage", "exit 0")
        self.codex = script(bins / "codex", "exit 0")
        digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
        self.pins = self.dir / "pins.json"
        self.pins.write_text(json.dumps({"ccusage": digest(self.ccusage), "codex": digest(self.codex)}))

    def tearDown(self):
        self.temporary.cleanup()

    def run_script(self, out: Path, pins: Path | None = None, extra: tuple[str, ...] = ()):
        command = [sys.executable, str(SCRIPT), "--version", "0.0.0-test", "--cli", str(self.cli), "--desktop", str(self.desktop),
                   "--ccusage", str(self.ccusage), "--codex", str(self.codex), "--out", str(out), "--pins", str(pins or self.pins), *extra]
        return subprocess.run(command, capture_output=True, text=True, timeout=600, check=False)

    def test_packages_have_no_scripts_keep_executables_and_the_launcher_runs_the_cli(self):
        out = self.dir / "out"
        bom = self.dir / "bom.json"
        bom.write_text('{"bomFormat": "CycloneDX"}\n')
        result = self.run_script(out, extra=("--sbom", str(bom)))
        self.assertEqual(result.returncode, 0, result.stderr)
        summary = json.loads(result.stdout)
        self.assertEqual(summary["tarballs"], ["openusage-linux-x64-0.0.0-test.tgz", "openusage-0.0.0-test.tgz"])
        sums = (out / "SHA256SUMS").read_text().splitlines()
        self.assertTrue(any(line.endswith("  openusage-0.0.0-test.tgz") for line in sums))
        bom_digest = hashlib.sha256(bom.read_bytes()).hexdigest()
        self.assertIn(f"{bom_digest}  openusage-0.0.0-test-linux-x64.cdx.json", sums)
        # Install the way npm does: each tarball's package folder in node_modules.
        modules = self.dir / "node_modules"
        for tarball, folder in ((out / summary["tarballs"][0], modules / "@openusage/linux-x64"), (out / summary["tarballs"][1], modules / "openusage")):
            with tarfile.open(tarball) as archive:
                manifest = json.load(archive.extractfile("package/package.json"))
                self.assertNotIn("scripts", manifest)
                folder.mkdir(parents=True)
                for member in archive.getmembers():
                    member.name = member.name.removeprefix("package/")
                    archive.extract(member, folder, filter="data")
        main = json.loads((modules / "openusage/package.json").read_text())
        self.assertEqual(main["optionalDependencies"], {"@openusage/linux-x64": "0.0.0-test"})
        platform = json.loads((modules / "@openusage/linux-x64/package.json").read_text())
        self.assertEqual((platform["os"], platform["cpu"]), (["linux"], ["x64"]))
        for name in ("openusage", "openusage-desktop", "openusage-ccusage", "openusage-codex"):
            self.assertTrue(os.access(modules / "@openusage/linux-x64/bin" / name, os.X_OK), name)
        for notice in ("LICENSE", "licenses/codex/LICENSE", "licenses/codex/NOTICE", "licenses/ccusage/LICENSE"):
            self.assertTrue((modules / "@openusage/linux-x64" / notice).is_file(), notice)
        launcher = modules / "openusage/bin/openusage.js"
        ran = subprocess.run(["node", str(launcher), "report", "--yes"], capture_output=True, text=True, timeout=60, check=False)
        self.assertEqual((ran.returncode, ran.stdout), (0, "synthetic openusage report --yes\n"))
        # The CLI exit code reaches the caller unchanged.
        self.assertEqual(subprocess.run(["node", str(launcher), "fail"], capture_output=True, timeout=60, check=False).returncode, 7)

    def test_a_binary_off_its_pin_or_a_used_folder_packs_nothing(self):
        wrong = self.dir / "wrong.json"
        wrong.write_text(json.dumps({"ccusage": "0" * 64, "codex": "0" * 64}))
        out = self.dir / "refused"
        result = self.run_script(out, wrong)
        self.assertEqual(result.returncode, 1)
        self.assertIn("does not match its pinned digest", result.stderr)
        self.assertFalse(out.exists() and any(out.iterdir()))
        used = self.dir / "used"
        used.mkdir()
        (used / "old.tgz").write_text("old")
        self.assertEqual(self.run_script(used).returncode, 1)

    def test_the_launcher_prints_the_fix_when_its_platform_package_is_missing(self):
        lone = self.dir / "lone/openusage/bin"
        lone.mkdir(parents=True)
        (lone / "openusage.js").write_text((ROOT / "packaging/npm/openusage/bin/openusage.js").read_text())
        ran = subprocess.run(["node", str(lone / "openusage.js")], capture_output=True, text=True, timeout=60, check=False)
        self.assertEqual(ran.returncode, 11)
        self.assertIn("Reinstall with: npm install -g openusage (install_incomplete)", ran.stderr)


if __name__ == "__main__":
    unittest.main()
