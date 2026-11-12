"""Transport opted-in resource directories from Veryl's resolved metadata v2.

This does not resolve dependencies or select compilation units. Veryl owns both.
An entry's containing directory is its resource bundle; paths remain relative.
"""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys


def projects(metadata):
    if metadata.get("format_version") != 2:
        raise ValueError("require Veryl metadata format 2")
    root = metadata["root"]
    result = [{**root, "id": "root"}, *metadata["dependencies"]]
    names, identities = set(), set()
    for project in result:
        name = project["name"]
        if not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", name):
            raise ValueError("invalid provider name")
        if name in names or project["id"] in identities:
            raise ValueError("duplicate resolved provider identity")
        names.add(name)
        identities.add(project["id"])
    return sorted(result, key=lambda item: item["id"])


def resources(project):
    settings = project.get("metadata", {}).get("rtl_flow")
    if settings is None:
        return None
    entry = PurePosixPath(settings["entry"])
    if entry.is_absolute() or ".." in entry.parts or len(entry.parts) < 2:
        raise ValueError("entry must be inside a package resource directory")
    root = Path(project["local_path"]).resolve()
    directory = root.joinpath(*entry.parent.parts)
    for parent in (directory, *directory.parents):
        if parent == root:
            break
        if parent.is_symlink():
            raise ValueError("resource directory may not be a symlink")
    files = sorted(directory.rglob("*"))
    if not (directory / entry.name).is_file():
        raise ValueError("missing declared resource entry")
    for path in files:
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError("resource tree must contain ordinary files/directories")
    selected = {p for p in files if p.is_file()}
    for name in settings.get("files", []):
        path = PurePosixPath(name)
        if path.is_absolute() or ".." in path.parts:
            raise ValueError("resource file must stay inside its package")
        file = root.joinpath(*path.parts)
        if not file.is_file() or file.is_symlink() or root not in file.resolve().parents:
            raise ValueError("missing or external package resource file")
        selected.add(file)
    return {"root": root, "entry": str(entry), "files": sorted(selected)}


def word(value):
    """A literal Tcl word, never executable substitution from a path/name."""
    value = str(value)
    for char in ('\\', '$', '[', ']', '"'):
        value = value.replace(char, '\\' + char)
    return '"' + value.replace('\n', '\\n').replace('\r', '\\r') + '"'


def bundle(entries, output):
    # Validate everything before creating an output; failed validation leaves no
    # apparently usable partial resource manifest.
    selected = [(p, resources(p)) for p in entries]
    if output.exists():
        raise ValueError("refusing to overwrite resource bundle")
    output.mkdir(parents=True)
    records, packages = [], {}
    for index, (project, resource) in enumerate(selected):
        if resource is None:
            continue
        relative = Path("ip") / f"package_{index}"
        names = []
        for file in resource["files"]:
            name = file.relative_to(resource["root"])
            target = output / relative / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(file, target)
            names.append(str(name))
        records.append("[dict create name %s id %s entry %s]" % (
            word(project["name"]), word(project["id"]), word(relative / resource["entry"])))
        packages[project["name"]] = {"directory": str(relative), "files": names}
    (output / "ip").mkdir(exist_ok=True)
    (output / "ip-manifest.tcl").write_text("return [list " + " ".join(records) + "]\n")
    (output / "ip-manifest.json").write_text(json.dumps(packages, indent=2) + "\n")


def inputs(entries):
    paths = set()
    for project in entries:
        root = Path(project["local_path"]).resolve()
        manifest = root / "Veryl.toml"
        if manifest.is_file():
            paths.add(manifest)
            # Ask the pinned compiler for its build configuration rather than
            # duplicating its TOML/default-source interpretation in this tool.
            launcher = Path(__file__).resolve().parents[1] / "veryl.sh"
            query = subprocess.run(
                ["bash", str(launcher), "metadata", "--format", "json", "--format-version", "1"],
                cwd=root, capture_output=True, text=True, check=True,
                env=dict(os.environ, GIT_MASTER="1"),
            )
            config = json.loads(query.stdout)
            for source in config.get("build", {}).get("sources", ["."]):
                directory = root / source
                if directory.is_file():
                    paths.add(directory)
                else:
                    paths.update(directory.rglob("*.veryl"))
        lock = root / "Veryl.lock"
        if lock.is_file():
            paths.add(lock)
        resource = resources(project)
        if resource:
            paths.update(resource["files"])
    for path in sorted(paths):
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        print(f"{digest}  {os.path.relpath(path)}")


def main():
    mode, source, *arguments = sys.argv[1:]
    if mode == "copy" and len(arguments) == 1:
        source, target = Path(source), Path(arguments[0])
        if any((target / name).exists() for name in ("ip", "ip-manifest.tcl", "ip-manifest.json")):
            raise ValueError("refusing to replace inherited package resources")
        shutil.copytree(source / "ip", target / "ip")
        for name in ("ip-manifest.tcl", "ip-manifest.json"):
            shutil.copyfile(source / name, target / name)
        return
    if mode == "locate" and len(arguments) == 2:
        provider, name = arguments
        root = Path(source).resolve()
        package = json.loads((root / "ip-manifest.json").read_text())[provider]
        if name not in package["files"]:
            raise ValueError("undeclared package resource")
        file = root / package["directory"] / name
        if not file.is_file() or root not in file.resolve().parents:
            raise ValueError("missing or external bundled resource")
        print(file)
        return
    entries = projects(json.loads(Path(source).read_text()))
    if mode == "bundle" and len(arguments) == 1:
        bundle(entries, Path(arguments[0]))
    elif mode == "inputs" and not arguments:
        inputs(entries)
    else:
        raise ValueError("usage: bundle.py bundle METADATA NEW_OUTPUT | inputs METADATA")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, TypeError, OSError, subprocess.CalledProcessError) as error:
        print(f"IP_RESOURCE: {error}", file=sys.stderr)
        sys.exit(2)
