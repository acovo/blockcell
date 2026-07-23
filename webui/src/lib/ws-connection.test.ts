import test from 'node:test';
import assert from 'node:assert/strict';
import { forCurrentSocket, sendJson } from './ws-connection';

test('ignores callbacks emitted by a socket that is no longer current', () => {
  const oldSocket = {};
  const currentSocket = {};
  let calls = 0;
  const callback = forCurrentSocket<[string]>(() => currentSocket, oldSocket, () => { calls += 1; });

  callback('late event');

  assert.equal(calls, 0);
});

test('reports failure instead of dropping a message on a disconnected socket', () => {
  const socket = { readyState: 3, send() { throw new Error('must not send'); } };
  assert.equal(sendJson(socket, { type: 'chat' }, 1), false);
});

test('sends and reports success when the socket is open', () => {
  let sent = '';
  const socket = { readyState: 1, send(value: string) { sent = value; } };
  assert.equal(sendJson(socket, { type: 'chat', content: 'hello' }, 1), true);
  assert.equal(sent, '{"type":"chat","content":"hello"}');
});
