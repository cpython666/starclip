import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { compileScript, parse } from '@vue/compiler-sfc';
import { createRenderer, markRaw, nextTick } from 'vue';
import { emptyGrowth } from '../src/starpet-growth.ts';
import type { PetSnapshot } from '../src/starpet-signal-api.ts';
import type { StarPetMoodId } from '../src/starpet.ts';
import type { StarPetInteraction } from '../src/starpet-growth-api.ts';

// A memory DOM runs the actual Vue template and handlers without a desktop or clipboard.
type Listener = (event: any) => void;
function eventHub() {
  const listeners = new Map<string, Map<Listener, boolean>>();
  return {
    hidden: false, activeElement: null as MemoryNode | null,
    addEventListener(type: string, listener: Listener, capture = false) {
      if (!listeners.has(type)) listeners.set(type, new Map());
      listeners.get(type)!.set(listener, capture);
    },
    removeEventListener(type: string, listener: Listener, capture = false) {
      const registered = listeners.get(type);
      if (registered?.get(listener) === capture) registered.delete(listener);
    },
    emit(type: string, target?: MemoryNode) { listeners.get(type)?.forEach((_capture, listener) => listener({ type, target })); },
    count() { return [...listeners.values()].reduce((sum, set) => sum + set.size, 0); },
    capture(type: string) { return [...(listeners.get(type)?.values() ?? [])]; },
  };
}
let currentDocument: ReturnType<typeof eventHub>;
class MemoryNode {
  type: string; text: string; props: Record<string, any> = {}; children: MemoryNode[] = []; parent?: MemoryNode; focusCount = 0;
  constructor(type: string, text = '') { this.type = type; this.text = text; markRaw(this); }
  contains(target: MemoryNode): boolean { return this === target || this.children.some(child => child.contains(target)); }
  querySelector() { return nodes(this).find(item => item.type === 'button' && !item.props.disabled) ?? null; }
  focus() { this.focusCount++; currentDocument.activeElement = this; currentDocument.emit('focusin', this); }
}
const renderer = createRenderer<MemoryNode, MemoryNode>({
  createElement: type => new MemoryNode(type), createText: text => new MemoryNode('#text', text), createComment: text => new MemoryNode('#comment', text),
  setText: (node, text) => { node.text = text; }, setElementText: (node, text) => { node.text = text; node.children = []; },
  patchProp: (node, key, _previous, value) => { node.props[key] = value; },
  insert: (child, parent, anchor) => { const previous = child.parent?.children.indexOf(child) ?? -1; if (previous >= 0) child.parent!.children.splice(previous, 1); child.parent = parent; const index = anchor ? parent.children.indexOf(anchor) : -1; parent.children.splice(index >= 0 ? index : parent.children.length, 0, child); },
  remove: child => { const index = child.parent?.children.indexOf(child) ?? -1; if (index >= 0) child.parent!.children.splice(index, 1); },
  parentNode: node => node.parent || null, nextSibling: node => node.parent?.children[(node.parent?.children.indexOf(node) ?? -1) + 1] || null,
});
function nodes(root: MemoryNode): MemoryNode[] { return [root, ...root.children.flatMap(nodes)]; }
function text(root: MemoryNode): string { return root.type === '#comment' ? '' : root.text + root.children.map(text).join(''); }
function button(root: MemoryNode, label: string) { const found = nodes(root).find(item => item.type === 'button' && (item.props['aria-label'] === label || text(item) === label)); assert(found, `missing ${label}`); return found; }
function opened(root: MemoryNode) { return button(root, '和星芽互动').props['aria-expanded']; }
async function flush() { await new Promise(resolve => setImmediate(resolve)); await nextTick(); }
let sequence = 0;
async function mount(options: { delayedBlur?: boolean; delayedSignal?: boolean; delayedSignalListener?: boolean; delayedInteractionListener?: boolean; initialSignal?: PetSnapshot; initialMood?: StarPetMoodId } = {}) {
  const document = currentDocument = eventHub(), window = eventHub();
  const globals = ['document', 'window', 'Node', 'petDesktopMenuMock'].map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)] as const);
  let nativeBlur = () => {}, stopBlur = 0, finishBlur!: (stop: () => void) => void;
  const calls: string[] = [];
  let signalCallback = (_snapshot: PetSnapshot) => {}, finishSignal!: (snapshot: PetSnapshot) => void, finishSignalListener!: (stop: () => void) => void, stopSignal = 0;
  let interactionCallback = (_interaction: StarPetInteraction) => {}, finishInteractionListener!: (stop: () => void) => void, stopInteraction = 0;
  let interactionRevision = options.initialSignal?.revision ?? 0;
  let preferences = { enabled: true, mood: options.initialMood ?? 'idle', color: '#FFD66E', size: 100 };
  const mock = {
    async getStarPetPreferences() { return { ...preferences }; },
    async onStarPetChanged() { return () => {}; },
    async setStarPetPreferences(patch: object) { calls.push('preferences'); return preferences = { ...preferences, ...patch }; },
    async showStarPetMainWindow() { calls.push('main'); }, async dragStarPet() { calls.push('drag'); },
    async getStarPetGrowth() { return emptyGrowth(); }, async onStarPetGrowthChanged() { return () => {}; },
    async interactStarPet(action: StarPetInteraction['action'], animate = true) { calls.push('interact'); interactionCallback({ action, animate, signalRevision: ++interactionRevision }); return emptyGrowth(); },
    onStarPetInteraction(callback: (interaction: StarPetInteraction) => void): Promise<() => void> {
      interactionCallback = callback;
      return options.delayedInteractionListener ? new Promise(resolve => { finishInteractionListener = resolve; }) : Promise.resolve(() => { stopInteraction++; });
    },
    getStarPetSignal(): Promise<PetSnapshot> { return options.delayedSignal ? new Promise(resolve => { finishSignal = resolve; }) : Promise.resolve(options.initialSignal ?? { revision: 0, signal: null }); },
    onStarPetSignal(callback: (snapshot: PetSnapshot) => void): Promise<() => void> {
      signalCallback = callback;
      return options.delayedSignalListener ? new Promise(resolve => { finishSignalListener = resolve; }) : Promise.resolve(() => { stopSignal++; });
    },
    onStarPetWindowBlur(callback: () => void): Promise<() => void> {
      nativeBlur = callback;
      return options.delayedBlur ? new Promise(resolve => { finishBlur = resolve; }) : Promise.resolve(() => { stopBlur++; });
    },
  };
  Object.assign(globalThis, { document, window, Node: MemoryNode, petDesktopMenuMock: mock });
  const api = `const api=globalThis.petDesktopMenuMock; export const { getStarPetPreferences, onStarPetChanged, setStarPetPreferences, showStarPetMainWindow, dragStarPet, getStarPetGrowth, onStarPetGrowthChanged, onStarPetWindowBlur, getStarPetSignal, onStarPetSignal, onStarPetInteraction }=api; export { petStatusLabel, petStatusMood } from '${new URL('../src/starpet-signal-api.ts', import.meta.url).href}'; export const interactStarPet=(...args)=>api.interactStarPet(...args); // ${sequence++}`;
  const apiUrl = 'data:text/javascript;base64,' + Buffer.from(api).toString('base64');
  const { descriptor } = parse(fs.readFileSync(new URL('../src/components/StarPetDesktop.vue', import.meta.url), 'utf8'));
  const script = compileScript(descriptor, { id: 'starpet-menu-test', inlineTemplate: true });
  const code = ts.transpileModule(script.content, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText
    .replace(/from (['"])vue\1/g, `from '${import.meta.resolve('vue')}'`)
    .replaceAll("'../starpet-signal-api'", `'${apiUrl}'`).replaceAll("'../starpet-api'", `'${apiUrl}'`).replaceAll("'../starpet-growth-api'", `'${apiUrl}'`)
    .replaceAll("'../starpet'", `'${new URL('../src/starpet.ts', import.meta.url).href}'`)
    .replaceAll("'../starpet-growth'", `'${new URL('../src/starpet-growth.ts', import.meta.url).href}'`)
    .replaceAll("'./Icon.vue'", "'data:text/javascript,export default {render(){return null}}'")
    .replaceAll("'./StarPet.vue'", `'data:text/javascript;base64,${Buffer.from(`import {h} from '${import.meta.resolve('vue')}'; export default {props:['mood','action','actionNonce'],render(){return h('pet-fixture',{...this.$props})}}`).toString('base64')}'`).replaceAll("'../i18n'", `'${new URL('../src/i18n/index.ts', import.meta.url).href}'`).replaceAll("'./i18n'", `'${new URL('../src/i18n/index.ts', import.meta.url).href}'`);
  const component = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  const root = new MemoryNode('root'), app = renderer.createApp(component.default); app.mount(root); await flush();
  const open = async () => { assert.equal(opened(root), false); await button(root, '和星芽互动').props.onClick(); await flush(); assert.equal(opened(root), true); };
  let unmounted = false;
  const unmount = () => { if (!unmounted) { unmounted = true; app.unmount(); } };
  return { root, mock, calls, document, window, open, unmount, nativeBlur: () => nativeBlur(), stopBlur: () => stopBlur,
    finishBlur: () => finishBlur(() => { stopBlur++; }),
    signal: (snapshot: PetSnapshot) => { interactionRevision = Math.max(interactionRevision, snapshot.revision); signalCallback(snapshot); }, finishSignal: (snapshot: PetSnapshot) => finishSignal(snapshot),
    finishSignalListener: () => finishSignalListener(() => { stopSignal++; }), stopSignal: () => stopSignal,
    interaction: (value: StarPetInteraction) => interactionCallback(value),
    finishInteractionListener: () => finishInteractionListener(() => { stopInteraction++; }), stopInteraction: () => stopInteraction,
    cleanup() { unmount(); for (const [key, descriptor] of globals) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else Reflect.deleteProperty(globalThis, key); } },
  };
}

test('pet menu keeps inside clicks, toggles once and dismisses for outside clicks or other controls', async () => {
  const fixture = await mount(); const { root, document, open } = fixture;
  try {
    assert.deepEqual(document.capture('pointerdown'), [true], 'outside press runs before other controls');
    await open();
    const item = button(root, '问个好');
    document.emit('pointerdown', item); document.emit('focusin', item); await flush(); assert.equal(opened(root), true);
    const trigger = button(root, '和星芽互动');
    document.emit('pointerdown', trigger); await trigger.props.onClick(); await flush(); assert.equal(opened(root), false, 'trigger must not reopen after outside dismissal');
    await open(); const focusCount = trigger.focusCount;
    document.emit('pointerdown', new MemoryNode('outside')); await flush();
    assert.equal(opened(root), false); assert.equal(trigger.focusCount, focusCount, 'outside press must not steal focus');
    for (const label of ['打开星笺', '换个心情', '收起星伴']) {
      await open(); await button(root, label).props.onClick(); await flush(); assert.equal(opened(root), false, label);
    }
    assert.deepEqual(fixture.calls, ['main', 'preferences', 'preferences']);
  } finally { fixture.cleanup(); }
});

test('pet menu returns focus only on explicit close and releases all outside/blur listeners', async () => {
  const fixture = await mount(); const { root, document, window, open } = fixture;
  try {
    const trigger = button(root, '和星芽互动');
    await open(); await button(root, '关闭互动菜单').props.onClick(); await flush();
    assert.equal(opened(root), false); assert.equal(document.activeElement, trigger);
    await open(); nodes(root).find(item => item.type === 'main')!.props.onKeydown({ key: 'Escape', stopPropagation() {} }); await flush();
    assert.equal(opened(root), false); assert.equal(document.activeElement, trigger);
    for (const dismiss of [() => fixture.nativeBlur(), () => window.emit('blur'), () => document.emit('focusin', new MemoryNode('outside')), () => { document.hidden = true; document.emit('visibilitychange'); }]) {
      await open(); const count = trigger.focusCount; dismiss(); await flush(); assert.equal(opened(root), false); assert.equal(trigger.focusCount, count);
    }
    fixture.unmount(); assert.equal(document.count(), 0); assert.equal(window.count(), 0); assert.equal(fixture.stopBlur(), 1); assert.equal(fixture.stopInteraction(), 1);
  } finally { fixture.cleanup(); }
});

test('pet interaction dismisses immediately and its delayed result cannot close a reopened menu or steal focus', async () => {
  const fixture = await mount(); const { root, mock, open, document } = fixture;
  let finish!: (value: ReturnType<typeof emptyGrowth>) => void;
  mock.interactStarPet = () => new Promise(resolve => { finish = resolve; });
  try {
    await open(); const work = button(root, '问个好').props.onClick(); await flush(); assert.equal(opened(root), false);
    await open(); const focused = document.activeElement;
    finish(emptyGrowth()); await work; await flush();
    assert.equal(opened(root), true, 'old action result must not dismiss a new menu'); assert.equal(document.activeElement, focused);
    const second = button(root, '问个好').props.onClick(); await flush(); assert.equal(opened(root), false);
    const outside = new MemoryNode('outside'); outside.focus(); fixture.window.emit('blur');
    finish(emptyGrowth()); await second; await flush(); assert.equal(document.activeElement, outside);
  } finally { fixture.cleanup(); }
});

test('late native subscription and opening nextTick are safe after unmount or dismissal', async () => {
  const fixture = await mount({ delayedBlur: true }); const { root, window } = fixture;
  try {
    const trigger = button(root, '和星芽互动');
    const opening = trigger.props.onClick(); window.emit('blur'); await opening; await flush();
    assert.equal(opened(root), false); assert.equal(trigger.focusCount, 0);
    const pendingOpen = trigger.props.onClick(); fixture.unmount(); await pendingOpen;
    fixture.finishBlur(); await flush(); assert.equal(fixture.stopBlur(), 1); assert.equal(trigger.focusCount, 0);
    assert.equal(fixture.document.count(), 0); assert.equal(window.count(), 0);
  } finally { fixture.cleanup(); }
});

test('native blur API subscribes only to the starpet window and browser preview installs no native listener', async () => {
  const savedWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const savedMock = Object.getOwnPropertyDescriptor(globalThis, 'petBlurApiMock');
  const registrations: { event: string; callback: () => void; options: unknown }[] = [];
  let stopped = 0, blurred = 0;
  const unsubscribe = () => { stopped++; };
  Object.assign(globalThis, { petBlurApiMock: { async listen(event: string, callback: () => void, options: unknown) { registrations.push({ event, callback, options }); return unsubscribe; } } });
  const eventUrl = 'data:text/javascript;base64,' + Buffer.from('export const listen=(...args)=>globalThis.petBlurApiMock.listen(...args);').toString('base64');
  const coreUrl = 'data:text/javascript,export function invoke(){throw new Error("unexpected native command")}';
  const source = fs.readFileSync(new URL('../src/starpet-api.ts', import.meta.url), 'utf8')
    .replaceAll("'@tauri-apps/api/core'", `'${coreUrl}'`).replaceAll("'@tauri-apps/api/event'", `'${eventUrl}'`);
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  try {
    Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {} } });
    const native = await import('data:text/javascript;base64,' + Buffer.from(code + '\n// native blur').toString('base64'));
    const stop = await native.onStarPetWindowBlur(() => { blurred++; });
    assert.equal(registrations.length, 1); assert.equal(registrations[0]!.event, 'tauri://blur');
    assert.deepEqual(registrations[0]!.options, { target: { kind: 'Window', label: 'starpet' } });
    registrations[0]!.callback(); assert.equal(blurred, 1); assert.equal(stop, unsubscribe); stop(); assert.equal(stopped, 1);
    Object.assign(globalThis, { window: {} });
    const browser = await import('data:text/javascript;base64,' + Buffer.from(code + '\n// browser blur').toString('base64'));
    (await browser.onStarPetWindowBlur(() => { blurred++; }))();
    assert.equal(registrations.length, 1); assert.equal(blurred, 1); assert.equal(stopped, 1);
  } finally {
    if (savedWindow) Object.defineProperty(globalThis, 'window', savedWindow); else Reflect.deleteProperty(globalThis, 'window');
    if (savedMock) Object.defineProperty(globalThis, 'petBlurApiMock', savedMock); else Reflect.deleteProperty(globalThis, 'petBlurApiMock');
  }
});


