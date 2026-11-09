import { test } from 'node:test';
import assert from 'node:assert/strict';
import { problems } from './check-repo.mjs';

const small = () => 10;

test('accepts normal project files', () => {
  assert.deepEqual(problems(['README.md', 'crates/core/src/lib.rs', 'assets/icons/icon.ico', 'web/public/fonts/README.md'], small), []);
});

test('rejects game data, fonts and secrets', () => {
  for (const f of ['Texts.pak', 'a/b/pack.rus.loc', 'x.fsb', 'x.bsb', 'intro.ogv', 'tex.dds', 'tex.bin',
    'web/public/fonts/Allods.ttf', '.env', '.env.local', 'k.pem']) {
    assert.equal(problems([f], small).length, 1, f);
  }
});

test('rejects samples directory and large files', () => {
  assert.equal(problems(['samples/readme.txt'], small).length, 1);
  assert.equal(problems(['docs/big.png'], () => 3_000_000).length, 1);
});
