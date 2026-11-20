"""Bind the VIP to compiler-emitted interface types, without guessing mangling."""
import json
from pathlib import Path
import re
import sys


def binding(snapshot):
    sources = list(snapshot.glob('*_profile.sv'))
    if len(sources) != 1:
        raise ValueError('expected one native integration profile')
    source = sources[0].read_text()
    types = {}
    for port, direction in (('cfg', 'slave'), ('memory', 'master')):
        matches = re.findall(r'(\w+)\.' + direction + r'\s+' + port + r'\b', source)
        if len(matches) != 1:
            raise ValueError(f'missing unique compiler interface for {port}')
        types[port] = matches[0]
    text = f'{types["cfg"]} cfg();\n{types["memory"]} memory();\n'
    axi5 = json.loads((snapshot / 'configuration.json').read_text())['memory_axi_version'] == 5
    if not axi5:
        text += "assign control.awatop = '0;\n"
    text += 'pdma_rtl_DmaConfigured integration(\n'
    ports = {name: f'control.{name}' for name in ('reg_clk', 'reg_rst_n', 'core_clk', 'core_rst_n',
             'bus_clk', 'bus_rst_n', 'irq', 'fatal', 'fatal_irq')}
    if axi5:
        ports['awatop'] = 'control.awatop'
    ports.update(cfg='cfg', memory='memory')
    text += ',\n'.join(f'    .{name}({value})' for name, value in ports.items()) + '\n);\n'
    (snapshot / 'dut_binding.svh').write_text(text)


if __name__ == '__main__':
    binding(Path(sys.argv[1]))
