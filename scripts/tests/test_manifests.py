"""The manifests follow the workspace rules: the root owns every version and path, a member only
says `workspace = true`, every member is listed, and nothing the root declares goes unused.

Run with `python3 -m unittest discover -s scripts/tests -p 'test_manifests.py'`.
"""
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def load(path):
    with open(path, "rb") as handle:
        return tomllib.load(handle)


def member_manifests():
    root = load(ROOT / "Cargo.toml")
    return root, {m: load(ROOT / m / "Cargo.toml") for m in root["workspace"]["members"]}


def dependency_tables(manifest):
    """Every dependency table of a manifest, including the target-specific ones."""
    for name in TABLES:
        if name in manifest:
            yield name, manifest[name]
    for target in manifest.get("target", {}).values():
        for name in TABLES:
            if name in target:
                yield name, target[name]


class Manifests(unittest.TestCase):
    def test_every_crate_and_app_on_disk_is_a_member_and_every_member_exists(self):
        root, _ = member_manifests()
        listed = set(root["workspace"]["members"])
        on_disk = {
            p.parent.relative_to(ROOT).as_posix()
            for pattern in ("apps/*/Cargo.toml", "crates/*/*/Cargo.toml")
            for p in ROOT.glob(pattern)
        }
        self.assertEqual(sorted(on_disk - listed), [], "on disk but not a member")
        self.assertEqual(sorted(listed - on_disk), [], "a member that is not there")

    def test_members_are_listed_by_layer_in_the_order_of_the_architecture(self):
        root, _ = member_manifests()
        order = ["apps", "foundation", "editing", "io", "adapters", "services", "presentation"]
        layers = [m.split("/")[1] if m.startswith("crates/") else "apps" for m in root["workspace"]["members"]]
        self.assertEqual(layers, sorted(layers, key=order.index))

    def test_a_member_never_names_a_version_or_a_path_the_root_owns(self):
        _, members = member_manifests()
        loose = []
        for name, manifest in members.items():
            for table, deps in dependency_tables(manifest):
                for dep, spec in deps.items():
                    if not (isinstance(spec, dict) and spec.get("workspace") is True):
                        loose.append(f"{name}: {dep} in [{table}] is {spec!r}")
        self.assertEqual(loose, [], "say `workspace = true` and declare it in the root")

    def test_every_dependency_a_member_uses_is_declared_in_the_root(self):
        root, members = member_manifests()
        declared = set(root["workspace"]["dependencies"])
        used = {
            (name, dep)
            for name, manifest in members.items()
            for _, deps in dependency_tables(manifest)
            for dep in deps
            if dep not in declared
        }
        self.assertEqual(sorted(used), [])

    def test_nothing_declared_in_the_root_goes_unused(self):
        root, members = member_manifests()
        used = {
            dep
            for manifest in members.values()
            for _, deps in dependency_tables(manifest)
            for dep in deps
        }
        unused = sorted(set(root["workspace"]["dependencies"]) - used)
        self.assertEqual(unused, [], "declared in the root and used by no member")

    def test_each_internal_package_points_at_the_crate_of_its_name(self):
        root, _ = member_manifests()
        wrong = []
        for dep, spec in root["workspace"]["dependencies"].items():
            if isinstance(spec, dict) and "path" in spec:
                manifest = ROOT / spec["path"] / "Cargo.toml"
                if not manifest.is_file() or load(manifest)["package"]["name"] != dep:
                    wrong.append(dep)
        self.assertEqual(wrong, [])


if __name__ == "__main__":
    unittest.main()
