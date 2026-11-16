import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const source = fileURLToPath(new URL('../veryl.sh', import.meta.url));
function fixture(t, initialized = true) {
    const root = mkdtempSync(join(tmpdir(), 'pdma-veryl-launcher-'));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const directory = join(root, 'tools/rtl-flow');
    mkdirSync(directory, { recursive: true });
    const script = join(directory, 'veryl.sh');
    copyFileSync(source, script);
    writeFileSync(join(directory, 'toolchain.env'), 'MY_VERYL_TOOLCHAIN=qualified-recipe\n');
    if (initialized) {
        const upstream = join(root, 'external/my-veryl');
        mkdirSync(upstream, { recursive: true });
        writeFileSync(join(upstream, 'veryl.sh'),
            '#!/usr/bin/env bash\nprintf "%s\\n" "$MY_VERYL_CACHE_DIR" "$@"\nexit 7\n');
    }
    function run(args, extra = {}) {
        const env = { ...process.env, ...extra };
        if (!Object.hasOwn(extra, 'MY_VERYL_CACHE_DIR')) delete env.MY_VERYL_CACHE_DIR;
        return spawnSync('bash', [script, ...args], { encoding: 'utf8', timeout: 5000, env });
    }
    return { root, run };
}

test('delegates the pinned recipe and arguments without changing their boundaries', t => {
    const f = fixture(t);
    const r = f.run(['test', '--filter', 'two words']);
    assert.equal(r.status, 7);
    assert.deepEqual(r.stdout.trim().split('\n'), [join(f.root, 'tools/rtl-flow/.run/tools'),
        '--toolchain', 'qualified-recipe', 'test', '--filter', 'two words']);
});

test('honors caller cache and forwards the binary-directory query', t => {
    const f = fixture(t);
    const r = f.run(['--print-bin-dir'], { MY_VERYL_CACHE_DIR: '/fixture/shared cache' });
    assert.equal(r.status, 7);
    assert.deepEqual(r.stdout.trim().split('\n'), ['/fixture/shared cache',
        '--toolchain', 'qualified-recipe', '--print-bin-dir']);
});

test('missing submodule fails with an actionable initialization command', t => {
    const r = fixture(t, false).run(['--version']);
    assert.equal(r.status, 2);
    assert.match(r.stderr, /git submodule update --init external\/my-veryl/);
    assert.equal(r.stdout, '');
});
