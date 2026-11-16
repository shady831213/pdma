#!/usr/bin/env python3
"""Bound URG fan-in without dropping metrics, tests, or source-run provenance."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import xml.etree.ElementTree as ET


def fingerprint(directory):
    digest = hashlib.sha256()
    for path in sorted(directory.rglob('*')):
        if path.is_symlink():
            raise ValueError(f'Coverage input contains a symbolic link: {path}')
        if path.is_file():
            digest.update(str(path.relative_to(directory)).encode() + b'\0')
            file_hash = hashlib.sha256()
            with path.open('rb') as source:
                for block in iter(lambda: source.read(1024 * 1024), b''):
                    file_hash.update(block)
            digest.update(file_hash.digest())
    return digest.hexdigest()


def sources(directory):
    records = set()
    tests = directory / 'snps/coverage/db/testdata'
    for name in ('siminfo.xml', 'test.xml'):
        for path in tests.glob(f'*/{name}'):
            data = path.read_bytes()
            if data.startswith(b'\x1f\x8b'):
                data = gzip.decompress(data)
            for source in ET.fromstring(data).iter('srctest'):
                # URG assigns these two labels when creating a merged database.
                # All original simulation attributes must otherwise survive.
                identity = {k: v for k, v in source.attrib.items() if k not in ('id', 'name')}
                records.add(hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest())
    if not records:
        raise ValueError(f'No native simulation provenance: {directory}')
    return records


def invoke(tool, design, inputs, expected, directory, report):
    directory.mkdir()
    database = directory / 'merged.vdb'
    # The vendor launcher reparses arguments. Local names keep whitespace in
    # caller-owned paths out of its argument parser without copying databases.
    aliases = {f'input-{i}.vdb': str(path) for i, path in enumerate([design, *inputs])}
    # URG treats the final pathname component as the merged test name.
    command = [tool, '-full64', '-strict', '-merge_strict', '-dir',
               *aliases, '-dbname', 'merged/all']
    if report is None:
        command += ['-noreport']
    else:
        command += ['-report', os.path.relpath(report, directory), '-format', 'text', '-show', 'summary', '-show', 'ratios']
    invocation = {'cwd': str(directory), 'command': command, 'inputs': aliases}
    (directory / 'invocation.json').write_text(json.dumps(invocation, indent=2) + '\n')
    log = directory / 'tool.log'
    try:
        for name, target in aliases.items():
            (directory / name).symlink_to(target, target_is_directory=True)
        with log.open('w') as output:
            status = subprocess.run(command, cwd=directory, stdout=output, stderr=subprocess.STDOUT, check=False).returncode
    finally:
        for name in aliases:
            (directory / name).unlink(missing_ok=True)
    (directory / 'exit.json').write_text(json.dumps({'exit': status}) + '\n')
    # URG can skip incompatible instance data yet return zero, even in strict
    # mode. A warning-bearing merge cannot qualify complete metric preservation.
    diagnostics = re.search(r'^(Error[: \-]|Fatal:|Warning-\[)', log.read_text(errors='replace'), re.MULTILINE)
    if status or diagnostics:
        raise RuntimeError(f'URG stage failed: {directory.name}, exit={status}')
    actual = sources(database)
    if actual != expected:
        raise RuntimeError(f'URG source records changed: {directory.name}')
    print(f'RTL_COVERAGE_STAGE_COMPLETE stage={directory.name} source_records={len(actual)}', flush=True)
    return database


def merge(args):
    design = args.design.resolve(strict=True)
    inputs = [Path(line).resolve(strict=True) for line in args.inputs.read_text().splitlines() if line]
    if not inputs or len(set(inputs)) != len(inputs):
        raise ValueError('Coverage inputs must be a nonempty set of distinct databases')
    if design in inputs or not design.is_dir() or any(not p.is_dir() for p in inputs):
        raise ValueError('Provide one design database and separate simulation databases')
    out = args.output.resolve()
    if any(out == p or out in p.parents or p in out.parents for p in [design, *inputs]):
        raise ValueError('Coverage output must not overlap an input database')
    tool = shutil.which(args.tool)
    if tool is None:
        raise ValueError(f'Coverage tool not found: {args.tool}')
    tool = str(Path(tool).absolute())
    out.mkdir(parents=True, exist_ok=True)
    if (out / 'report').exists():
        raise ValueError('Coverage report already exists')
    work = out / 'merge'
    work.mkdir()
    hashes = {str(p): fingerprint(p) for p in [design, *inputs]}
    pending = [(p, sources(p)) for p in inputs]
    expected = set()
    for _, records in pending:
        if expected & records:
            raise ValueError('Input databases overlap in original simulation records')
        expected |= records
    manifest = {'input_fingerprints': hashes, 'source_records': len(expected),
                'batch_size': args.batch_size, 'stages': []}
    (out / 'merge-inputs.json').write_text(json.dumps(manifest, indent=2) + '\n')
    level = 0
    while len(pending) > args.batch_size:
        following = []
        for offset in range(0, len(pending), args.batch_size):
            group = pending[offset:offset + args.batch_size]
            records = set().union(*(entry[1] for entry in group))
            stage = work / f'level-{level}-batch-{offset // args.batch_size}'
            database = invoke(tool, design, [entry[0] for entry in group], records, stage, None)
            following.append((database, records))
            manifest['stages'].append({'stage': stage.name, 'source_records': len(records)})
        pending = following
        level += 1
    database = invoke(tool, design, [entry[0] for entry in pending], expected, work / 'final', out / 'report')
    if not (out / 'report/dashboard.txt').is_file():
        raise RuntimeError('URG did not produce a coverage summary')
    if hashes != {str(p): fingerprint(p) for p in [design, *inputs]}:
        raise RuntimeError('Coverage input databases changed during merge')
    manifest.update(merged_database=str(database), merged_fingerprint=fingerprint(database), complete=True)
    (out / 'merge-result.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'RTL_COVERAGE_MERGE_COMPLETE source_records={len(expected)}', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--design', type=Path, required=True)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--tool', default='urg')
    parser.add_argument('--batch-size', type=int, default=32)
    args = parser.parse_args()
    if not 2 <= args.batch_size <= 32:
        parser.error('--batch-size must be in 2..32')
    merge(args)


if __name__ == '__main__':
    main()
