import test from 'node:test';
import assert from 'node:assert/strict';
import { isTerminalTransition } from './runStatus.ts';

test('isTerminalTransition fires only when a known active run finishes', () => {
  assert.equal(isTerminalTransition('running', 'success'), true);
  assert.equal(isTerminalTransition('pending', 'failed'), true);
  assert.equal(isTerminalTransition(null, 'success'), false);
  assert.equal(isTerminalTransition('success', 'success'), false);
  assert.equal(isTerminalTransition('running', 'running'), false);
});
