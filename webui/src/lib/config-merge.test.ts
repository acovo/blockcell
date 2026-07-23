import test from 'node:test';
import assert from 'node:assert/strict';
import { mergeLlmConfig } from './config-merge';

test('merges LLM edits into the latest config without overwriting concurrent changes', () => {
  const latest = {
    gateway: { port: 19999 },
    providers: { old: { apiKey: 'old' } },
    agents: { defaults: { maxIterations: 42, model: 'old' }, list: [{ id: 'ops' }] },
  };
  const merged = mergeLlmConfig(latest, { openai: { apiKey: 'new' } }, [
    { model: 'gpt-test', provider: 'openai', weight: 1, priority: 1 },
  ]);

  assert.deepEqual(merged.gateway, { port: 19999 });
  assert.deepEqual(merged.agents.list, [{ id: 'ops' }]);
  assert.equal(merged.agents.defaults.maxIterations, 42);
  assert.equal(merged.agents.defaults.model, 'gpt-test');
  assert.deepEqual(merged.providers, { openai: { apiKey: 'new' } });
});