test('Agent signal wins over delayed snapshot, renders plain text and expires without growth writes', async context => {
  context.mock.timers.enable({ apis: ['setTimeout'] });
  const fixture = await mount({ delayedSignal: true });
  try {
    const pet = () => nodes(fixture.root).find(item => item.type === 'pet-fixture')!;
    const signal = { source: '原创测试 Agent', status: 'working' as const, action: 'greet' as const, message: '<b>这是原创测试消息</b>', expiresAt: Date.now() + 5000 };
    fixture.signal({ revision: 2, signal }); await flush();
    assert.equal(pet().props.mood, 'writing'); assert.equal(pet().props.actionNonce, 1);
    fixture.finishSignal({ revision: 1, signal: null }); await flush();
    assert.equal(pet().props.mood, 'writing', 'delayed initial snapshot cannot erase a newer event');
    fixture.signal({ revision: 2, signal }); await flush(); assert.equal(pet().props.actionNonce, 1, 'duplicate revision cannot replay action');
    assert.match(text(fixture.root), /原创测试 Agent · 工作中/); assert.match(text(fixture.root), /<b>这是原创测试消息<\/b>/);
    assert.equal(nodes(fixture.root).some(item => item.type === 'b' || 'innerHTML' in item.props), false);
    assert.deepEqual(fixture.calls, [], 'external actions never invoke growth or preferences');
    context.mock.timers.tick(5000); await flush(); assert.equal(pet().props.mood, 'idle'); assert.doesNotMatch(text(fixture.root), /原创测试 Agent/);
    fixture.signal({ revision: 3, signal: { ...signal, expiresAt: Date.now() - 1 } }); await flush(); assert.equal(pet().props.actionNonce, 1, 'expired event never animates');
    fixture.signal({ revision: 4, signal }); await flush(); fixture.signal({ revision: 5, signal: null }); await flush();
    assert.equal(pet().props.mood, 'idle'); assert.doesNotMatch(text(fixture.root), /原创测试 Agent/);
    fixture.unmount(); assert.equal(fixture.stopSignal(), 1);
  } finally { fixture.cleanup(); context.mock.timers.reset(); }
});

