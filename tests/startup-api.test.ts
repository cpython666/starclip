import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';

let moduleNumber = 0;
async function loadApi(desktop: boolean) {
  Object.assign(globalThis, { window: desktop ? { __TAURI_INTERNALS__: {} } : {} });
  const source = fs.readFileSync(new URL('../src/startup-api.ts', import.meta.url), 'utf8')
    .replaceAll("import('@tauri-apps/api/core')", 'Promise.resolve(globalThis.startupTestCore)');
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  return import('data:text/javascript;base64,' + Buffer.from(code + `\n// module ${moduleNumber++}`).toString('base64'));
}
test.afterEach(() => { delete (globalThis as { window?: unknown }).window; });

test('startup preferences use dedicated partial updates and a native quit command', async () => {
  const calls: unknown[] = [];
  const saved = { autoLaunch: true, startHidden: true, showTrayIcon: false };
  Object.assign(globalThis, { startupTestCore: { async invoke(command: string, args?: Record<string, unknown>) {
    calls.push([command, args]); return command === 'quit_starclip' ? undefined : saved;
  } } });
  const api = await loadApi(true);
  assert.equal(api.startupDesktopAvailable, true);
  assert.deepEqual(api.initialStartupPreferences(), { autoLaunch: false, startHidden: false, showTrayIcon: true });
  assert.deepEqual(await api.getStartupPreferences(), saved);
  assert.deepEqual(await api.setStartupPreferences({ autoLaunch: true }), saved);
  await api.setStartupPreferences({ startHidden: true });
  await api.setStartupPreferences({ showTrayIcon: false });
  await api.quitStarclip();
  assert.deepEqual(calls, [
    ['get_startup_preferences', undefined],
    ['set_startup_preferences', { patch: { autoLaunch: true } }],
    ['set_startup_preferences', { patch: { startHidden: true } }],
    ['set_startup_preferences', { patch: { showTrayIcon: false } }],
    ['quit_starclip', undefined],
  ]);
});

test('browser startup settings never register a login item, change preferences or quit', async () => {
  let invokes = 0;
  Object.assign(globalThis, { startupTestCore: { async invoke() { invokes++; throw new Error('unexpected native call'); } } });
  const api = await loadApi(false);
  assert.equal(api.startupDesktopAvailable, false);
  await assert.rejects(api.getStartupPreferences(), /桌面版/);
  await assert.rejects(api.setStartupPreferences({ autoLaunch: true }), /桌面版/);
  await assert.rejects(api.quitStarclip(), /桌面版/);
  assert.equal(invokes, 0);
});
