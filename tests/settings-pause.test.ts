import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';

// API-only fixed fixtures: no native application, database or clipboard access.
let sequence = 0;
async function loadApi(desktop: boolean) {
  Object.assign(globalThis, { window: desktop ? { __TAURI_INTERNALS__: {} } : {} });
  const ipc = 'data:text/javascript;base64,' + Buffer.from('export const invoke = (...args) => globalThis.settingsPauseInvoke(...args);').toString('base64');
  const source = fs.readFileSync(new URL('../src/api.ts', import.meta.url), 'utf8')
    .replace("from './privacy'", `from '${new URL('../src/privacy.ts', import.meta.url).href}'`)
    .replace("from './snippets'", `from '${new URL('../src/snippets.ts', import.meta.url).href}'`)
    .replace("from './i18n/index.ts'", `from '${new URL('../src/i18n/index.ts', import.meta.url).href}'`)
    .replace("'@tauri-apps/api/core'", `'${ipc}'`) + `\n// fixture: ${sequence++}`;
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  return (await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'))).api;
}

test('native preference save sends the capture baseline in the same command', async () => {
  const commands: unknown[] = [];
  Object.assign(globalThis, { settingsPauseInvoke: async (...args: unknown[]) => { commands.push(args); } });
  const api = await loadApi(true);
  const settings = { paused: false, pauseRevision: 7, layout: 'list' };
  await api.saveSettings(settings);
  await api.saveSettings(settings, 5);
  assert.deepEqual(commands, [
    ['save_settings', { settings, expectedPauseRevision: 7 }],
    ['save_settings', { settings, expectedPauseRevision: 5 }],
  ]);
});

test('browser preference save preserves a newer pause but permits a current explicit resume', async () => {
  const api = await loadApi(false);
  const original = (await api.getState()).settings;
  await api.saveSettings({ ...original, paused: false });
  const staleDraft = { ...(await api.getState()).settings, layout: 'list' };
  await api.saveSettings({ ...staleDraft, paused: true });
  await api.saveSettings(staleDraft);
  assert.equal((await api.getState()).settings.paused, true);
  assert.equal((await api.getState()).settings.pauseRevision, 2);
  assert.equal((await api.getState()).settings.layout, 'list');
  await api.saveSettings({ ...(await api.getState()).settings, paused: false });
  assert.equal((await api.getState()).settings.paused, false);
  assert.equal((await api.getState()).settings.pauseRevision, 3);
});

test('browser pause revision rejects an unseen resume-pause round trip and ignores requested version values', async () => {
  const api = await loadApi(false);
  const opened = (await api.getState()).settings;
  assert.equal(opened.pauseRevision, 0);
  const staleResume = { ...opened, paused: false, layout: 'list' };
  await api.saveSettings({ ...opened, paused: false, pauseRevision: 999 }, opened.pauseRevision);
  const resumed = (await api.getState()).settings;
  assert.equal(resumed.pauseRevision, 1);
  await api.saveSettings({ ...resumed, paused: true });
  await api.saveSettings(staleResume);
  const paused = (await api.getState()).settings;
  assert.equal(paused.paused, true); assert.equal(paused.pauseRevision, 2);
  assert.equal(paused.layout, 'list');
  await api.saveSettings({ ...paused, paused: false, pauseRevision: 999 }, paused.pauseRevision);
  assert.equal((await api.getState()).settings.paused, false);
  assert.equal((await api.getState()).settings.pauseRevision, 3);
  await api.setTheme('forest');
  assert.equal((await api.getState()).settings.pauseRevision, 3, 'theme-only saves preserve the capture version');
});
