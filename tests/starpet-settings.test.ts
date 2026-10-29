import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { compileScript, parse } from '@vue/compiler-sfc';
import { createRenderer, nextTick } from 'vue';

type Node = { type: string; text: string; props: Record<string, any>; children: Node[]; parent?: Node };
const node = (type: string, text = ''): Node => ({ type, text, props: {}, children: [] });
const renderer = createRenderer<Node, Node>({
  createElement: type => node(type), createText: text => node('#text', text), createComment: text => node('#comment', text),
  setText: (node, text) => { node.text = text; }, setElementText: (node, text) => { node.text = text; node.children = []; },
  patchProp: (node, key, _previous, value) => { node.props[key] = value; },
  insert: (child, parent, anchor) => { const previous = child.parent?.children.indexOf(child) ?? -1; if (previous >= 0) child.parent!.children.splice(previous, 1); child.parent = parent; const index = anchor ? parent.children.indexOf(anchor) : -1; parent.children.splice(index >= 0 ? index : parent.children.length, 0, child); },
  remove: child => { const index = child.parent?.children.indexOf(child) ?? -1; if (index >= 0) child.parent!.children.splice(index, 1); },
  parentNode: node => node.parent || null, nextSibling: node => node.parent?.children[(node.parent?.children.indexOf(node) ?? -1) + 1] || null,
});
const nodes = (root: Node): Node[] => [root, ...root.children.flatMap(nodes)];
const text = (root: Node): string => root.type === '#comment' ? '' : root.text + root.children.map(text).join('');
function button(root: Node, label: string) { const found = nodes(root).find(item => item.type === 'button' && (item.props['aria-label'] === label || text(item) === label)); assert(found, `missing ${label}`); return found; }
async function flush() { await new Promise(resolve => setImmediate(resolve)); await nextTick(); }
function moduleUrl(source: string) { return 'data:text/javascript;base64,' + Buffer.from(ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText).toString('base64'); }

test('gallery starts with everyday moods, other expressions require a choice and automatic tours remain positive', async context => {
  context.mock.timers.enable({ apis: ['setInterval'] });
  const savedGlobal = Object.getOwnPropertyDescriptor(globalThis, 'starPetSettingsMock');
  let preferences = { enabled: true, mood: 'sad', color: '#FFD66E', size: 100 };
  const writes: object[] = [];
  Object.assign(globalThis, { starPetSettingsMock: {
    starPetDesktopAvailable: true,
    async getStarPetPreferences() { return preferences; }, async onStarPetChanged() { return () => {}; },
    async setStarPetPreferences(patch: object) { writes.push(patch); return preferences = { ...preferences, ...patch }; },
  } });
  const api = moduleUrl('export const { starPetDesktopAvailable, getStarPetPreferences, onStarPetChanged, setStarPetPreferences } = globalThis.starPetSettingsMock;');
  const empty = moduleUrl('export default {render(){return null}}');
  const pet = moduleUrl(`import { h } from '${import.meta.resolve('vue')}'; export default {props:['mood'],render(){return h('pet-fixture',{...this.$props})}}`);
  const select = moduleUrl(`import { h } from '${import.meta.resolve('vue')}'; export default {props:['modelValue','options'],emits:['update:modelValue'],render(){return h('select-fixture',{...this.$props,change:value=>this.$emit('update:modelValue',value)})}}`);
  const { descriptor } = parse(fs.readFileSync(new URL('../src/components/StarPetSettings.vue', import.meta.url), 'utf8'));
  let source = compileScript(descriptor, { id: 'starpet-settings-test', inlineTemplate: true }).content
    .replace(/from (['"])vue\1/g, `from '${import.meta.resolve('vue')}'`)
    .replaceAll("'../starpet-api'", `'${api}'`).replaceAll("'./StarPet.vue'", `'${pet}'`).replaceAll("'./UiSelect.vue'", `'${select}'`);
  for (const path of ['starpet', 'starpet-growth', 'starpet-size', 'i18n/index']) source = source.replaceAll(`'../${path === 'i18n/index' ? 'i18n' : path}'`, `'${new URL(`../src/${path}.ts`, import.meta.url).href}'`);
  for (const path of ['Icon', 'UiColor', 'StarPetGrowth']) source = source.replaceAll(`'./${path}.vue'`, `'${empty}'`);
  const component = await import(moduleUrl(source));
  const root = node('root'), app = renderer.createApp(component.default);
  try {
    app.mount(root); await flush();
    const previews = () => nodes(root).filter(item => item.type === 'button' && item.props['aria-label']?.startsWith('预览'));
    const stage = () => nodes(root).find(item => item.type === 'pet-fixture')!.props.mood;
    const other = () => nodes(root).find(item => item.type === 'input' && item.props.type === 'checkbox')!;
    assert.equal(previews().length, 26); assert.equal(stage(), 'sad', 'explicit saved mood is not reset on opening settings');
    assert.equal(previews().some(item => item.props['aria-label'] === '预览眼泪汪汪'), false);
    assert.deepEqual(writes, []);
    other().props.onChange({ target: { checked: true } }); await flush();
    assert.equal(previews().length, 32);
    await button(root, '预览眼泪汪汪').props.onClick(); await flush(); assert.equal(stage(), 'crying');
    await button(root, '用这个表情陪我 ↗').props.onClick(); await flush();
    assert.equal(preferences.mood, 'crying', 'manual negative choice is still saved explicitly');
    other().props.onChange({ target: { checked: false } }); await flush();
    assert.equal(previews().length, 26); assert.equal(stage(), 'crying', 'hiding the optional catalog does not change a chosen mood');
    other().props.onChange({ target: { checked: true } }); await flush();
    await button(root, '表情巡游').props.onClick(); await flush();
    const encountered = new Set<string>();
    for (let step = 0; step < 52; step++) {
      assert.ok(!['puzzled', 'sad', 'crying', 'angry', 'nervous', 'error'].includes(stage()), stage());
      encountered.add(stage()); context.mock.timers.tick(2600); await flush();
    }
    assert.equal(encountered.size, 26, 'tour excludes other moods even when optional tiles are shown');
    const category = nodes(root).find(item => item.type === 'select-fixture')!;
    category.props.change('emotion'); await flush();
    assert.equal(previews().length, 12, 'all optional moods remain selectable in their category');
    for (let step = 0; step < 14; step++) {
      assert.ok(['happy', 'delighted', 'love', 'shy', 'surprised', 'proud', 'relieved'].includes(stage()), stage());
      context.mock.timers.tick(2600); await flush();
    }
    assert.equal(writes.length, 1, 'tour never persists its temporary preview');
  } finally {
    app.unmount(); context.mock.timers.reset();
    if (savedGlobal) Object.defineProperty(globalThis, 'starPetSettingsMock', savedGlobal); else Reflect.deleteProperty(globalThis, 'starPetSettingsMock');
  }
});
