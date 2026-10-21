import test from 'node:test';
import assert from 'node:assert/strict';
import {sameProcess} from './process_identity.mjs';

test('a live original process remains a cleanup failure', () => {
  assert.equal(sameProcess({ProcessId: 42, Created: '100'}, {ProcessId: 42, Created: '100'}), true);
});
test('a reused PID is not the original process', () => {
  assert.equal(sameProcess({ProcessId: 42, Created: '100'}, {ProcessId: 42, Created: '200'}), false);
});
test('missing identity fails visibly', () => {
  assert.throws(() => sameProcess({ProcessId: 42}, {ProcessId: 42, Created: '100'}));
});
