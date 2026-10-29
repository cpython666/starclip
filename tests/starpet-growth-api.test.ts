import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import type { GrowthState } from '../src/starpet-growth.ts';
import { growthArchive } from '../src/starpet-growth.ts';

test('浏览器成长模拟独立内存、通知副本，过期导入/重置拒绝覆盖新进度', async () => {
  Object.assign(globalThis, { window: {} });
  const source = fs.readFileSync(new URL('../src/starpet-growth-api.ts', import.meta.url), 'utf8')
    .replaceAll("from './starpet-growth'", `from '${new URL('../src/starpet-growth.ts', import.meta.url).href}'`);
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
  const api = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  const first = await api.getStarPetGrowth() as GrowthState;
  assert.equal(first.enabled, false);
  const revisions: number[] = [];
  const stop = await api.onStarPetGrowthChanged((state: GrowthState) => { revisions.push(state.revision); state.xp = 99999; });
  await api.setStarPetGrowthEnabled(true);
  const greeted = await api.interactStarPet('greet') as GrowthState;
  assert.equal(greeted.xp, 10, 'listeners cannot mutate saved state');
  const oldArchive = growthArchive(greeted);
  await api.interactStarPet('highfive');
  const progressed = await api.getStarPetGrowth() as GrowthState;
  await assert.rejects(api.importStarPetGrowth(oldArchive, greeted.revision), /进度已变化/);
  await assert.rejects(api.resetStarPetGrowth(greeted.revision), /进度已变化/);
  assert.deepEqual(await api.getStarPetGrowth(), progressed);
  const imported = await api.importStarPetGrowth(oldArchive, progressed.revision) as GrowthState;
  assert.equal(imported.xp, 10, 'explicit import replaces rather than adds experience');
  assert.ok(imported.revision > progressed.revision);
  const reset = await api.resetStarPetGrowth(imported.revision) as GrowthState;
  assert.equal(reset.enabled, false);
  assert.equal(reset.xp, 0);
  assert.deepEqual(reset.unlockedItems, []);
  stop();
  const notificationCount = revisions.length;
  await api.setStarPetGrowthEnabled(true);
  assert.equal(revisions.length, notificationCount);
});

test('native interaction forwards animation intent once and the playback subscription releases cleanly', async () => {
  const savedWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const savedMock = Object.getOwnPropertyDescriptor(globalThis, 'petInteractionApiMock');
  const calls: unknown[] = [], received: unknown[] = [];
  let handler!: (event: { payload: unknown }) => void, stopped = 0;
  Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {} }, petInteractionApiMock: {
    async invoke(command: string, args: unknown) { calls.push({ command, args }); return { revision: 1 }; },
    async listen(event: string, callback: typeof handler) { assert.equal(event, 'starpet-interaction'); handler = callback; return () => { stopped++; }; },
  } });
  const mockUrl = 'data:text/javascript;base64,' + Buffer.from('export const {invoke,listen}=globalThis.petInteractionApiMock;').toString('base64');
  const source = fs.readFileSync(new URL('../src/starpet-growth-api.ts', import.meta.url), 'utf8')
    .replaceAll("from './starpet-growth'", `from '${new URL('../src/starpet-growth.ts', import.meta.url).href}'`)
    .replaceAll("'@tauri-apps/api/core'", `'${mockUrl}'`).replaceAll("'@tauri-apps/api/event'", `'${mockUrl}'`);
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
  try {
    const api = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
    await api.interactStarPet('greet'); await api.interactStarPet('greet', false);
    assert.deepEqual(calls, [{ command: 'interact_starpet', args: { action: 'greet', animate: true } }, { command: 'interact_starpet', args: { action: 'greet', animate: false } }]);
    const stop = await api.onStarPetInteraction((event: unknown) => received.push(event));
    const event = { action: 'stretch', animate: true, signalRevision: 2 }; handler({ payload: event });
    assert.deepEqual(received, [event]); assert.equal(calls.length, 2, 'playback never invokes another interaction'); stop(); assert.equal(stopped, 1);
    Object.assign(globalThis, { window: {} });
    const browser = await import('data:text/javascript;base64,' + Buffer.from(code + '\n// browser interaction').toString('base64'));
    (await browser.onStarPetInteraction(() => assert.fail('no desktop events in browser preview')))();
    assert.equal(calls.length, 2); assert.equal(stopped, 1);
  } finally {
    if (savedWindow) Object.defineProperty(globalThis, 'window', savedWindow); else Reflect.deleteProperty(globalThis, 'window');
    if (savedMock) Object.defineProperty(globalThis, 'petInteractionApiMock', savedMock); else Reflect.deleteProperty(globalThis, 'petInteractionApiMock');
  }
});
