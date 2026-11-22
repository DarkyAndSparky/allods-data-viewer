import test from 'node:test';
import assert from 'node:assert/strict';
import { licenceOk } from './third-party.mjs';

test('licence expressions', () => {
  assert.ok(licenceOk('MIT'));
  assert.ok(licenceOk('MIT OR Apache-2.0'));
  assert.ok(licenceOk('MIT OR Apache-2.0 OR LGPL-2.1-or-later'));
  assert.ok(licenceOk('Apache-2.0 WITH LLVM-exception'));
  assert.ok(!licenceOk('GPL-3.0-only'));
  assert.ok(!licenceOk('AGPL-3.0 OR GPL-2.0'));
  assert.ok(!licenceOk('UNKNOWN'));
});
