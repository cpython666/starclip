import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { compileScript, parse } from '@vue/compiler-sfc';
import { createRenderer, markRaw, nextTick } from 'vue';
import type { TelemetryState } from '../src/telemetry-api.ts';

// Exercise the real settings panel without a WebView, network or clipboard.
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
async function flush() { await new Promise(resolve => setImmediate(resolve)); await nextTick(); }
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
let moduleNumber = 0;
async function mount(mock: Record<string, unknown>, props: Record<string, unknown> = {}) {
  Object.assign(globalThis, { telemetryPanelMock: mock });
  const api = `const api = globalThis.telemetryPanelMock; export const { ${Object.keys(mock).filter(key => /^[A-Za-z_$][\w$]*$/.test(key)).join(', ')} } = api;`;
  const apiUrl = 'data:text/javascript;base64,' + Buffer.from(api + `\n// ${moduleNumber++}`).toString('base64');
  const source = fs.readFileSync(new URL('../src/components/TelemetrySettings.vue', import.meta.url), 'utf8');
  const { descriptor } = parse(source);
  const script = compileScript(descriptor, { id: 'telemetry-panel-test', inlineTemplate: true });
  const code = ts.transpileModule(script.content, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText
    .replace(/from (['"])vue\1/g, `from '${import.meta.resolve('vue')}'`)
    .replaceAll("'../telemetry-api'", `'${apiUrl}'`)
    .replaceAll("'../i18n'", `'${new URL('../src/i18n/index.ts', import.meta.url).href}'`);
  const component = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  const root = node('root'); const app = renderer.createApp(component.default, props);
  const exposed = app.mount(root) as unknown as { canRelease: boolean }; await flush();
  return { root, app, exposed };
}

async function loadApi(desktop: boolean) {
  Object.assign(globalThis, { window: desktop ? { __TAURI_INTERNALS__: {} } : {} });
  const source = fs.readFileSync(new URL('../src/telemetry-api.ts', import.meta.url), 'utf8')
    .replaceAll("import('@tauri-apps/api/core')", 'Promise.resolve(globalThis.telemetryTestCore)')
    .replaceAll("import('@tauri-apps/api/event')", 'Promise.resolve(globalThis.telemetryTestEvents)');
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  return import('data:text/javascript;base64,' + Buffer.from(code + `\n// module ${moduleNumber++}`).toString('base64'));
}

const initialState = (): TelemetryState => ({ revision: 0, enabled: false, configured: false, endpointHost: null, queuedEvents: [], lastResult: 'never', installHint: null });
const eventNames = ['app_started', 'studio_opened', 'card_exported', 'backup_exported', 'backup_imported', 'update_check_completed', 'update_download_completed', 'update_install_started', 'font_downloaded', 'app_active'];
function nativeMock(initial: TelemetryState = initialState()) {
  let state = { ...initial };
  let receive: (value: TelemetryState) => void = () => {};
  const calls: string[] = [];
  const mock = {
    calls, telemetryDesktopAvailable: true, usageEventNames: eventNames, initialTelemetryState: initialState,
    async getTelemetryState() { calls.push('read'); return { ...state }; },
    async onTelemetryStateChanged(callback: typeof receive) { calls.push('listen'); receive = callback; return () => { calls.push('dispose'); }; },
    send(value: TelemetryState) { state = value; receive(value); },
    async setTelemetryEnabled(enabled: boolean) {
      calls.push(`enabled:${enabled}`);
      state = { ...state, revision: state.revision + 1, enabled, queuedEvents: enabled ? state.queuedEvents : [], lastResult: enabled ? 'never' : 'disabled', installHint: enabled ? state.installHint : null };
      return { ...state };
    },
    async flushTelemetry() { calls.push('send'); state = { ...state, revision: state.revision + 1, queuedEvents: [], lastResult: 'sent' }; return { ...state }; },
    async clearTelemetryQueue() { calls.push('clear'); state = { ...state, revision: state.revision + 1, queuedEvents: [] }; return { ...state }; },
    async resetTelemetryInstall() { calls.push('reset'); state = { ...state, revision: state.revision + 1, installHint: 'bbbbbb', queuedEvents: [{ name: 'app_active', count: 1 }] }; return { ...state }; },
  };
  return mock;
}
function telemetrySwitch(root: TestNode): TestNode {
  const found = nodes(root).find(item => item.props.role === 'switch' && item.props['aria-label'] === '帮助改进星笺');
  assert(found, 'missing usage consent switch'); return found;
}

test('an unconfigured receiver cannot enable or send despite the default-on policy', async () => {
  const mock = nativeMock(); const { root, app } = await mount(mock);
  assert.deepEqual(mock.calls, ['listen', 'read'], 'opening settings only reads local state');
  assert.equal(telemetrySwitch(root).props['aria-checked'], false);
  assert.equal(telemetrySwitch(root).props.disabled, true);
  assert.match(text(root), /默认开启 · 随时关闭 · 独立即时保存/);
  assert.match(text(root), /接收端未配置，当前不会收集或发送使用统计/);
  assert.equal(button(root, '发送待发统计').props.disabled, true);
  await telemetrySwitch(root).props.onClick(); await button(root, '发送待发统计').props.onClick(); await flush();
  assert.deepEqual(mock.calls, ['listen', 'read']);
  app.unmount(); assert.equal(mock.calls.at(-1), 'dispose');
});

test('configured native preferences show default-on or a saved opt-out without changing either choice', async () => {
  for (const enabled of [true, false]) {
    const mock = nativeMock({ ...initialState(), configured: true, enabled, endpointHost: 'fixture.example' });
    const { root, app } = await mount(mock);
    assert.equal(telemetrySwitch(root).props['aria-checked'], enabled);
    assert.equal(telemetrySwitch(root).props.disabled, false);
    assert.match(text(root), /默认开启 · 随时关闭 · 独立即时保存/);
    assert.deepEqual(mock.calls, ['listen', 'read'], 'mounting must not enable, disable or flush telemetry');
    app.unmount();
  }
});

test('statistics deliveries and network failures stay silent, including an IPC rejection', async () => {
  const mock = nativeMock({ ...initialState(), configured: true, enabled: true, queuedEvents: [{ name: 'app_started', count: 1 }] });
  let rejectSend = false;
  const send = mock.flushTelemetry;
  mock.flushTelemetry = async () => { if (rejectSend) throw new Error('FIXTURE_NETWORK_FAILURE'); return send(); };
  const { root, app } = await mount(mock);
  assert.match(text(root), /发送给 Google Analytics 4；/);
  assert.doesNotMatch(text(root), /Microsoft|Application Insights|两个服务|免费版/);
  await button(root, '发送待发统计').props.onClick(); await flush();
  assert.doesNotMatch(text(root), /使用统计已发送|暂时未能发送统计/);
  assert.doesNotMatch(text(root), /Microsoft|Application Insights|两个服务|报表|入库/);
  mock.send({ ...initialState(), revision: 2, configured: true, enabled: true, lastResult: 'failed' }); await flush();
  assert.doesNotMatch(text(root), /使用统计已发送|暂时未能发送统计/);
  assert.deepEqual(mock.calls, ['listen', 'read', 'send']);
  mock.send({ ...initialState(), revision: 3, configured: true, enabled: true, queuedEvents: [{ name: 'studio_opened', count: 1 }] }); await flush();
  rejectSend = true;
  await button(root, '发送待发统计').props.onClick(); await flush();
  assert.doesNotMatch(text(root), /FIXTURE_NETWORK_FAILURE/);
  assert.equal(nodes(root).some(item => item.props.role === 'alert'), false);
  assert.match(text(root), /最多保留 72 小时、每种 100 次/);
  assert.match(text(root), /不收集序列号、电脑名称、用户名或硬件标识/);
  app.unmount();
});

test('enabling waits for confirmation and a failed save keeps consent off', async () => {
  const mock = nativeMock({ ...initialState(), configured: true, endpointHost: 'fixture.example' });
  const saving = deferred<TelemetryState>(); const busy: boolean[] = [];
  let attempts = 0;
  mock.setTelemetryEnabled = async enabled => {
    mock.calls.push(`enabled:${enabled}`);
    return ++attempts === 1 ? saving.promise : { ...initialState(), revision: 1, configured: true, enabled };
  };
  const { root, app } = await mount(mock, { onBusy: (value: boolean) => busy.push(value) });
  assert.equal(telemetrySwitch(root).props.disabled, false);
  const toggle = telemetrySwitch(root).props.onClick(); await flush();
  assert.equal(telemetrySwitch(root).props['aria-checked'], false, 'consent is not optimistically enabled');
  assert.equal(telemetrySwitch(root).props.disabled, true); assert.equal(busy.at(-1), true);
  await telemetrySwitch(root).props.onClick(); assert.equal(mock.calls.filter(call => call === 'enabled:true').length, 1);
  saving.reject(new Error('固定测试：统计偏好保存失败')); await toggle; await flush();
  assert.equal(telemetrySwitch(root).props['aria-checked'], false);
  assert.equal(telemetrySwitch(root).props.disabled, true); assert.equal(busy.at(-1), false);
  assert.match(text(root), /统计偏好保存失败/); assert(!mock.calls.includes('send'));
  await button(root, '重试保存统计选择').props.onClick(); await flush();
  assert.deepEqual(mock.calls.filter(call => call.startsWith('enabled:')), ['enabled:true', 'enabled:true']);
  assert.equal(telemetrySwitch(root).props.disabled, false);
  assert.equal(telemetrySwitch(root).props['aria-checked'], true);
  app.unmount();
});

test('confirmed disable clears the displayed pending queue without flushing it', async () => {
  const mock = nativeMock({ ...initialState(), revision: 3, configured: true, enabled: true, queuedEvents: [{ name: 'card_exported', count: 2 }] });
  const { root, app } = await mount(mock);
  assert.match(text(root), /导出图片卡片 · 2/);
  await telemetrySwitch(root).props.onClick(); await flush();
  assert.equal(telemetrySwitch(root).props['aria-checked'], false);
  assert.match(text(root), /没有待发送事件/); assert.match(text(root), /统计已关闭，待发送事件已清空/);
  assert.equal(button(root, '发送待发统计').props.disabled, true);
  assert.deepEqual(mock.calls, ['listen', 'read', 'enabled:false']);
  app.unmount();
});

test('retrying a failed consent write preserves the disable choice after runtime collection already stopped', async () => {
  const mock = nativeMock({ ...initialState(), revision: 4, configured: true, enabled: true, queuedEvents: [{ name: 'studio_opened', count: 2 }] });
  const save = mock.setTelemetryEnabled; let attempts = 0;
  mock.setTelemetryEnabled = async enabled => {
    if (++attempts > 1) return save(enabled);
    mock.calls.push(`enabled:${enabled}`);
    mock.send({ ...initialState(), revision: 5, configured: true, enabled: false, lastResult: 'disabled' });
    throw new Error('固定测试：本次已停止，但偏好保存失败，重启前请重试。');
  };
  const { root, app, exposed } = await mount(mock);
  assert.equal(exposed.canRelease, true);
  await telemetrySwitch(root).props.onClick(); await flush();
  assert.equal(telemetrySwitch(root).props['aria-checked'], false);
  assert.match(text(root), /没有待发送事件/); assert.match(text(root), /本次已停止，但偏好保存失败/);
  assert.equal(exposed.canRelease, false, 'an unsaved consent choice keeps the settings state alive');
  assert.equal(telemetrySwitch(root).props.disabled, true);
  await telemetrySwitch(root).props.onClick(); await flush();
  assert.deepEqual(mock.calls.filter(call => call.startsWith('enabled:')), ['enabled:false']);
  await button(root, '重试保存统计选择').props.onClick(); await flush();
  assert.deepEqual(mock.calls.filter(call => call.startsWith('enabled:')), ['enabled:false', 'enabled:false']);
  assert.equal(telemetrySwitch(root).props['aria-checked'], false);
  assert.equal(exposed.canRelease, true); assert.doesNotMatch(text(root), /重试保存统计选择|偏好保存失败/);
  assert(!mock.calls.includes('send')); app.unmount();
});

test('event preview renders only fixed labels and bounded counts, never arbitrary payload fields', async () => {
  const mock = nativeMock({
    ...initialState(), configured: true, enabled: true,
    queuedEvents: [
      { name: 'card_exported', count: 3, payload: 'FIXTURE_PRIVATE_PAYLOAD', content: '<img src=x>' },
      { name: 'update_download_completed', count: 2 },
      { name: 'update_install_started', count: 1 },
      { name: 'FIXTURE_UNKNOWN_EVENT', count: 7 },
      { name: 'studio_opened', count: 'FIXTURE_TEXT_COUNT' },
      { name: 'backup_imported', count: -1 },
      { name: 'font_downloaded', count: 101 },
    ],
  } as unknown as TelemetryState);
  const { root, app } = await mount(mock);
  assert.deepEqual(nodes(root).filter(item => item.type === 'li').map(text), ['导出图片卡片 · 3', '更新包下载完成 · 2', '开始安装更新 · 1']);
  assert.match(text(root), /统计在后台静默发送，失败不提示/);
  assert.match(text(root), /安装计数表示开始尝试，不代表安装成功；退出前未送达的计数可能丢失/);
  assert.doesNotMatch(text(root), /FIXTURE_|<img src=x>/);
  assert.equal(nodes(root).some(item => item.type === 'img'), false);
  assert(!mock.calls.includes('send'), 'preview does not send events'); app.unmount();
});

test('a newer event wins over both a delayed initial read and a delayed preference response', async () => {
  const mock = nativeMock(); const read = deferred<TelemetryState>();
  mock.getTelemetryState = async () => { mock.calls.push('read'); return read.promise; };
  const { root, app } = await mount(mock);
  mock.send({ ...initialState(), revision: 5, configured: true, enabled: true, queuedEvents: [{ name: 'studio_opened', count: 4 }] });
  read.resolve({ ...initialState(), revision: 2 }); await flush();
  assert.equal(telemetrySwitch(root).props['aria-checked'], true); assert.match(text(root), /打开星绘坊 · 4/);
  const saved = deferred<TelemetryState>();
  mock.setTelemetryEnabled = async enabled => { mock.calls.push(`enabled:${enabled}`); return saved.promise; };
  const disabling = telemetrySwitch(root).props.onClick(); await flush();
  mock.send({ ...initialState(), revision: 7, configured: true, enabled: false, lastResult: 'disabled' });
  saved.resolve({ ...initialState(), revision: 6, configured: true, enabled: true, queuedEvents: [{ name: 'studio_opened', count: 9 }] });
  await disabling; await flush();
  assert.equal(telemetrySwitch(root).props['aria-checked'], false); assert.match(text(root), /没有待发送事件/);
  assert.doesNotMatch(text(root), /打开星绘坊 · 9/); app.unmount();
});

test('browser panel remains offline and disabled even when handlers are invoked directly', async () => {
  const mock = nativeMock(); mock.telemetryDesktopAvailable = false;
  const { root, app } = await mount(mock);
  assert.match(text(root), /使用统计设置仅在桌面版可用/);
  assert.equal(telemetrySwitch(root).props.disabled, true);
  await telemetrySwitch(root).props.onClick(); await button(root, '发送待发统计').props.onClick();
  await button(root, '清空待发送事件').props.onClick(); await flush();
  assert.deepEqual(mock.calls, []); app.unmount(); assert.deepEqual(mock.calls, []);
});

test('late telemetry subscriptions are released without reading after unmount', async () => {
  const mock = nativeMock(); const listening = deferred<() => void>(); let stopped = 0;
  mock.onTelemetryStateChanged = () => listening.promise;
  const { app } = await mount(mock); app.unmount();
  listening.resolve(() => { stopped++; }); await flush();
  assert.equal(stopped, 1); assert.deepEqual(mock.calls, []);
});

test('retry after subscription failure reconnects before reading local telemetry state', async () => {
  const mock = nativeMock(); const subscribe = mock.onTelemetryStateChanged; let attempts = 0;
  mock.onTelemetryStateChanged = async callback => {
    if (++attempts === 1) throw new Error('固定测试：统计订阅失败');
    return subscribe(callback);
  };
  const { root, app } = await mount(mock);
  assert.match(text(root), /统计订阅失败/); assert.deepEqual(mock.calls, []);
  await button(root, '重新读取').props.onClick(); await flush();
  assert.equal(attempts, 2); assert.deepEqual(mock.calls, ['listen', 'read']);
  mock.send({ ...initialState(), revision: 3, configured: true, enabled: true }); await flush();
  assert.equal(telemetrySwitch(root).props['aria-checked'], true); app.unmount();
});

test('the install ID is disclosed, shown only as a prefix and can be reset', async () => {
  const mock = nativeMock({ ...initialState(), configured: true, enabled: true, endpointHost: 'fixture.example', installHint: 'aaaaaa' });
  const { root, app } = await mount(mock);
  assert.match(text(root), /统计使用人数、每日活跃和留存/);
  assert.match(text(root), /关闭统计即删除/);
  assert.match(text(root), /本机统计编号：aaaaaa…/);
  await button(root, '重置统计编号').props.onClick(); await flush();
  assert.match(text(root), /本机统计编号：bbbbbb…/);
  assert.match(text(root), /当日使用 · 1/);
  // Anything other than a short hex prefix is never rendered.
  mock.send({ ...initialState(), revision: 9, configured: true, enabled: true, installHint: 'fixture-not-hex-0123456789abcdef' }); await flush();
  assert.doesNotMatch(text(root), /本机统计编号：|fixture-not-hex/);
  assert.equal(nodes(root).some(item => item.type === 'button' && text(item) === '重置统计编号'), false);
  await telemetrySwitch(root).props.onClick(); await flush();
  assert.deepEqual(mock.calls, ['listen', 'read', 'reset', 'enabled:false']);
  assert.doesNotMatch(text(root), /本机统计编号：/);
  app.unmount();
});

test('actual browser API never invokes native IPC or subscribes, and studio events are ignored', async () => {
  let invokes = 0, subscriptions = 0;
  Object.assign(globalThis, {
    telemetryTestCore: { async invoke() { invokes++; throw new Error('unexpected native invoke'); } },
    telemetryTestEvents: { async listen() { subscriptions++; throw new Error('unexpected native subscription'); } },
  });
  const api = await loadApi(false);
  assert.equal(api.telemetryDesktopAvailable, false);
  assert.deepEqual(api.initialTelemetryState(), initialState());
  assert.deepEqual([...api.usageEventNames].sort(), [...eventNames].sort());
  await assert.rejects(api.getTelemetryState(), /桌面版/);
  await assert.rejects(api.setTelemetryEnabled(true), /桌面版/);
  await assert.rejects(api.flushTelemetry(), /桌面版/);
  await assert.rejects(api.clearTelemetryQueue(), /桌面版/);
  await assert.rejects(api.resetTelemetryInstall(), /桌面版/);
  (await api.onTelemetryStateChanged(() => assert.fail('browser event')))();
  api.recordStudioOpened(); await flush();
  assert.equal(invokes, 0); assert.equal(subscriptions, 0);
  delete (globalThis as { window?: unknown }).window;
});
