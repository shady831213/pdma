"""Report DMA implementation and port coverage from a completed native merge."""
import argparse
from pathlib import Path
import re
import shutil
import subprocess


DUT = 'pdma_vip_top.dut.integration.dma'
RTL = [f'+tree {DUT}', '-module common_cell_mailbox_checker',
       '-module common_cell_async_queue_checker']


def generate(tool, output, name, options):
    log = output / (name + '.log')
    # Relative paths keep whitespace in the caller's directories out of URG's
    # argument parser. The instance counts in hierarchy.txt include descendants.
    with log.open('w') as stream:
        subprocess.run([tool, '-full64', '-dir', '../merge/final/merged.vdb',
                        *options, '-show', 'ratios',
                        '-format', 'text', '-report', name], cwd=output,
                       stdout=stream, stderr=subprocess.STDOUT, check=True)
    if re.search(r'^(Warning-\[|Error[: -]|Fatal:)', log.read_text(), re.M):
        raise ValueError(f'{name} coverage diagnostic; see {log}')


def report(tool, output, name, scope, options, required_rows, show=('summary', '32')):
    rules = output / (name + '.hier')
    rules.write_text('\n'.join(scope) + '\n')
    generate(tool, output, name, ['-hier', rules.name, *options, '-show', *show])
    hierarchy = (output / name / 'hierarchy.txt').read_text().splitlines()
    rows = {line.split()[-1]: re.findall(r'\b(\d+)/(\d+)\b', line)
            for line in hierarchy if re.search(r'\d+/[1-9]\d*\s', line)}
    missing = set(required_rows + ['pdma_vip_top']) - rows.keys()
    if missing:
        raise ValueError(f'{name} has no measured coverage for {sorted(missing)}')
    if 'dma' in required_rows and rows['dma'] != rows['pdma_vip_top']:
        raise ValueError(f'{name} includes coverage outside the DMA instance')
    return rows


def external_ports(output, top, buses):
    def counts(row):
        (ratio,) = row  # These reports select a single metric.
        return tuple(map(int, ratio))

    direct = counts(top['dma'])
    cfg, memory = counts(buses['cfg']), counts(buses['memory'])
    combined = tuple(map(sum, zip(cfg, memory)))
    if counts(buses['pdma_vip_top']) != combined:
        raise ValueError('Bus toggle counts include objects outside the two DMA interfaces')
    total = tuple(map(sum, zip(direct, combined)))
    with (output / 'external-ports.txt').open('w') as stream:
        stream.write('Boundary  Covered/Measured  Toggle\n')
        for label, (hit, measured) in [('direct', direct), ('cfg', cfg), ('memory', memory),
                                       ('total', total)]:
            stream.write(f'{label}  {hit}/{measured}  {100 * hit / measured:.2f}%\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('merged_report', type=Path)
    parser.add_argument('--tool', default='urg')
    parser.add_argument('--metrics', required=True)
    args = parser.parse_args()
    executable = shutil.which(args.tool)
    if executable is None:
        parser.error(f'coverage tool unavailable: {args.tool}')
    tool = str(Path(executable).absolute())
    base = args.merged_report.resolve()
    output = base / 'dma'
    output.mkdir()
    report(tool, output, 'rtl', RTL, ['-metric', args.metrics],
           ['dma', 'control', 'frontend', 'commands', 'transport', 'endpoint'])
    logic = '+'.join(metric for metric in args.metrics.split('+') if metric != 'tgl')
    generate(tool, output, 'logic-holes',
             ['-hier', 'rtl.hier', '-metric', logic, '-show', 'brief', logic])
    if not (output / 'logic-holes/modinfo.txt').is_file():
        raise ValueError('Missing DMA logic coverage detail')
    ports = ['-metric', 'tgl', '-tgl', 'portsonly']
    report(tool, output, 'module-ports', RTL, ports, ['dma'])
    top = report(tool, output, 'top-ports', [f'+tree {DUT} 1'], ports, ['dma'], ('brief', 'tgl'))
    # AXI modport members belong to these two SV interface instances in the
    # wrapper. URG classifies them as signals, outside the DMA instance tree.
    buses = report(tool, output, 'bus-ports',
                   ['+tree pdma_vip_top.dut.cfg 1', '+tree pdma_vip_top.dut.memory 1'],
                   ['-metric', 'tgl'], ['cfg', 'memory'], ('brief', 'tgl'))
    external_ports(output, top, buses)
    # A fully covered group can still contain holes on individual channels.
    # Brief mode drops that group's entire detail page.
    generate(tool, output, 'group-bins', ['-metric', 'group'])
    groups = (base / 'report/groups.txt').read_text().splitlines()
    header = next(line for line in groups if 'AUTO BIN MAX' in line)
    pdma = [line for line in groups if re.search(r'\bpdma_vip_pkg::', line)]
    if not pdma:
        raise ValueError('Missing PDMA functional coverage groups')
    details = (output / 'group-bins/grpinfo.txt').read_text()
    sections = re.split(r'(?=^Group : )', details, flags=re.M)
    selected = [section for section in sections if section.startswith('Group : pdma_vip_pkg::')]
    reported = {section.splitlines()[0].removeprefix('Group : ').strip() for section in selected}
    missing = {line[line.index('pdma_vip_pkg::'):].strip() for line in pdma} - reported
    if missing:
        raise ValueError(f'Missing PDMA functional bin details for {sorted(missing)}')
    (output / 'functional.txt').write_text('\n'.join([header, *pdma, '', *selected]) + '\n')


if __name__ == '__main__':
    main()
