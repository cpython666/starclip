import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { compileScript, parse } from '@vue/compiler-sfc';
import { createRenderer, h, nextTick, ref } from 'vue';
import { defaultAppearance } from '../src/appearance.ts';
import type { Appearance } from '../src/appearance.ts';

type Node = { type: string; text: string; props: Record<string, any>; children: Node[]; parent?: Node };
function node(type: string, text = ''): Node { return { type, text, props: {}, children: [] }; }
const renderer = createRenderer<Node, Node>({
  createElement: type => node(type), createText: text => node('#text', text), createComment: text => node('#comment', text),
  setText: (node, text) => { node.text = text; }, setElementText: (node, text) => { node.text = text; node.children = []; },
  patchProp: (node, key, _previous, value) => { node.props[key] = value; },
  insert: (child, parent, anchor) => { const previous = child.parent?.children.indexOf(child) ?? -1; if (previous >= 0) child.parent!.children.splice(previous, 1); child.parent = parent; const index = anchor ? parent.children.indexOf(anchor) : -1; parent.children.splice(index >= 0 ? index : parent.children.length, 0, child); },
  remove: child => { const index = child.parent?.children.indexOf(child) ?? -1; if (index >= 0) child.parent!.children.splice(index, 1); },
  parentNode: node => node.parent || null, nextSibling: node => node.parent?.children[(node.parent?.children.indexOf(node) ?? -1) + 1] || null,
});
function nodes(root: Node): Node[] { return [root, ...root.children.flatMap(nodes)]; }
function text(root: Node): string { return root.type === '#comment' ? '' : root.text + root.children.map(text).join(''); }
function button(root: Node, label: string) { const found = nodes(root).find(item => item.type === 'button' && text(item) === label); assert(found, `missing ${label}`); return found; }
async function flush() { await new Promise(resolve => setImmediate(resolve)); await nextTick(); }
function moduleUrl(source: string) { return 'data:text/javascript;base64,' + Buffer.from(ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText).toString('base64'); }

test('appearance panel saves only confirmed edits, blocks duplicates, preserves state on failure and ignores late unmounted results', async () => {
  const current = ref<Appearance>({ ...defaultAppearance(), stickerEnabled: true, stickerDataUrl: 'data:image/png;base64,fixture', stickerName: '固定样例.png' }); const changes: Appearance[] = [], busy: boolean[] = [], patches: unknown[] = [];
  let settle!: (value: Appearance) => void, reject!: (reason: Error) => void;
  Object.assign(globalThis, { appearancePanelApi: {
    appearanceDesktopAvailable: true,
    saveAppearance(patch: unknown) { patches.push(patch); return new Promise<Appearance>((resolve, failure) => { settle = resolve; reject = failure; }); },
    importAppearanceSticker() { throw new Error('not called'); }, removeAppearanceSticker() { throw new Error('not called'); },
  } });
  const apiUrl = moduleUrl('export const { appearanceDesktopAvailable, saveAppearance, importAppearanceSticker, removeAppearanceSticker }=globalThis.appearancePanelApi;');
  const helperUrl = moduleUrl(fs.readFileSync(new URL('../src/appearance.ts', import.meta.url), 'utf8'));
  const { descriptor } = parse(fs.readFileSync(new URL('../src/components/AppearanceSettings.vue', import.meta.url), 'utf8'));
  const script = compileScript(descriptor, { id: 'appearance-panel-test', inlineTemplate: true });
  const source = script.content.replace(/from (['"])vue\1/g, `from '${import.meta.resolve('vue')}'`)
    .replaceAll("'../appearance-api'", `'${apiUrl}'`).replaceAll("'../appearance'", `'${helperUrl}'`)
    .replaceAll("'./UiColor.vue'", "'data:text/javascript,export default {render(){return null}}'")
    .replaceAll("'./UiSelect.vue'", "'data:text/javascript,export default {render(){return null}}'");
  const component = await import(moduleUrl(source));
  const root = node('root');
  const app = renderer.createApp({ setup: () => () => h(component.default, { value: current.value, onChange(value: Appearance) { changes.push(value); current.value = value; }, onBusy(value: boolean) { busy.push(value); } }) });
  app.mount(root); await flush(); assert.equal(patches.length, 0, 'mounting does not save or read data');
  const checks = nodes(root).filter(item => item.type === 'input' && item.props.type === 'checkbox');
  for (const checkbox of checks) {
    const nativeInput = { checked: false };
    checkbox.props.onChange({ target: nativeInput });
    assert.equal(nativeInput.checked, true, 'restore native toggled checked before the independent save');
    reject(new Error('测试：开关保存失败')); await flush();
    assert.equal(current.value.stickerEnabled, true); assert.equal(current.value.accent, null); assert.equal(changes.length, 0);
  }
  assert.equal(patches.length, 2); patches.splice(0);
  const saving = button(root, '柔粉奶油').props.onClick(); await flush();
  assert.equal(busy.at(-1), true); assert.equal(button(root, '替换 PNG 贴纸').props.disabled, true);
  await button(root, '柔粉奶油').props.onClick(); assert.equal(patches.length, 1);
  assert.equal(changes.length, 0); assert.equal(current.value.background, 'theme');
  reject(new Error('测试：磁盘不可写')); await saving; await flush();
  assert.equal(current.value.background, 'theme'); assert.match(text(root), /原外观保留/); assert.match(text(root), /磁盘不可写/); assert.equal(busy.at(-1), false);
  const retried = button(root, '柔粉奶油').props.onClick();
  settle({ ...defaultAppearance(), background: 'gradient' }); await retried; await flush();
  assert.equal(current.value.background, 'gradient'); assert.equal(changes.length, 1); assert.match(text(root), /外观已保存并生效/);
  const pending = button(root, '恢复主题配色').props.onClick(); await flush();
  assert.deepEqual(patches.at(-1), { accent: null, background: 'theme' });
  app.unmount(); settle(defaultAppearance()); await pending; await flush();
  assert.equal(changes.length, 1, 'a response after unmount cannot change parent appearance'); assert.equal(busy.at(-1), false);
});
