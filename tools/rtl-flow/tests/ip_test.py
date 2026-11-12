"""Exercise resource transport; source discovery uses native Veryl metadata."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


TOOL = Path(__file__).resolve().parents[1] / "ip" / "bundle.py"
RUNTIME = TOOL.with_name("runtime.tcl")


class PackageResources(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.project = self.root / "project"
        self.project.mkdir()
        (self.project / "Veryl.toml").write_text('[project]\nname="top"\nversion="0.1.0"\n')
        self.metadata = {
            "format_version": 2,
            "root": {"name": "top", "local_path": str(self.project), "metadata": {}},
            "dependencies": [],
        }

    def package(self, name, value):
        directory = self.root / name
        (directory / "constraints").mkdir(parents=True)
        (directory / "src").mkdir()
        (directory / "Veryl.toml").write_text(
            f'[project]\nname="{name}"\nversion="0.1.0"\n[build]\nsources = ["src"]\n'
        )
        (directory / "src" / "cell.veryl").write_text(f"// source {value}\n")
        (directory / "constraints" / "helper.tcl").write_text(
            f"proc validate {{spec}} {{return {value}}}\n"
        )
        (directory / "constraints" / "ip.tcl").write_text(
            'source [file join [file dirname [info script]] helper.tcl]\n'
            'return [dict create validate [namespace current]::validate]\n'
        )
        self.metadata["dependencies"].append({
            "id": f"dep:{name}", "name": name, "project": "same_project",
            "local_path": str(directory), "dependencies": [],
            "source": {"kind": "path", "path": str(directory)},
            "metadata": {"rtl_flow": {"entry": "constraints/ip.tcl"}},
        })
        return directory

    def invoke(self, mode="bundle", success=True):
        source = self.root / "metadata.json"
        source.write_text(json.dumps(self.metadata))
        command = [sys.executable, str(TOOL), mode, str(source)]
        if mode == "bundle":
            command.append(str(self.root / "output"))
        result = subprocess.run(command, capture_output=True, text=True)
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("IP_RESOURCE:", result.stderr)
        return result

    def test_bundle_is_relocatable_and_isolates_two_versions(self):
        self.package("cells_v1", 17)
        self.package("cells_v2", 29)
        self.invoke()
        moved = self.root / "relocated resources"
        (self.root / "output").rename(moved)
        for dependency in self.metadata["dependencies"]:
            Path(dependency["local_path"]).rename(Path(dependency["local_path"] + "-gone"))
        script = self.root / "check.tcl"
        script.write_text(
            f'source {{{RUNTIME}}}\nrtl_ip::load {{{moved / "ip-manifest.tcl"}}}\n'
            'puts [rtl_ip::invoke cells_v1 validate {}]\n'
            'puts [rtl_ip::invoke cells_v2 validate {}]\n'
        )
        result = subprocess.run(["tclsh", str(script)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["17", "29"])

    def test_reject_missing_declared_resource(self):
        directory = self.package("cells", 1)
        (directory / "constraints" / "ip.tcl").unlink()
        self.invoke(success=False)

    def test_reject_resource_escape(self):
        self.package("cells", 1)
        self.metadata["dependencies"][0]["metadata"]["rtl_flow"]["entry"] = "../outside.tcl"
        self.invoke(success=False)

    def test_reject_symlink_escape(self):
        directory = self.package("cells", 1)
        secret = self.root / "external.txt"
        secret.write_text("not a package resource")
        (directory / "constraints" / "outside.tcl").symlink_to(secret)
        self.invoke(success=False)

    def test_reject_duplicate_provider_names(self):
        self.package("cells", 1)
        self.metadata["dependencies"].append(self.metadata["dependencies"][0])
        self.invoke(success=False)

    def test_inputs_include_resources_and_declared_hardware_sources(self):
        directory = self.package("cells", 1)
        (directory / "README.md").write_text("not a compiler input")
        first = self.invoke("inputs").stdout
        self.assertIn("cell.veryl", first)
        self.assertIn("helper.tcl", first)
        self.assertNotIn("README.md", first)
        (directory / "constraints" / "helper.tcl").write_text("changed resource\n")
        self.assertNotEqual(self.invoke("inputs").stdout, first)

    def test_existing_bundle_is_never_overwritten(self):
        self.package("cells", 1)
        self.invoke()
        self.invoke(success=False)

    def test_opted_in_checker_file_is_copied_and_locatable(self):
        directory = self.package("cells", 1)
        (directory / "verification").mkdir()
        (directory / "verification" / "checker.sv").write_text("// independent checker\n")
        self.metadata["dependencies"][0]["metadata"]["rtl_flow"]["files"] = ["verification/checker.sv"]
        self.invoke()
        result = subprocess.run([sys.executable, str(TOOL), "locate", str(self.root / "output"),
                                 "cells", "verification/checker.sv"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(result.stdout.strip()).read_text(), "// independent checker\n")

    def test_derived_snapshot_keeps_resources_and_rejects_replacement(self):
        self.package("cells", 1)
        self.invoke()
        target = self.root / "derived"
        target.mkdir()
        command = [sys.executable, str(TOOL), "copy", str(self.root / "output"), str(target)]
        first = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual((target / "ip-manifest.tcl").read_bytes(),
                         (self.root / "output" / "ip-manifest.tcl").read_bytes())
        second = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(second.returncode, 0)
        self.assertIn("IP_RESOURCE:", second.stderr)


if __name__ == "__main__":
    unittest.main()
