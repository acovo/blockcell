import test from 'node:test';
import assert from 'node:assert/strict';
import { authProtocol, getAuthToken, setAuthToken } from './auth';
import { authenticatedFileEndpoint } from './authenticated-file';

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) { this.values.set(key, value); }
}

test('migrates a legacy persistent token into session storage', () => {
  const session = new MemoryStorage();
  const local = new MemoryStorage();
  local.setItem('blockcell_token', 'legacy-token');

  assert.equal(getAuthToken(session, local), 'legacy-token');
  assert.equal(session.getItem('blockcell_token'), 'legacy-token');
  assert.equal(local.getItem('blockcell_token'), null);
});

test('stores new tokens only for the browser session', () => {
  const session = new MemoryStorage();
  const local = new MemoryStorage();

  setAuthToken('new-token', session, local);

  assert.equal(session.getItem('blockcell_token'), 'new-token');
  assert.equal(local.getItem('blockcell_token'), null);
});

test('encodes websocket authentication as a subprotocol without exposing the token in a URL', () => {
  const protocol = authProtocol('a token/with+symbols');
  assert.match(protocol, /^blockcell-auth\.[A-Za-z0-9_-]+$/);
  assert.equal(protocol.includes('a token'), false);
});

test('builds file endpoints without putting credentials in the URL', () => {
  const endpoint = authenticatedFileEndpoint('http://localhost:18790', 'media/a b.png', 'ops', true);
  assert.equal(endpoint, 'http://localhost:18790/v1/files/serve?path=media%2Fa+b.png&agent=ops');
  assert.equal(endpoint.includes('token'), false);
});
