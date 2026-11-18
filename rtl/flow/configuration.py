"""Resolve a DMA elaboration profile once for RTL, testbench and implementation.

This selects native Veryl types. It does not rewrite emitted RTL or duplicate the
source tree. Physical feasibility is qualified per profile, not implied by legality.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / 'rtl/profiles/default.json'


def inputs(value):
    if not isinstance(value, dict):
        raise ValueError('profile must be a JSON object of independent inputs')
    return value.copy()


def resolve(overrides=None):
    with DEFAULT.open() as stream:
        values = json.load(stream)
    overrides = inputs({} if overrides is None else overrides)
    if set(overrides) - set(values):
        raise ValueError('only independent profile inputs may be configured')
    values.update(overrides)
    if any(type(v) is not int or v <= 0 for v in values.values()):
        raise ValueError('profile inputs must be positive integers')
    if not 1 <= values['channels'] <= 8:
        raise ValueError('channels must be in 1..8')
    if values['register_bits'] not in (32, 64):
        raise ValueError('register_bits must be 32 or 64')
    if values['memory_bits'] not in (32, 64, 128, 256, 512, 1024):
        raise ValueError('memory_bits must be an AXI width from 32 to 1024')
    for key in ('request_depth', 'receipt_depth'):
        n = values[key]
        # The pinned common_cell AsyncQueue implementation/constraint contract
        # requires this bound even if a smaller queue simulates successfully.
        if n < 4 or n & (n - 1):
            raise ValueError(f'{key} must be a power of two, at least four')
    if not 1 <= values['burst_beats'] <= 256:
        raise ValueError('burst_beats must be in 1..256')
    values['memory_bytes'] = values['memory_bits'] // 8
    values['register_bytes'] = values['register_bits'] // 8
    values['burst_bytes'] = values['memory_bytes'] * values['burst_beats']
    if values['burst_bytes'] < 8:
        raise ValueError('a retained burst must hold one Fence64 value')
    values['rows'] = values['channels'] * values['slots']
    values['row_bits'] = values['burst_bytes'] * 8
    values['slot_bits'] = values['slots'].bit_length()
    if max(values.values()) >= 2**31:
        raise ValueError('profile exceeds RTL integer geometry')
    return values


def selected(path=None):
    with Path(path or DEFAULT).open() as stream:
        overrides = inputs(json.load(stream))
    if 'PDMA_CHANNEL_COUNT' in os.environ:
        count = os.environ['PDMA_CHANNEL_COUNT']
        if not re.fullmatch('[1-8]', count):
            raise ValueError('PDMA_CHANNEL_COUNT must be in 1..8')
        overrides['channels'] = int(count)
    return resolve(overrides)


def tcl(values):
    return 'return [dict create ' + ' '.join(f'{k} {v}' for k, v in values.items()) + ']\n'


def project(destination, values):
    destination.mkdir(parents=True, exist_ok=False)
    (destination / 'src').mkdir()
    manifest = (ROOT / 'rtl/Veryl.toml').read_text()
    sources = json.dumps([str(ROOT / 'rtl/src'), 'src'])
    manifest, count = re.subn(r'^sources\s*=.*$', 'sources = ' + sources, manifest, count=1, flags=re.M)
    if count != 1:
        raise ValueError('native project must have one sources declaration')
    (destination / 'Veryl.toml').write_text(manifest)
    shutil.copyfile(ROOT / 'rtl/Veryl.lock', destination / 'Veryl.lock')
    (destination / 'configuration.json').write_text(json.dumps(values, indent=2) + '\n')
    (destination / 'configuration.tcl').write_text(tcl(values))
    defines = {'CHANNEL_COUNT': 'channels', 'REGISTER_BITS': 'register_bits', 'MEMORY_BITS': 'memory_bits',
               'BURST_BEATS': 'burst_beats', 'COPY_SLOTS': 'slots', 'SLOT_BITS': 'slot_bits',
               'READ_OUTSTANDING': 'read_outstanding', 'WRITE_OUTSTANDING': 'write_outstanding'}
    (destination / 'configuration.svh').write_text('`ifndef PDMA_CONFIGURATION_SVH\n`define PDMA_CONFIGURATION_SVH\n'
        + ''.join(f'`define PDMA_{k} {values[v]}\n' for k, v in defines.items()) + '`endif\n')
    r = f'$std::axi4_lite_pkg::<32, {values["register_bytes"]}, 1>'
    b = f'$std::axi4_pkg::<64, {values["memory_bytes"]}, 1, 35, 1, 2, 35, 2>'
    ports = '''reg_clk: input 'registers clock, reg_rst_n: input 'registers reset_async_low,
    core_clk: input 'core clock, core_rst_n: input 'core reset_async_low,
    bus_clk: input 'bus clock, bus_rst_n: input 'bus reset_async_low,
    cfg: modport 'registers $std::axi4_lite_if::<REG>::slave,
    memory: modport 'bus $std::axi4_if::<BUS>::master,
    awatop: output 'bus logic<6>,
    irq: output 'core logic<CHANNELS>, irq_ready: input 'core logic<CHANNELS>,
    fatal_irq: output 'core logic, fatal_irq_ready: input 'core logic, fatal: output 'core logic,'''
    ports = ports.replace('REG', r).replace('BUS', b).replace('CHANNELS', str(values['channels']))
    knobs = {'CHANNEL_COUNT':'channels', 'READ_OUTSTANDING':'read_outstanding',
             'WRITE_OUTSTANDING':'write_outstanding', 'REQUEST_DEPTH':'request_depth', 'RECEIPT_DEPTH':'receipt_depth'}
    parameters = ',\n        '.join(f'{name}: {values[key]}' for name, key in knobs.items())
    native = f'''// Generated static integration binding; no added state or latency.
pub module DmaConfigured (
    {ports}
) {{
    inst dma: DmaCopy::<{r}, {b}, {values['burst_beats']}, {values['slots']}> #(
        {parameters},
    ) (
        reg_clk, reg_rst_n, core_clk, core_rst_n, bus_clk, bus_rst_n,
        cfg, memory, awatop, irq, irq_ready, fatal_irq, fatal_irq_ready, fatal,
    );
}}
'''
    (destination / 'src/profile.veryl').write_text(native)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path)
    parser.add_argument('--project', type=Path)
    parser.add_argument('--tcl', action='store_true')
    args = parser.parse_args()
    try:
        values = selected(args.profile)
        if args.project:
            project(args.project, values)
        else:
            print(' '.join(f'{k} {v}' for k, v in values.items()) if args.tcl else json.dumps(values, indent=2))
    except (ValueError, OSError) as error:
        parser.error(str(error))


if __name__ == '__main__':
    main()