test('Agent initial snapshot never replays action and manual dismissal or hidden window clears the temporary message', async () => {
  const signal = { source: '原创恢复测试', status: 'thinking' as const, action: 'greet' as const, message: '只恢复短句', expiresAt: Date.now() + 5000 };
  const fixture = await mount({ initialSignal: { revision: 1, signal } });
  try {
    const pet = () => nodes(fixture.root).find(item => item.type === 'pet-fixture')!;
    assert.equal(pet().props.mood, 'thinking'); assert.equal(pet().props.actionNonce, 0);
    await button(fixture.root, '收起 Agent 提示').props.onClick(); await flush(); assert.equal(pet().props.mood, 'idle');
    fixture.signal({ revision: 2, signal }); await flush();
    fixture.document.hidden = true; fixture.document.emit('visibilitychange'); await flush();
    assert.equal(pet().props.mood, 'idle'); assert.doesNotMatch(text(fixture.root), /原创恢复测试/);
    fixture.document.hidden = false; fixture.document.emit('visibilitychange'); await flush(); assert.equal(pet().props.mood, 'idle');
  } finally { fixture.cleanup(); }
  const late = await mount({ delayedSignalListener: true });
  try { late.unmount(); late.finishSignalListener(); await flush(); assert.equal(late.stopSignal(), 1); }
  finally { late.cleanup(); }
});

