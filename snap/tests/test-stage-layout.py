#!/usr/bin/env python3
"""Exercise real craft-parts organize semantics, not a Snapcraft build.

Run with: uv run --with craft-parts --with pyyaml python3 snap/tests/test-stage-layout.py
All payloads are labelled text path fixtures in a deleted temporary directory;
these are NOT runtime libraries, ELF validation evidence or release artifacts.
"""
from pathlib import Path
import shutil
import tempfile
import unittest

import yaml
from craft_parts.executor.organize import organize_files

ROOT = Path(__file__).resolve().parents[2]
RUNTIME_PATHS = {
    "pulseaudio-utils": [
        "usr/bin/pactl",
        "usr/lib/x86_64-linux-gnu/libpulse.so.0",
        "usr/lib/x86_64-linux-gnu/pulseaudio/libpulsecommon.so",
    ],
    "libpipewire-0.3-0t64": ["usr/lib/x86_64-linux-gnu/libpipewire-0.3.so.0"],
    "libudev1": ["usr/lib/x86_64-linux-gnu/libudev.so.1"],
    "libnss3": ["usr/lib/x86_64-linux-gnu/libnss3.so"],
    "libnspr4": ["usr/lib/x86_64-linux-gnu/libnspr4.so"],
    "libasound2t64": ["usr/lib/x86_64-linux-gnu/libasound.so.2"],
}


class StageLayoutTest(unittest.TestCase):
    def test_runtime_paths_survive_application_organize(self):
        recipe = yaml.safe_load((ROOT / "snap/snapcraft.yaml").read_text())
        with tempfile.TemporaryDirectory(prefix="ssgg-organize-fixture-") as temp:
            temp = Path(temp)
            stage = temp / "stage"
            packages = set()
            for name, part in recipe["parts"].items():
                install = temp / name / "install"
                install.mkdir(parents=True)
                paths = []
                if name == "application":
                    paths += ["ssgg-gui", "resources/ssgg-desktop", "libffmpeg.so"]
                for package in part.get("stage-packages", []):
                    packages.add(package)
                    paths += RUNTIME_PATHS.get(package, [])
                for path in paths:
                    fixture = install / path
                    fixture.parent.mkdir(parents=True, exist_ok=True)
                    fixture.write_text(f"SSGG path-only test fixture: {path}\n")
                organize_files(
                    part_name=name,
                    file_map=part.get("organize", {}) if paths else {},
                    install_dir_map={None: install},
                    overwrite=False,
                    default_partition="default",
                )
                shutil.copytree(install, stage, dirs_exist_ok=True)

            actual = sorted(str(p.relative_to(stage)) for p in stage.rglob("*") if p.is_file())
            for paths in RUNTIME_PATHS.values():
                for path in paths:
                    self.assertIn(path, actual, f"runtime path relocated; staged paths: {actual}")
                    self.assertFalse((stage / "app" / path).exists())
            for path in ["ssgg-gui", "resources/ssgg-desktop", "libffmpeg.so"]:
                self.assertTrue((stage / "app" / path).is_file())
            self.assertEqual(packages, set(RUNTIME_PATHS))
            self.assertNotIn("stage-packages", recipe["parts"]["application"])
            self.assertEqual(recipe["parts"]["runtime"]["plugin"], "nil")
            self.assertNotIn("organize", recipe["parts"]["runtime"])


if __name__ == "__main__":
    unittest.main()
