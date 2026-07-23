import test from 'node:test';
import assert from 'node:assert/strict';
import { applyPersonaContent, saveBeforeSwitch } from './persona-operations';

test('applies a completed AI result to the file captured when generation started', () => {
  const contents = { 'AGENTS.md': 'old agents', 'SOUL.md': 'old soul' };
  assert.deepEqual(applyPersonaContent(contents, 'AGENTS.md', 'new agents'), {
    'AGENTS.md': 'new agents',
    'SOUL.md': 'old soul',
  });
});

test('does not switch when saving the source file fails', async () => {
  let switched = false;
  const ok = await saveBeforeSwitch(async () => false, () => { switched = true; });
  assert.equal(ok, false);
  assert.equal(switched, false);
});
