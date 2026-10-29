import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { compileScript, parse } from '@vue/compiler-sfc';
import { createRenderer, markRaw, nextTick } from 'vue';
import { setLocale } from '../src/i18n/index.ts';
import type { StartupPreferences, StartupPreferencesPatch } from '../src/startup-api.ts';

// Exercise the real component with fixed state, without a WebView or login-item changes.
type TestNode = { type: string; text: string; props: Record<string, any>; children: TestNode[]; parent?: TestNode };
function node(type: string, text = ''): TestNode { return markRaw({ type, text, props: {}, children: [] }); }
const renderer = createRenderer<TestNode, TestNode>({
  createElement: type => node(type), createText: text => node('#text', text), createComment: text => node('#comment', text),
  setText: (node, text) => { node.text = text; }, setElementText: (node, text) => { node.text = text; node.children = []; },
  patchProp: (node, key, _previous, value) => { node.props[key] = value; },
  insert: (child, parent, anchor) => { const index = child.parent?.children.indexOf(child) ?? -1; if (index >= 0) child.parent!.children.splice(index, 1); child.parent = parent; const target = anchor ? parent.children.indexOf(anchor) : -1; parent.children.splice(target >= 0 ? target : parent.children.length, 0, child); },
  remove: child => { const index = child.parent?.children.indexOf(child) ?? -1; if (index >= 0) child.parent!.children.splice(index, 1); },
  parentNode: node => node.parent || null, nextSibling: node => node.parent?.children[(node.parent?.children.indexOf(node) ?? -1) + 1] || null,
});
function nodes(root: TestNode): TestNode[] { return [root, ...root.children.flatMap(nodes)]; }
function text(root: TestNode): string { return root.type === '#comment' ? '' : root.text + root.children.map(text).join(''); }
function button(root: TestNode, label: string): TestNode { const found = nodes(root).find(item => item.type === 'button' && text(item) === label); assert(found, `missing button: ${label}`); return found; }
function settingSwitch(root: TestNode, label: string): TestNode { const found = nodes(root).find(item => item.props.role === 'switch' && item.props['aria-label'] === label); assert(found, `missing switch: ${label}`); return found; }
async function flush() { await new Promise(resolve => setImmediate(resolve)); await nextTick(); }
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
let moduleNumber = 0;
async function mount(mock: Record<string, unknown>, props: Record<string, unknown> = {}) {
  Object.assign(globalThis, { startupPanelMock: mock });
  const api = 'export const { startupDesktopAvailable, initialStartupPreferences, getStartupPreferences, setStartupPreferences } = globalThis.startupPanelMock;';
  const apiUrl = 'data:text/javascript;base64,' + Buffer.from(api + `\n// ${moduleNumber++}`).toString('base64');
  const { descriptor } = parse(fs.readFileSync(new URL('../src/components/StartupSettings.vue', import.meta.url), 'utf8'));
  const script = compileScript(descriptor, { id: 'startup-panel-test', inlineTemplate: true });
  const code = ts.transpileModule(script.content, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText
    .replace(/from (['"])vue\1/g, `from '${import.meta.resolve('vue')}'`)
    .replaceAll("'../startup-api'", `'${apiUrl}'`)
    .replaceAll("'../i18n'", `'${new URL('../src/i18n/index.ts', import.meta.url).href}'`);
  const component = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  const root = node('root'); const app = renderer.createApp(component.default, props);
  const exposed = app.mount(root) as unknown as { canRelease: boolean }; await flush();
  return { root, app, exposed };
}
const defaults = (): StartupPreferences => ({ autoLaunch: false, startHidden: false, showTrayIcon: true });
const labels = ['登录系统后自动启动', '启动时收起主窗口', '显示菜单栏 / 托盘图标'];
function nativeMock(initial = defaults()) {
  let current = { ...initial };
  const calls: Array<string | StartupPreferencesPatch> = [];
  return {
    calls, startupDesktopAvailable: true, initialStartupPreferences: defaults,
    async getStartupPreferences() { calls.push('read'); return { ...current }; },
    async setStartupPreferences(patch: StartupPreferencesPatch) { calls.push(patch); current = { ...current, ...patch }; return { ...current }; },
  };
}
test.afterEach(() => setLocale('zh-CN', false));

test('each startup option saves independently, keeps hidden-tray choices, and quit delegates to the parent', async () => {
  const mock = nativeMock(); let quits = 0;
  const { root, app, exposed } = await mount(mock, { onQuit: () => { quits++; } });
  assert.deepEqual(mock.calls, ['read'], 'opening settings does not register login items or save ordinary settings');
  assert.deepEqual(labels.map(label => settingSwitch(root, label).props['aria-checked']), [false, false, true]);
  assert.equal(exposed.canRelease, true);
  for (const label of labels) await settingSwitch(root, label).props.onClick();
  await flush();
  assert.deepEqual(mock.calls, ['read', { autoLaunch: true }, { startHidden: true }, { showTrayIcon: false }]);
  assert.deepEqual(labels.map(label => settingSwitch(root, label).props['aria-checked']), [true, true, false]);
  assert.match(text(root), /星伴按已保存的开关运行/);
  assert.match(text(root), /隐藏图标后，仍可重新打开星笺应用或使用呼出快捷键/);
  await button(root, '退出星笺').props.onClick(); assert.equal(quits, 1);
  assert.equal(mock.calls.length, 4, 'quit emits an intent instead of calling an exit API itself');
  app.unmount();
});

test('saving waits for confirmation, blocks duplicates and release, and keeps the real switch state on failure', async () => {
  const mock = nativeMock(); const saving = deferred<StartupPreferences>(); const busy: boolean[] = []; let quits = 0, attempts = 0;
  mock.setStartupPreferences = async patch => { mock.calls.push(patch); return ++attempts === 1 ? saving.promise : { ...defaults(), ...patch }; };
  const { root, app, exposed } = await mount(mock, { onBusy: (value: boolean) => busy.push(value), onQuit: () => { quits++; } });
  const pending = settingSwitch(root, labels[2]).props.onClick(); await flush();
  assert.equal(settingSwitch(root, labels[2]).props['aria-checked'], true, 'a button switch never changes its confirmed DOM state before the result');
  assert.equal(exposed.canRelease, false); assert.equal(busy.at(-1), true);
  for (const label of labels) { assert.equal(settingSwitch(root, label).props.disabled, true); await settingSwitch(root, label).props.onClick(); }
  await button(root, '退出星笺').props.onClick(); assert.equal(quits, 0); assert.equal(mock.calls.length, 2);
  saving.reject(new Error('固定测试：磁盘不可写')); await pending; await flush();
  assert.equal(settingSwitch(root, labels[2]).props['aria-checked'], true);
  assert.equal(exposed.canRelease, true); assert.equal(busy.at(-1), false);
  assert.match(text(root), /未能保存启动设置：固定测试：磁盘不可写/);
  await button(root, '重新读取').props.onClick(); await flush();
  assert.equal(settingSwitch(root, labels[2]).props['aria-checked'], true);
  assert.doesNotMatch(text(root), /未能保存/);
  await settingSwitch(root, labels[2]).props.onClick(); await flush();
  assert.equal(settingSwitch(root, labels[2]).props['aria-checked'], false);
  assert.deepEqual(mock.calls, ['read', { showTrayIcon: false }, 'read', { showTrayIcon: false }]);
  assert.doesNotMatch(text(root), /未能保存/);
  app.unmount();
});

test('a failed initial read can retry without presenting editable defaults or preventing exit', async () => {
  const mock = nativeMock(); const read = mock.getStartupPreferences; let attempts = 0;
  mock.getStartupPreferences = async () => { if (++attempts === 1) throw new Error('固定测试：无法读取'); return read(); };
  const { root, app, exposed } = await mount(mock);
  assert.match(text(root), /启动设置读取失败：固定测试：无法读取/);
  for (const label of labels) { assert.equal(settingSwitch(root, label).props.disabled, true); await settingSwitch(root, label).props.onClick(); }
  assert.equal(exposed.canRelease, true); assert.equal(button(root, '退出星笺').props.disabled, false);
  assert.deepEqual(mock.calls, []);
  await button(root, '重新读取').props.onClick(); await flush();
  assert.equal(attempts, 2); assert.deepEqual(mock.calls, ['read']);
  assert.equal(settingSwitch(root, labels[0]).props.disabled, false);
  assert.doesNotMatch(text(root), /读取失败/); app.unmount();
});

test('unmount clears busy and late reads or saves cannot update the component or emit quit', async () => {
  for (const operation of ['read', 'save']) {
    const mock = nativeMock(); const result = deferred<StartupPreferences>(); const busy: boolean[] = []; let quits = 0;
    if (operation === 'read') mock.getStartupPreferences = () => result.promise;
    else mock.setStartupPreferences = () => result.promise;
    const { root, app, exposed } = await mount(mock, { onBusy: (value: boolean) => busy.push(value), onQuit: () => { quits++; } });
    const quit = button(root, '退出星笺').props.onClick;
    if (operation === 'save') settingSwitch(root, labels[0]).props.onClick();
    await flush(); assert.equal(exposed.canRelease, false);
    app.unmount(); assert.equal(busy.at(-1), false); const events = busy.length;
    result.resolve({ autoLaunch: true, startHidden: true, showTrayIcon: false }); await flush();
    quit(); assert.equal(quits, 0); assert.equal(busy.length, events, 'late results do not emit fresh activity');
  }
});

test('browser and parent-disabled controls remain inactive, including direct handler calls', async () => {
  for (const desktop of [false, true]) {
    const mock = nativeMock(); mock.startupDesktopAvailable = desktop; let quits = 0;
    const { root, app, exposed } = await mount(mock, { disabled: desktop, onQuit: () => { quits++; } });
    if (!desktop) assert.match(text(root), /浏览器不会注册登录项或更改菜单栏/);
    for (const label of labels) { assert.equal(settingSwitch(root, label).props.disabled, true); await settingSwitch(root, label).props.onClick(); }
    assert.equal(button(root, '退出星笺').props.disabled, true); await button(root, '退出星笺').props.onClick();
    assert.deepEqual(mock.calls, desktop ? ['read'] : []); assert.equal(quits, 0); assert.equal(exposed.canRelease, true);
    app.unmount();
  }
});

test('English startup controls, descriptions and save feedback are translated', async () => {
  setLocale('en-US', false);
  const mock = nativeMock(); const { root, app } = await mount(mock);
  assert.equal(settingSwitch(root, 'Launch automatically at login').props['aria-checked'], false);
  for (const label of ['Launch automatically at login', 'Start with the main window hidden', 'Show menu bar / tray icon']) {
    await settingSwitch(root, label).props.onClick(); await flush();
    assert.doesNotMatch(text(root), /[\u4e00-\u9fff]/);
  }
  assert(button(root, 'Quit Starclip')); app.unmount();
});