test('settings interaction events replay identical actions once each without extra growth writes or preference changes', async () => {
  const fixture = await mount();
  try {
    const pet = () => nodes(fixture.root).find(item => item.type === 'pet-fixture')!;
    for (const action of ['greet', 'highfive', 'stretch', 'stretch'] as const) {
      const previous = pet().props.actionNonce;
      fixture.interaction({ action, animate: true, signalRevision: 0 }); await flush();
      assert.equal(pet().props.action, action); assert.equal(pet().props.actionNonce, previous + 1);
      assert.equal(pet().props.mood, 'idle');
    }
    assert.deepEqual(fixture.calls, [], 'receiving settings actions never records XP or saves preferences');
    await fixture.open(); const previous = pet().props.actionNonce;
    await button(fixture.root, '问个好').props.onClick(); await flush();
    assert.deepEqual(fixture.calls, ['interact']); assert.equal(pet().props.actionNonce, previous + 1, 'native event plus command response plays once');
    fixture.interaction({ action: 'greet', animate: false, signalRevision: 1 }); await flush();
    assert.equal(pet().props.actionNonce, previous + 1, 'body touches preserve their existing reaction instead of playing a second action');
    fixture.document.hidden = true; fixture.document.emit('visibilitychange');
    fixture.interaction({ action: 'stretch', animate: true, signalRevision: 1 }); await flush();
    assert.equal(pet().props.actionNonce, previous + 1, 'hidden windows do not replay or show themselves');
    fixture.unmount(); fixture.interaction({ action: 'stretch', animate: true, signalRevision: 2 }); await flush();
    assert.equal(fixture.stopInteraction(), 1);
  } finally { fixture.cleanup(); }
  const late = await mount({ delayedInteractionListener: true });
  try { late.unmount(); late.finishInteractionListener(); await flush(); assert.equal(late.stopInteraction(), 1); }
  finally { late.cleanup(); }
});

