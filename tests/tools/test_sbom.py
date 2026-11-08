"""The SBOM: crate scopes, the vendored patch, npm resolution, hashes and a stable document."""

import base64
import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("sbom", ROOT / "scripts/sbom.py")
sbom = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sbom)

LOCK = {"package": [
    {"name": "serde", "version": "1.0.0", "checksum": "a" * 64},
    {"name": "cc", "version": "1.2.0", "checksum": "b" * 64},
    {"name": "openssl-src", "version": "300.0.0", "checksum": "c" * 64},
    {"name": "glib", "version": "0.18.5"},
]}
LINKED = ["serde v1.0.0|MIT/Apache-2.0", f"glib v0.18.5 ({ROOT}/vendor/glib-0.18.5)|MIT", f"openusage-cli v0.1.0 ({ROOT}/crates/openusage-cli)|Apache-2.0"]
EVERYTHING = LINKED + ["cc v1.2.0|MIT OR Apache-2.0", "openssl-src v300.0.0|MIT/Apache-2.0", "serde_derive v1.0.0 (proc-macro)|MIT"]


def integrity(text: str) -> str:
    return "sha512-" + base64.b64encode(hashlib.sha512(text.encode()).digest()).decode()


BUN_LOCK = f"""{{
  "lockfileVersion": 1,
  "workspaces": {{
    "": {{
      "name": "openusage",
      "dependencies": {{ "@scope/app": "1.0.0", "b": "1.0.0", }},
      "devDependencies": {{ "vite": "8.0.0", }},
    }},
  }},
  "packages": {{
    "@scope/app": ["@scope/app@1.0.0", "", {{ "dependencies": {{ "b": "^2.0.0" }}, "peerDependencies": {{ "react": "*" }} }}, "{integrity('app')}"],
    "@scope/app/b": ["b@2.0.0", "", {{}}, "{integrity('b2')}"],
    "b": ["b@1.0.0", "", {{}}, "{integrity('b1')}"],
    "vite": ["vite@8.0.0", "", {{}}, "{integrity('vite')}"],
  }}
}}
"""


class SbomTests(unittest.TestCase):
    def test_crates_get_scopes_hashes_spdx_and_the_patch_note(self):
        components = {c["name"]: c for c in sbom.cargo_components(LINKED, EVERYTHING, LOCK)}
        self.assertNotIn("openusage-cli", components, "A workspace member is OpenUsage itself.")
        self.assertEqual(components["serde"]["scope"], "required")
        self.assertEqual(components["serde"]["licenses"], [{"expression": "MIT OR Apache-2.0"}])
        self.assertEqual(components["serde"]["hashes"], [{"alg": "SHA-256", "content": "a" * 64}])
        self.assertEqual(components["cc"]["scope"], "excluded", "A build tool is not in the binary.")
        self.assertEqual(components["serde_derive"]["scope"], "excluded")
        self.assertEqual(components["openssl-src"]["scope"], "required", "Its C source is linked.")
        self.assertNotIn("hashes", components["glib"])
        self.assertEqual(components["glib"]["properties"], [{"name": "openusage:source", "value": "local patch in vendor/glib-0.18.5"}])
        with self.assertRaises(sbom.SbomError):
            sbom.cargo_components(LINKED, ["not a tree line"], LOCK)

    def test_npm_walk_takes_the_nearest_copy_and_skips_dev_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            modules = Path(directory)
            nested = modules / "@scope/app/node_modules/b"
            nested.mkdir(parents=True)
            (nested / "package.json").write_text(json.dumps({"license": "ISC"}))
            components = {c["purl"]: c for c in sbom.npm_components(sbom.parse_bun_lock(BUN_LOCK), modules)}
        self.assertEqual(sorted(components), ["pkg:npm/%40scope/app@1.0.0", "pkg:npm/b@1.0.0", "pkg:npm/b@2.0.0"])
        self.assertEqual(components["pkg:npm/b@2.0.0"]["licenses"], [{"expression": "ISC"}])
        self.assertNotIn("licenses", components["pkg:npm/b@1.0.0"])
        self.assertEqual(components["pkg:npm/b@1.0.0"]["hashes"], [{"alg": "SHA-512", "content": hashlib.sha512(b"b1").hexdigest()}])
        broken = BUN_LOCK.replace('"b": ["b@1.0.0", "", {}, ', '"c": ["c@1.0.0", "", {}, ')
        with self.assertRaises(sbom.SbomError):
            sbom.npm_components(sbom.parse_bun_lock(broken), Path("/nonexistent"))

    def test_the_document_is_stable_and_names_the_pinned_binaries(self):
        components = sbom.cargo_components(LINKED, EVERYTHING, LOCK) + sbom.binary_components(sbom.PINS)
        first = sbom.document("0.1.0", 1_700_000_000, components)
        second = sbom.document("0.1.0", 1_700_000_000, list(reversed(components)))
        self.assertEqual(json.dumps(first), json.dumps(second))
        self.assertEqual((first["bomFormat"], first["specVersion"]), ("CycloneDX", "1.5"))
        self.assertRegex(first["serialNumber"], r"^urn:uuid:[0-9a-f-]{36}$")
        self.assertEqual(first["metadata"]["timestamp"], "2023-11-14T22:13:20Z")
        binaries = {c["name"]: c for c in first["components"] if c["type"] == "application"}
        ccusage = json.loads(sbom.PINS["ccusage"].read_text())
        self.assertEqual(binaries["@ccusage/ccusage-linux-x64"]["hashes"][0]["content"], ccusage["executableSha256"])
        self.assertEqual(binaries["codex"]["purl"], "pkg:github/openai/codex@rust-v0.163.0")
        self.assertNotEqual(first["serialNumber"], sbom.document("0.1.1", 1_700_000_000, components)["serialNumber"])


if __name__ == "__main__":
    unittest.main()
