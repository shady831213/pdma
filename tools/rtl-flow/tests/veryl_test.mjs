import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const source = fileURLToPath(new URL('../veryl.sh', import.meta.url));
const digest = (text) => createHash('sha256').update(text).digest('hex');

function fixture(t, patched = false) {
    const root = mkdtempSync(join(tmpdir(), 'veryl-install-test-'));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const directory = join(root, 'tools/rtl-flow');
    mkdirSync(join(directory, 'patches'), { recursive: true });
    const script = join(directory, 'veryl.sh');
    copyFileSync(source, script);
    const patch = join(directory, 'patches/fix.patch');
    const bytes = 'fixture patch\n';
    if (patched) writeFileSync(patch, bytes);
    const manifest = patched ? `${digest(bytes)}  fix.patch\n` : '';
    const hash = digest(manifest);
    writeFileSync(join(directory, 'toolchain.env'), [
        'VERYL_VERSION=fixture', 'VERYL_COMMIT=fixture',
        `VERYL_SOURCE_SHA256=${'0'.repeat(64)}`,
        'VERYL_RUST_VERSION=fixture-uninstalled',
        'VERYL_BUILD_PROFILE=release-verylup',
        `VERYL_PATCH_FILES=(${patched ? 'fix.patch' : ''})`,
        `VERYL_PATCH_SHA256=${hash}`, '',
    ].join('\n'));
    function install(profile, marker) {
        const location = join(directory, '.run/tools',
            `veryl-fixture-rustfixture-uninstalled-${profile}-p${hash.slice(0, 12)}`);
        mkdirSync(join(location, 'bin'), { recursive: true });
        const binary = join(location, 'bin/veryl');
        const program = `#!/bin/sh\nprintf '%s\\n' '${marker}'\n`;
        writeFileSync(binary, program, { mode: 0o755 });
        writeFileSync(join(location, 'SHA256SUMS'), `${digest(program)}  bin/veryl\n`);
        return { binary, directory: join(location, 'bin') };
    }
    function run(argument = '--version') {
        const result = spawnSync('bash', [script, argument], { encoding: 'utf8', timeout: 5000 });
        assert.equal(result.error, undefined);
        assert.equal(result.signal, null);
        return result;
    }
    return { install, run, patch };
}

test('empty patch list selects a verified optimized installation without building', (t) => {
    const f = fixture(t);
    f.install('dev', 'legacy');
    f.install('release-verylup', 'optimized');
    const result = f.run();
    assert.equal(result.status, 0);
    assert.equal(result.stdout, 'optimized\n');
});

test('ordered patch manifest permits an unchanged verified installation', (t) => {
    const f = fixture(t, true);
    const installed = f.install('release-verylup', 'patched');
    const result = f.run('--print-bin-dir');
    assert.equal(result.status, 0);
    assert.equal(result.stdout, `${installed.directory}\n`);
});

test('changed patch bytes reject the cached installation before execution', (t) => {
    const f = fixture(t, true);
    f.install('release-verylup', 'must-not-run');
    writeFileSync(f.patch, 'changed patch\n');
    const result = f.run();
    assert.equal(result.status, 1);
    assert.match(result.stderr, /Veryl patch manifest checksum mismatch/);
    assert.equal(result.stdout, '');
});

test('changed installed binary is rejected rather than executed', (t) => {
    const f = fixture(t);
    const installed = f.install('release-verylup', 'original');
    writeFileSync(installed.binary, '#!/bin/sh\nprintf "must-not-run\\n"\n');
    const result = f.run();
    assert.equal(result.status, 1);
    assert.equal(result.stdout, '');
});