test('a local interaction clears the previous Agent signal while a newer Agent revision still wins', async () => {
  const fixture = await mount({ delayedSignal: true });
  try {
    const pet = () => nodes(fixture.root).find(item => item.type === 'pet-fixture')!;
    const signal = { source: '原创同步测试', status: 'working' as const, action: 'greet' as const, message: '临时 Agent 状态', expiresAt: Date.now() + 5000 };
    fixture.signal({ revision: 1, signal }); await flush(); assert.equal(pet().props.mood, 'writing');
    fixture.interaction({ action: 'highfive', animate: true, signalRevision: 2 }); await flush();
    assert.equal(pet().props.mood, 'idle'); assert.equal(pet().props.action, 'highfive'); assert.equal(pet().props.actionNonce, 2);
    assert.doesNotMatch(text(fixture.root), /临时 Agent 状态/);
    fixture.finishSignal({ revision: 1, signal }); await flush(); assert.equal(pet().props.mood, 'idle', 'late startup snapshot cannot restore the cleared signal');
    fixture.signal({ revision: 3, signal }); await flush(); assert.equal(pet().props.mood, 'writing');
    fixture.interaction({ action: 'stretch', animate: true, signalRevision: 2 }); await flush();
    assert.equal(pet().props.mood, 'writing'); assert.equal(pet().props.action, 'greet', 'late local event cannot overwrite a newer Agent signal');
    assert.deepEqual(fixture.calls, []);
  } finally { fixture.cleanup(); }
});


