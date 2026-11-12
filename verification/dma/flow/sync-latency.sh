#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
snapshot=$(realpath "${1:?Usage: sync-latency.sh SNAPSHOT NEW_SIMULATION_SNAPSHOT}")
output=$(realpath -m "${2:?Supply a fresh snapshot directory}")
(cd "$snapshot" && sha256sum --check --status snapshot.sha256)
pdma_new_run "$output"
python3 "$root/tools/rtl-flow/ip/bundle.py" copy "$snapshot" "$output"
python3 - "$snapshot" "$output" "$root/verification/dma/tests/sync_latency.sv" <<'PY'
import hashlib,json,shutil,sys
from pathlib import Path
source,output,replacement=map(Path,sys.argv[1:])
files=[Path(line) for line in (source/'sources.f').read_text().splitlines()]
targets=[file for file in files if file.name.endswith('_synchronizer_basic.sv')]
if len(targets)!=1:raise SystemExit('Expected one pinned synchronizer implementation')
for file in files:shutil.copyfile(file,output/file.name)
shutil.copyfile(replacement,output/targets[0].name)
for name in ['compiler.txt','design.sha256']:shutil.copyfile(source/name,output/name)
(output/'sources.f').write_text('+define+PDMA_CDC_LATENCY_STRESS\n'+''.join(str(output/file.name)+'\n' for file in files))
(output/'simulation-only.json').write_text(json.dumps({
 'purpose':'independent coherent-vector 0/1 destination-cycle synchronizer latency stress',
 'production_snapshot_sha256':hashlib.sha256((source/'snapshot.sha256').read_bytes()).hexdigest(),
 'replacement_sha256':hashlib.sha256(replacement.read_bytes()).hexdigest(),
 'not_qualified_for':'synthesis, static closure, analog metastability or MTBF'
},indent=2)+'\n')
PY
(cd "$output" && {
    sha256sum ./*.sv sources.f compiler.txt design.sha256 simulation-only.json ip-manifest.*
    find ip -type f -print0 | sort -z | xargs -0 -r sha256sum
} > snapshot.sha256)
(cd "$snapshot" && sha256sum --check --status snapshot.sha256)
printf 'PDMA_LATENCY_SNAPSHOT_COMPLETE\n'
