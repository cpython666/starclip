import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { compileScript, parse } from '@vue/compiler-sfc';
import { createRenderer, nextTick } from 'vue';
import { defaultAppearance } from '../src/appearance.ts';
import type { AppState } from '../src/types.ts';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}
function moduleUrl(source: string) {
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  return 'data:text/javascript;base64,' + Buffer.from(code).toString('base64');
}
async function flush() { await new Promise(resolve => setImmediate(resolve)); await nextTick(); }
function globals(values: Record<string, unknown>) {
  const saved = Object.keys(values).map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)] as const);
  for (const [key, value] of Object.entries(values)) Object.defineProperty(globalThis, key, { value, configurable: true, writable: true });
  return () => { for (const [key, descriptor] of saved) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else Reflect.deleteProperty(globalThis, key); } };
}
function eventHub() {
  const listeners = new Map<string, Set<Function>>();
  return {
    addEventListener(name: string, callback: Function) { if (!listeners.has(name)) listeners.set(name, new Set()); listeners.get(name)!.add(callback); },
    removeEventListener(name: string, callback: Function) { listeners.get(name)?.delete(callback); },
    count() { return [...listeners.values()].reduce((sum, callbacks) => sum + callbacks.size, 0); },
  };
}

// Exercise the actual App setup, reactive guards and lifecycle. Its unrelated
// template/children are omitted; API responses are fixed in-memory fixtures.
const renderer = createRenderer<object, object>({
  createElement: () => ({}), createText: () => ({}), createComment: () => ({}),
  setText() {}, setElementText() {}, patchProp() {}, insert() {}, remove() {},
  parentNode: () => null, nextSibling: () => null,
});
let sequence = 0;
async function mount(options: { delayedRead?: boolean; delayedCloseListener?: boolean } = {}) {
  const state: AppState = {
    onboardingCompleted: true, clips: [], groups: [],
    settings: { paused: true, theme: 'dark', layout: 'grid', maxHistory: 0, retentionDays: 0, alwaysOnTop: false, showStartup: false, stickerPack: 'none', shortcut: 'CommandOrControl+Shift+V' },
    clipboardError: null, shortcutError: null, windowError: null,
  };
  const windowEvents = eventHub(), themeEvents = eventHub();
  const read = deferred<AppState>(), listening = deferred<() => void>();
  const calls: { release: boolean; requestId?: number }[] = [];
  let closeRequest: ((requestId: number) => void) | undefined;
  const stopped = { change: 0, close: 0, growth: 0 };
  const stopClose = () => { stopped.close++; };
  const mock = {
    async getState() { return options.delayedRead ? read.promise : structuredClone(state); },
    async privacyRules() { return { revision: 1, rules: [] }; },
    async onChange() { return () => { stopped.change++; }; },
    async hideWindow(release: boolean, requestId?: number) { calls.push({ release, requestId }); },
    async onWindowCloseRequested(callback: (requestId: number) => void) { closeRequest = callback; return options.delayedCloseListener ? listening.promise : stopClose; },
    async setTheme(theme: string) { return { ...state.settings, theme }; },
  };
  const restore = globals({
    window: { ...windowEvents, matchMedia: () => ({ ...themeEvents, matches: false }) },
    document: { documentElement: { dataset: {}, style: { setProperty() {}, removeProperty() {} } } },
    navigator: { platform: 'MacIntel' },
    windowReleaseMock: {
      api: mock, getAppearance: async () => defaultAppearance(),
      async listen(name: string) { assert.equal(name, 'starpet-growth-error'); return () => { stopped.growth++; }; },
    },
  });
  const mockUrl = moduleUrl(`export const { api, getAppearance, listen } = globalThis.windowReleaseMock; export const isDesktop = true; export const appVersion = '0.0.0-test'; // ${sequence++}`);
  const stubUrl = moduleUrl('export default { render() { return null; } };');
  const { descriptor } = parse(fs.readFileSync(new URL('../src/App.vue', import.meta.url), 'utf8'));
  const script = compileScript(descriptor, { id: 'window-release-test' });
  const source = script.content.replace(/from (['"])vue\1/g, `from '${import.meta.resolve('vue')}'`)
    .replace(/(['"])\.\/components\/[^'"]+\.vue\1/g, `'${stubUrl}'`)
    .replaceAll("'./api'", `'${mockUrl}'`).replaceAll("'./appearance-api'", `'${mockUrl}'`).replaceAll("'./update-api'", `'${mockUrl}'`)
    .replaceAll("'@tauri-apps/api/event'", `'${mockUrl}'`)
    .replaceAll("'./appearance'", `'${new URL('../src/appearance.ts', import.meta.url).href}'`)
    .replaceAll("'./privacy'", `'${new URL('../src/privacy.ts', import.meta.url).href}'`)
    .replaceAll("'./theme'", `'${new URL('../src/theme.ts', import.meta.url).href}'`);
  const component = await import(moduleUrl(source));
  const app = renderer.createApp({ ...component.default, render() { return null; } });
  app.mount({}); await flush();
  const vm = app._instance!.setupState as Record<string, any>;
  let unmounted = false;
  const unmount = () => { if (!unmounted) { unmounted = true; app.unmount(); } };
  return {
    vm, mock, calls, stopped, windowEvents, themeEvents, unmount,
    finishRead: () => read.resolve(structuredClone(state)), finishListener: () => listening.resolve(stopClose),
    async close(requestId: number) { assert(closeRequest, 'native close handler registered'); closeRequest(requestId); await flush(); },
    cleanup() { unmount(); restore(); },
  };
}

test('idle App permits release while editing, settings and studio keep their in-memory drafts', async () => {
  const fixture = await mount(); const { vm, calls } = fixture;
  try {
    await fixture.close(11); assert.deepEqual(calls.pop(), { release: true, requestId: 11 });
    vm.openClip(); vm.clipContent = '原创未保存草稿';
    await fixture.close(12); assert.deepEqual(calls.pop(), { release: false, requestId: 12 });
    assert.equal(vm.clipContent, '原创未保存草稿'); assert.equal(vm.modal, 'clip');
    vm.closeModal(); vm.openSettings();
    assert.equal(vm.settingsDirty, false);
    await fixture.close(13); assert.deepEqual(calls.pop(), { release: false, requestId: 13 }, 'a settings child can own a draft before the parent is dirty');
    vm.settingsDraft.maxHistory = 500;
    assert.equal(vm.settingsDirty, true);
    await fixture.close(13); assert.deepEqual(calls.pop(), { release: false, requestId: 13 });
    assert.equal(vm.settingsDraft.maxHistory, 500); assert.equal(vm.modal, 'settings');
    vm.closeModal(); vm.openStudio();
    await fixture.close(14); assert.deepEqual(calls.pop(), { release: false, requestId: 14 }); assert.equal(vm.studioOpen, true);
    vm.studioOpen = false;
    vm.keyboard({ key: 'Escape', target: { matches: () => false, closest: () => null } }); await flush();
    assert.deepEqual(calls.pop(), { release: true, requestId: undefined }, 'Escape uses the same release guard');
  } finally { fixture.cleanup(); }
});

test('loading and active work defer release until they settle, including every child busy flag', async () => {
  const fixture = await mount({ delayedRead: true }); const { vm, mock, calls } = fixture;
  try {
    await vm.hideMainWindow(20); assert.deepEqual(calls.pop(), { release: false, requestId: 20 });
    fixture.finishRead(); await flush();
    const operation = deferred<void>();
    const running = vm.run(() => operation.promise); await flush();
    await fixture.close(21); assert.deepEqual(calls.pop(), { release: false, requestId: 21 });
    operation.resolve(); await running;
    const theme = deferred<any>(); mock.setTheme = () => theme.promise;
    const savingTheme = vm.changeTheme('forest'); await flush();
    await fixture.close(22); assert.deepEqual(calls.pop(), { release: false, requestId: 22 });
    theme.resolve({ ...vm.state.settings, theme: 'forest' }); await savingTheme;
    for (const flag of ['dataOperationBusy', 'updateBusy', 'shareBusy', 'appearanceBusy', 'privacyBusy', 'aiBusy', 'clipPrivacyBusy']) {
      vm[flag] = true; assert.equal(vm.dataBusy, true, flag);
      await fixture.close(23); assert.deepEqual(calls.pop(), { release: false, requestId: 23 }, flag);
      vm[flag] = false;
    }
    for (const flag of ['cleanupOpen', 'guideVisible', 'guideBusy']) {
      vm[flag] = true;
      await fixture.close(24); assert.deepEqual(calls.pop(), { release: false, requestId: 24 }, flag);
      vm[flag] = false;
    }
    await fixture.close(25); assert.deepEqual(calls.pop(), { release: true, requestId: 25 });
  } finally { fixture.cleanup(); }
});

test('App unregisters close listeners and ignores close events after unmount, including late subscription', async () => {
  const fixture = await mount();
  try {
    fixture.unmount(); assert.deepEqual(fixture.stopped, { change: 1, close: 1, growth: 1 });
    assert.equal(fixture.windowEvents.count(), 0); assert.equal(fixture.themeEvents.count(), 0);
    await fixture.close(30); assert.deepEqual(fixture.calls, []);
  } finally { fixture.cleanup(); }
  const pending = await mount({ delayedCloseListener: true });
  try {
    pending.unmount(); pending.finishListener(); await flush();
    assert.deepEqual(pending.stopped, { change: 1, close: 1, growth: 0 });
    await pending.close(31); assert.deepEqual(pending.calls, []);
    assert.equal(pending.windowEvents.count(), 0); assert.equal(pending.themeEvents.count(), 0);
  } finally { pending.cleanup(); }
});

test('native close API forwards release and request ID, delivers payloads and returns unlisten; browser stays inert', async () => {
  const commands: { command: string; args: unknown }[] = [], requests: number[] = [];
  let handler!: (event: { payload: { requestId: number } }) => void, registrations = 0, stopped = 0;
  const unlisten = () => { stopped++; };
  const restore = globals({
    window: { __TAURI_INTERNALS__: {} },
    windowReleaseApiMock: {
      async invoke(command: string, args: unknown) { commands.push({ command, args }); },
      async listen(name: string, callback: typeof handler) { assert.equal(name, 'main-window-close-requested'); registrations++; handler = callback; return unlisten; },
    },
  });
  const nativeUrl = moduleUrl('export const { invoke, listen } = globalThis.windowReleaseApiMock;');
  const source = fs.readFileSync(new URL('../src/api.ts', import.meta.url), 'utf8')
    .replaceAll("'@tauri-apps/api/core'", `'${nativeUrl}'`).replaceAll("'@tauri-apps/api/event'", `'${nativeUrl}'`)
    .replaceAll("'./privacy'", `'${new URL('../src/privacy.ts', import.meta.url).href}'`)
    .replaceAll("'./snippets'", `'${new URL('../src/snippets.ts', import.meta.url).href}'`);
  try {
    const { api } = await import(moduleUrl(source));
    await api.hideWindow(); await api.hideWindow(true, 41); await api.hideWindow(false, 42);
    assert.deepEqual(commands, [
      { command: 'hide_window', args: { release: false, requestId: undefined } },
      { command: 'hide_window', args: { release: true, requestId: 41 } },
      { command: 'hide_window', args: { release: false, requestId: 42 } },
    ]);
    const stop = await api.onWindowCloseRequested((requestId: number) => requests.push(requestId));
    handler({ payload: { requestId: 43 } }); assert.deepEqual(requests, [43]);
    assert.equal(stop, unlisten); stop(); assert.equal(stopped, 1);
    Object.assign(globalThis, { window: {} });
    const browser = await import(moduleUrl(source + '\n// browser close protocol'));
    await browser.api.hideWindow(true, 44); (await browser.api.onWindowCloseRequested(() => assert.fail('unexpected browser close event')))();
    assert.equal(commands.length, 3); assert.equal(registrations, 1); assert.equal(stopped, 1);
  } finally { restore(); }
});
