import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { compileScript, parse } from '@vue/compiler-sfc';
import { createRenderer, nextTick } from 'vue';

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
let sequence = 0;
async function mount(mock: Record<string, unknown>, props: Record<string, unknown> = {}) {
  Object.assign(globalThis, { productSharePanelMock: mock });
  const api = 'const api=globalThis.productSharePanelMock; export const { copyProductShareImage, copyProductShareText, saveProductShareImage, shareImages, shareImageSize, shareImageSource, shareTexts }=api;';
  const apiUrl = 'data:text/javascript;base64,' + Buffer.from(api + `\n// ${sequence++}`).toString('base64');
  const { descriptor } = parse(fs.readFileSync(new URL('../src/components/ProductShare.vue', import.meta.url), 'utf8'));
  const script = compileScript(descriptor, { id: 'product-share-test', inlineTemplate: true });
  const code = ts.transpileModule(script.content, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText
    .replace(/from (['"])vue\1/g, `from '${import.meta.resolve('vue')}'`)
    .replaceAll("'../product-share'", `'${apiUrl}'`)
    .replaceAll("'./Icon.vue'", "'data:text/javascript,export default {render(){return null}}'");
  const component = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  const root = node('root'); const app = renderer.createApp(component.default, props); app.mount(root); await flush();
  return { root, app };
}
function mockApi() {
  const calls: string[] = [];
  return {
    calls, shareTexts: [{ id: 'short', label: '简短介绍', content: '原创文案 <script>不会执行</script>' }, { id: 'detailed', label: '详细介绍', content: '原创详细文案' }],
    shareImages: ['overview', 'workflow', 'companion'].map((id, i) => ({ id, label: ['产品总览', '灵感工作流', '星伴陪伴'][i], description: '原创产品示例', path: `share/${id}.png`, filename: `${id}.png`, width: 1080, height: 1440, bytes: 102400 })),
    shareImageSize: () => '100 KB', shareImageSource: (image: { path: string }) => `/${image.path}`,
    async copyProductShareText(id: string) { calls.push(`text:${id}`); }, async copyProductShareImage(id: string) { calls.push(`image:${id}`); },
    async saveProductShareImage(id: string): Promise<{ kind: string; path?: string; filename?: string }> { calls.push(`save:${id}`); return { kind: 'cancelled' }; },
  };
}

test('sharing panel previews fixed materials, locks duplicate actions and reports only completed copy', async () => {
  const mock = mockApi(); const busy: boolean[] = []; let finish!: () => void;
  mock.copyProductShareImage = id => { mock.calls.push(`image:${id}`); return new Promise<void>(resolve => { finish = resolve; }); };
  const { root, app } = await mount(mock, { onBusy: (value: boolean) => busy.push(value) });
  assert.deepEqual(mock.calls, []);
  await button(root, '分享星笺文案与产品图片，准备好就能分享').props.onClick(); await flush();
  assert.match(text(root), /<script>不会执行<\/script>/); assert.equal(nodes(root).some(item => item.type === 'script'), false);
  await button(root, '详细介绍').props.onClick(); await flush(); await button(root, '复制文案').props.onClick(); await flush();
  assert.deepEqual(mock.calls, ['text:detailed']); assert.match(text(root), /文案已复制/);
  const copiedNotice = nodes(root).find(item => item.props.role === 'status' && text(item).includes('文案已复制'))!;
  const textCopyButton = button(root, '复制文案');
  assert.equal(textCopyButton.parent, copiedNotice.parent);
  assert.equal(copiedNotice.parent!.children.indexOf(copiedNotice), textCopyButton.parent!.children.indexOf(textCopyButton) + 1,
    'text copy receipt must stay beside its button, above the tall image preview');
  await button(root, '星伴陪伴').props.onClick(); await flush();
  const copying = button(root, '复制图片').props.onClick(); await flush();
  assert.equal(busy.at(-1), true); assert.equal(button(root, '保存图片').props.disabled, true);
  await button(root, '正在复制…').props.onClick();
  assert.deepEqual(mock.calls, ['text:detailed', 'image:companion']);
  assert.doesNotMatch(text(root), /「星伴陪伴」已复制/);
  finish(); await copying; await flush(); assert.equal(busy.at(-1), false); assert.match(text(root), /「星伴陪伴」已复制/);
  app.unmount();
});

test('sharing panel distinguishes cancelled save, actual file path, browser download and copy failure', async () => {
  const mock = mockApi(); const { root, app } = await mount(mock);
  await button(root, '分享星笺文案与产品图片，准备好就能分享').props.onClick(); await flush();
  await button(root, '保存图片').props.onClick(); await flush(); assert.match(text(root), /已取消保存，没有写入文件/);
  mock.saveProductShareImage = async () => ({ kind: 'saved', path: '/synthetic-output/产品图.png' });
  // Exported API functions are bound when loaded; vary the same function through a new mount.
  app.unmount(); const saved = await mount(mock);
  await button(saved.root, '分享星笺文案与产品图片，准备好就能分享').props.onClick(); await flush();
  await button(saved.root, '保存图片').props.onClick(); await flush(); assert.match(text(saved.root), /图片已保存：\/synthetic-output\/产品图.png/); saved.app.unmount();
  mock.saveProductShareImage = async () => ({ kind: 'downloaded', filename: 'overview.png' });
  mock.copyProductShareText = async () => { throw new Error('测试：剪贴板权限被拒绝'); };
  const download = await mount(mock);
  await button(download.root, '分享星笺文案与产品图片，准备好就能分享').props.onClick(); await flush();
  await button(download.root, '保存图片').props.onClick(); await flush(); assert.match(text(download.root), /已发起下载：overview.png/);
  await button(download.root, '复制文案').props.onClick(); await flush();
  assert.match(text(download.root), /剪贴板权限被拒绝/); assert.doesNotMatch(text(download.root), /文案已复制/);
  assert.equal(button(download.root, '复制文案').props.disabled, false); download.app.unmount();
});
