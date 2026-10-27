import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { compileScript, parse } from '@vue/compiler-sfc';
import { createRenderer, markRaw, nextTick } from 'vue';
import type { RecoveryPoint, UpdateStatus } from '../src/update-api.ts';

// Render the actual Vue panel in memory; no WebView, clipboard or user data is touched.
type TestNode = { type: string; text: string; props: Record<string, any>; children: TestNode[]; focus: () => void; open: boolean; showModal: () => void; close: () => void; parent?: TestNode };
function node(type: string, text = ''): TestNode { return markRaw({ type, text, props: {}, children: [], focus() {}, open: false, showModal() { this.open = true; }, close() { this.open = false; } }); }
const renderer = createRenderer<TestNode, TestNode>({
  createElement: type => node(type), createText: text => node('#text', text), createComment: text => node('#comment', text),
  setText: (node, text) => { node.text = text; }, setElementText: (node, text) => { node.text = text; node.children = []; },
  patchProp: (node, key, _previous, value) => { node.props[key] = value; },
  insert: (child, parent, anchor) => { const index = child.parent?.children.indexOf(child) ?? -1; if (index >= 0) child.parent!.children.splice(index, 1); child.parent = parent; const target = anchor ? parent.children.indexOf(anchor) : -1; parent.children.splice(target >= 0 ? target : parent.children.length, 0, child); },
  remove: child => { const index = child.parent?.children.indexOf(child) ?? -1; if (index >= 0) child.parent!.children.splice(index, 1); },
  parentNode: node => node.parent || null, nextSibling: node => node.parent?.children[(node.parent?.children.indexOf(node) ?? -1) + 1] || null,
});
const base: UpdateStatus = { revision: 0, currentVersion: '0.2.0', configured: true, phase: 'idle', downloaded: 0 };
const point: RecoveryPoint = { id: 'fixed-test-point', createdAt: 1760000000000, appVersion: '0.2.0', schemaVersion: 3, reason: 'manual', totalBytes: 1024, path: '/test-only/snapshot', warnings: [], compatible: true };
function nodes(root: TestNode): TestNode[] { return [root, ...root.children.flatMap(nodes)]; }
function text(root: TestNode): string { return root.type === '#comment' ? '' : root.text + root.children.map(text).join(''); }
function button(root: TestNode, label: string): TestNode { const found = nodes(root).find(item => item.type === 'button' && text(item) === label); assert(found, `missing button: ${label}`); return found; }
async function flush() { await new Promise(resolve => setImmediate(resolve)); await nextTick(); }
let moduleNumber = 0;
async function mount(mock: Record<string, unknown>, props: Record<string, unknown> = {}) {
  Object.assign(globalThis, { updatePanelMock: mock });
  const api = `const api = globalThis.updatePanelMock; export const { updateDesktopAvailable, initialUpdateStatus, watchStarclipUpdates, listRecoveryPoints, recoveryPendingPath, createRecoveryPoint, restoreRecoveryPoint, restartAfterUpdate } = api;`;
  const apiUrl = 'data:text/javascript;base64,' + Buffer.from(api + `\n// ${moduleNumber++}`).toString('base64');
  const source = fs.readFileSync(new URL('../src/components/UpdateSettings.vue', import.meta.url), 'utf8');
  const { descriptor } = parse(source);
  const script = compileScript(descriptor, { id: 'update-panel-test', inlineTemplate: true });
  const code = ts.transpileModule(script.content, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText
    .replace(/from (['"])vue\1/g, `from '${import.meta.resolve('vue')}'`)
    .replaceAll("'../update-api'", `'${apiUrl}'`)
    .replaceAll("'../update-state'", `'${new URL('../src/update-state.ts', import.meta.url).href}'`);
  const component = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  const root = node('root'); const app = renderer.createApp(component.default, props); app.mount(root); await flush();
  return { root, app };
}
function nativeMock(initial: UpdateStatus = base) {
  let receive: (state: UpdateStatus) => void = () => {};
  let state = { ...initial };
  const calls: string[] = [];
  const mock = {
    calls, updateDesktopAvailable: true, initialUpdateStatus: () => base,
    send(next: UpdateStatus) { state = next; receive(next); },
    async downloadUpdate(version: string) { calls.push(`download:${version}`); mock.send({ ...base, revision: 3, phase: 'ready', available: { version }, downloaded: 100, total: 100 }); },
    async installUpdate(version: string) { calls.push(`install:${version}`); mock.send({ ...base, revision: 4, phase: 'installed' }); },
    watchStarclipUpdates(callback: typeof receive) {
      receive = callback;
      return {
        async read() { calls.push('read'); receive(state); },
        async check() { calls.push('check'); mock.send({ ...base, revision: 1, phase: 'up-to-date' }); },
        download: (version: string) => mock.downloadUpdate(version),
        install: (version: string) => mock.installUpdate(version),
        dispose() { calls.push('dispose'); },
      };
    },
    async listRecoveryPoints() { return [point]; }, async recoveryPendingPath(): Promise<string | null> { return null; },
    async createRecoveryPoint() { return point; }, async restoreRecoveryPoint(id: string) { calls.push(`restore:${id}`); return point; },
    async restartAfterUpdate() { calls.push('restart'); },
  };
  return mock;
}

test('download opens a ready dialog without installing; later preserves the package and confirmation installs then restarts', async () => {
  const mock = nativeMock(); const busy: boolean[] = [];
  const { root, app } = await mount(mock, { onBusy: (value: boolean) => busy.push(value) });
  assert.deepEqual(mock.calls, ['read']);
  await button(root, '检查更新').props.onClick(); await flush();
  assert.match(text(root), /已检查，当前没有更新/);
  mock.send({ ...base, revision: 2, phase: 'available', available: { version: '0.3.0', notes: '<img src=x onerror=alert(1)>' } }); await flush();
  assert.match(text(root), /<img src=x onerror=alert\(1\)>/); assert.equal(nodes(root).some(item => item.type === 'img'), false);
  await button(root, '下载更新').props.onClick(); await flush();
  assert.match(text(root), /新版本 0.3.0 已准备就绪/);
  assert(mock.calls.includes('download:0.3.0')); assert(!mock.calls.some(call => call.startsWith('install:')));
  assert.equal(busy.at(-1), false, 'downloaded package does not freeze editing');
  let dialog = nodes(root).find(item => item.type === 'dialog')!; assert.equal(dialog.open, true);
  await button(dialog, '稍后再说').props.onClick(); await flush();
  assert.equal(dialog.open, false); assert(!nodes(root).some(item => item.type === 'dialog'));
  mock.send({ ...base, revision: 4, phase: 'ready', available: { version: '0.3.0' }, downloaded: 100, total: 100 }); await flush();
  assert(!nodes(root).some(item => item.type === 'dialog'), 'repeated ready notifications do not undo the later choice');
  assert.match(text(root), /已下载并验证，等待安装/);
  await button(root, '安装并重启').props.onClick(); await flush();
  dialog = nodes(root).find(item => item.type === 'dialog')!;
  await button(dialog, '安装并重启').props.onClick(); await flush();
  assert(mock.calls.includes('install:0.3.0'), 'installation uses the version the user confirmed');
  assert.equal(busy.at(-1), true); assert.equal(button(root, '保存当前恢复点').props.disabled, true);
  assert.deepEqual(mock.calls.slice(-2), ['install:0.3.0', 'restart'], 'confirmed installation restarts automatically');
  app.unmount(); assert.equal(busy.at(-1), false); assert(mock.calls.includes('dispose'));
});

test('restore confirmation names old data; lost restore response still freezes if the directory switch committed', async () => {
  const mock = nativeMock(); let committed = false; const busy: boolean[] = [];
  mock.restoreRecoveryPoint = async () => { committed = true; throw new Error('测试响应中断'); };
  mock.recoveryPendingPath = async () => committed ? '/test-only/restored-directory' : null;
  const { root, app } = await mount(mock, { onBusy: (value: boolean) => busy.push(value) });
  await button(root, '恢复此时数据').props.onClick(); await flush();
  assert.match(text(root), /旧历史、旧偏好与旧星伴成长/); assert.match(text(root), /不会合并/);
  await button(root, '取消').props.onClick(); await flush(); assert.equal(committed, false);
  await button(root, '恢复此时数据').props.onClick(); await flush();
  await button(root, '确认恢复旧数据').props.onClick(); await flush();
  assert.equal(busy.at(-1), true); assert.match(text(root), /\/test-only\/restored-directory/); assert.match(text(root), /测试响应中断/);
  assert.equal(button(root, '恢复此时数据').props.disabled, true);
  app.unmount();
});

test('ready state survives reopening settings; Escape and backdrop dismiss only its dialog; unsaved preferences block installation', async () => {
  const mock = nativeMock({ ...base, phase: 'ready', available: { version: '0.3.0' }, downloaded: 100, total: 100 });
  const { root, app } = await mount(mock, { unsavedChanges: true });
  let dialog = nodes(root).find(item => item.type === 'dialog')!; assert.equal(dialog.open, true);
  assert.equal(button(dialog, '安装并重启').props.disabled, true);
  await button(dialog, '安装并重启').props.onClick(); await flush();
  assert(!mock.calls.some(call => call.startsWith('install:')));
  assert.match(text(dialog), /尚未保存的偏好/);
  let stopped = 0, prevented = 0;
  dialog.props.onCancel({ preventDefault() { prevented++; }, stopPropagation() { stopped++; } }); await flush();
  assert.equal(stopped, 1); assert.equal(prevented, 1); assert.equal(dialog.open, false);
  await button(root, '安装并重启').props.onClick(); await flush();
  dialog = nodes(root).find(item => item.type === 'dialog')!;
  dialog.props.onClick({ target: dialog }); await flush();
  assert.equal(dialog.open, false); assert.match(text(root), /等待安装/);
  app.unmount();
});

test('changing the prepared version invalidates the open confirmation and stale clicks cannot install', async () => {
  const mock = nativeMock({ ...base, phase: 'ready', available: { version: '0.3.0' } });
  const { root, app } = await mount(mock);
  const dialog = nodes(root).find(item => item.type === 'dialog')!;
  const staleInstall = button(dialog, '安装并重启').props.onClick;
  mock.send({ ...base, revision: 3, phase: 'available', available: { version: '0.4.0' } }); await flush();
  assert.equal(dialog.open, false); assert(!nodes(root).some(item => item.type === 'dialog'));
  await staleInstall(); await flush();
  assert(!mock.calls.some(call => call.startsWith('install:')));
  assert(button(root, '下载更新')); app.unmount();
});

test('download or installation failure offers a new download and never restarts', async () => {
  const available: UpdateStatus = { ...base, phase: 'available', available: { version: '0.3.0' } };
  const mock = nativeMock(available); let failDownload = true;
  const successfulDownload = mock.downloadUpdate;
  mock.downloadUpdate = async version => {
    if (!failDownload) return successfulDownload(version);
    mock.send({ ...available, revision: 2, phase: 'error', error: '固定测试下载失败' });
    throw new Error('固定测试下载失败');
  };
  mock.installUpdate = async () => {
    mock.send({ ...available, revision: 4, phase: 'error', error: '固定测试备份失败' });
    throw new Error('固定测试备份失败');
  };
  const { root, app } = await mount(mock);
  await button(root, '下载更新').props.onClick(); await flush();
  assert.match(text(root), /固定测试下载失败/); assert(!nodes(root).some(item => item.type === 'dialog'));
  failDownload = false;
  await button(root, '重新下载更新').props.onClick(); await flush();
  const dialog = nodes(root).find(item => item.type === 'dialog')!;
  await button(dialog, '安装并重启').props.onClick(); await flush();
  assert.match(text(root), /固定测试备份失败/); assert(button(root, '重新下载更新'));
  assert(!mock.calls.includes('restart')); app.unmount();
});

test('lost install response recovers installed state; automatic restart failure stays frozen and can retry restart', async () => {
  const mock = nativeMock({ ...base, phase: 'ready', available: { version: '0.3.0' } }); const busy: boolean[] = [];
  mock.installUpdate = async version => { mock.calls.push(`install:${version}`); mock.send({ ...base, revision: 4, phase: 'installed' }); throw new Error('固定测试响应中断'); };
  let restartFails = true;
  mock.restartAfterUpdate = async () => { mock.calls.push('restart'); if (restartFails) throw new Error('固定测试重启失败'); };
  const { root, app } = await mount(mock, { onBusy: (value: boolean) => busy.push(value) });
  const dialog = nodes(root).find(item => item.type === 'dialog')!;
  await button(dialog, '安装并重启').props.onClick(); await flush();
  assert.match(text(root), /自动重启未完成.*再次点击/); assert.match(text(root), /固定测试重启失败/);
  assert.equal(busy.at(-1), true); assert.equal(button(root, '保存当前恢复点').props.disabled, true);
  restartFails = false;
  await button(root, '重启星笺').props.onClick(); await flush();
  assert.equal(mock.calls.filter(call => call === 'restart').length, 2);
  assert.equal(mock.calls.filter(call => call.startsWith('install:')).length, 1);
  app.unmount();
});

test('browser preview is explicit and never reads native state', async () => {
  const mock = nativeMock(); mock.updateDesktopAvailable = false;
  const { root, app } = await mount(mock);
  assert.match(text(root), /浏览器演示没有已安装的软件/);
  assert.equal(nodes(root).filter(item => item.type === 'button').length, 0);
  assert.deepEqual(mock.calls, []); app.unmount();
});


test('installer launch failure keeps editing frozen and requests restart without claiming success', async () => {
  const mock = nativeMock(); const busy: boolean[] = [];
  mock.listRecoveryPoints = async () => [{ ...point, createdAt: 0, appVersion: 'unknown', compatible: false, warnings: ['固定测试：恢复点清单损坏'] }];
  const { root, app } = await mount(mock, { onBusy: (value: boolean) => busy.push(value) });
  assert.match(text(root), /时间未知/); assert.match(text(root), /数据损坏或版本不兼容/);
  assert.equal(button(root, '恢复此时数据').props.disabled, true);
  mock.send({ ...base, revision: 4, phase: 'restart-required', error: '固定测试：安装器未启动，请先重启' }); await flush();
  assert.equal(busy.at(-1), true); assert.match(text(root), /安装未完成，请先重启星笺/);
  assert.doesNotMatch(text(root), /新版已安装/); assert.equal(button(root, '保存当前恢复点').props.disabled, true);
  await button(root, '重启星笺').props.onClick(); await flush(); assert(mock.calls.includes('restart'));
  app.unmount();
});
