"""The crate layers hold: a crate depends only on crates in its own layer or one below it.

Layers, lowest first, are the folders under `crates/` (apps live under `apps/` and are above all
of them). See docs/architecture.md.
"""
import json
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ORDER = ["foundation", "editing", "io", "adapters", "services", "presentation", "apps"]


def layers_and_dependencies():
    metadata = json.loads(subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
        cwd=ROOT, capture_output=True, text=True, check=True).stdout)
    layer = {}
    for package in metadata["packages"]:
        relative = Path(package["manifest_path"]).parent.relative_to(ROOT).parts
        layer[package["name"]] = relative[1] if relative[0] == "crates" else "apps"
    edges = [
        (package["name"], dependency["name"])
        for package in metadata["packages"]
        for dependency in package["dependencies"]
        if dependency["name"] in layer and dependency.get("kind") is None  # normal, not dev
    ]
    return layer, edges


class Layers(unittest.TestCase):
    def test_every_crate_is_in_a_known_layer(self):
        layer, _ = layers_and_dependencies()
        self.assertEqual({name for name, where in layer.items() if where not in ORDER}, set())

    def test_no_crate_depends_on_a_layer_above_its_own(self):
        layer, edges = layers_and_dependencies()
        upward = [
            f"{a} ({layer[a]}) -> {b} ({layer[b]})"
            for a, b in edges
            if ORDER.index(layer[b]) > ORDER.index(layer[a])
        ]
        self.assertEqual(upward, [], "a crate must not depend on a layer above it")

    def test_apps_are_not_depended_on(self):
        layer, edges = layers_and_dependencies()
        self.assertEqual([(a, b) for a, b in edges if layer[b] == "apps"], [])


if __name__ == "__main__":
    unittest.main()
