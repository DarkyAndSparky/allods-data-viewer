import { test } from 'node:test';
import assert from 'node:assert/strict';
import { targets, problems, current } from './version.mjs';

test('every target finds the version in the real files', () => {
  assert.match(current(), /^\d+\.\d+\.\d+$/);
  assert.deepEqual(problems(), []);
});

test('write only touches the project packages in Cargo.lock', () => {
  const lock = targets.find((t) => t.file === 'Cargo.lock');
  const text = [
    'name = "adv-core"\nversion = "0.1.1"\n',
    'name = "adv-cli"\nversion = "0.1.1"\n',
    'name = "adv-desktop"\nversion = "0.1.1"\n',
    'name = "adv-session"\nversion = "0.1.1"\n',
    'name = "tracing-core"\nversion = "0.1.1"\n',
  ].join('\n');
  const out = lock.write(text, '0.1.9');
  assert.equal(out.match(/version = "0\.1\.9"/g).length, 4);
  assert.match(out, /name = "tracing-core"\nversion = "0\.1\.1"/);
});

test('Cargo.toml write updates the package and the internal dependencies', () => {
  const toml = targets[0];
  const text = '[workspace.package]\nversion = "0.1.1"\n\n[workspace.dependencies]\nadv-core = { path = "crates/core", version = "0.1.1" }\nserde = { version = "0.1.1" }\n';
  const out = toml.write(text, '0.1.2');
  assert.equal(out.match(/0\.1\.2/g).length, 2);
  assert.match(out, /serde = \{ version = "0\.1\.1" \}/);
});