test('routine mood cycling stays positive or neutral while saved choices and explicit Agent errors remain available', async () => {
  const fixture = await mount({ initialMood: 'sad' });
  try {
    const pet = () => nodes(fixture.root).find(item => item.type === 'pet-fixture')!;
    assert.equal(pet().props.mood, 'sad', 'keep an explicitly saved mood');
    const cycle = new Set<string>();
    for (let step = 0; step < 52; step++) {
      await button(fixture.root, '换个心情').props.onClick(); await flush();
      const mood = pet().props.mood;
      assert.ok(!['puzzled', 'sad', 'crying', 'angry', 'nervous', 'error'].includes(mood), mood);
      cycle.add(mood);
    }
    assert.equal(cycle.size, 26, 'two complete cycles still offer all everyday expressions');
    const savedMood = pet().props.mood;
    fixture.signal({ revision: 1, signal: { source: '原创状态测试', status: 'error', message: '明确请求错误状态', expiresAt: Date.now() + 5000 } }); await flush();
    assert.equal(pet().props.mood, 'error', 'an explicitly supplied Agent status is preserved');
    await button(fixture.root, '收起 Agent 提示').props.onClick(); await flush();
    assert.equal(pet().props.mood, savedMood, 'dismissing explicit status returns to the chosen everyday mood');
  } finally { fixture.cleanup(); }
});
