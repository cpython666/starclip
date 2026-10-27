import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';

const catalog = {
  texts: [{ id: 'short', label: '简短介绍', content: '原创固定分享文案' }],
  images: [{ id: 'overview', label: '产品总览', path: 'share/overview.png', filename: 'starclip-overview.png', width: 1080, height: 1440, bytes: 12345, description: '原创产品图片' }],
};
let sequence = 0;
async function load(desktop: boolean) {
  Object.assign(globalThis, { window: desktop ? { __TAURI_INTERNALS__: {} } : {} });
  const source = fs.readFileSync(new URL('../src/product-share.ts', import.meta.url), 'utf8')
    .replace(/import catalog[^\n]+/, `const catalog = ${JSON.stringify(catalog)};`)
    .replaceAll("import('@tauri-apps/api/core')", 'Promise.resolve(globalThis.productShareTestCore)');
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  return import('data:text/javascript;base64,' + Buffer.from(code + `\n// ${sequence++}`).toString('base64'));
}
function navigatorWith(clipboard: Record<string, unknown>) {
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { clipboard: {
    read() { throw new Error('A share action must not read the clipboard'); },
    readText() { throw new Error('A share action must not read the clipboard'); },
    ...clipboard,
  } } });
}

test('desktop share sends only fixed catalog ids, preserves native cancellation and actual save path', async () => {
  const calls: unknown[] = []; let savePath: string | null = null;
  Object.assign(globalThis, { productShareTestCore: { async invoke(command: string, args: unknown) { calls.push([command, args]); return command === 'save_product_share_image' ? savePath : undefined; } } });
  navigatorWith({});
  const api = await load(true);
  await api.copyProductShareText('short'); await api.copyProductShareImage('overview');
  assert.deepEqual(await api.saveProductShareImage('overview'), { kind: 'cancelled' });
  savePath = '/synthetic-output/产品图.png';
  assert.deepEqual(await api.saveProductShareImage('overview'), { kind: 'saved', path: savePath });
  assert.deepEqual(calls, [
    ['copy_product_share_text', { id: 'short' }], ['copy_product_share_image', { id: 'overview' }],
    ['save_product_share_image', { id: 'overview' }], ['save_product_share_image', { id: 'overview' }],
  ]);
  for (const id of ['../../private.png', 'https://example.com/image.png', 'missing']) {
    await assert.rejects(api.copyProductShareImage(id), /暂不可用/);
    await assert.rejects(api.saveProductShareImage(id), /暂不可用/);
    await assert.rejects(api.copyProductShareText(id), /暂不可用/);
  }
  assert.equal(calls.length, 4);
});

test('browser image copy starts write during the click and awaits promised PNG, never reads clipboard', async () => {
  const events: string[] = []; let resolveFetch!: (response: Response) => void;
  Object.assign(globalThis, {
    fetch(url: string) { assert.equal(url, '/share/overview.png'); events.push('fetch'); return new Promise<Response>(resolve => { resolveFetch = resolve; }); },
    ClipboardItem: class { data: Record<string, Promise<Blob>>; constructor(data: Record<string, Promise<Blob>>) { this.data = data; events.push('item'); } },
  });
  navigatorWith({
    async write(items: Array<{ data: Record<string, Promise<Blob>> }>) {
      events.push('write'); const blob = await items[0].data['image/png'];
      assert.equal(blob.type, 'image/png'); events.push('copied');
    },
    async writeText(text: string) { assert.equal(text, catalog.texts[0].content); events.push('text'); },
  });
  const api = await load(false);
  const copying = api.copyProductShareImage('overview');
  assert.deepEqual(events, ['fetch', 'item', 'write'], 'do not await fetch before invoking clipboard.write');
  resolveFetch(new Response(new Blob(['fixed PNG stand-in'], { type: 'image/png' }))); await copying;
  await api.copyProductShareText('short');
  assert.deepEqual(events, ['fetch', 'item', 'write', 'copied', 'text']);
});

test('browser rejects denied or unsupported copy and downloads only a valid built-in PNG', async () => {
  const api = await load(false);
  navigatorWith({});
  await assert.rejects(api.copyProductShareText('short'), /手动复制/);
  await assert.rejects(api.copyProductShareImage('overview'), /保存图片/);
  navigatorWith({ writeText: async () => { throw new Error('permission denied'); }, write: async () => { throw new Error('permission denied'); } });
  Object.assign(globalThis, { fetch: async () => new Response(new Blob(['fixed'], { type: 'image/png' })) });
  await assert.rejects(api.copyProductShareText('short'), /未能复制/);
  await assert.rejects(api.copyProductShareImage('overview'), /未能复制/);
  let clicked = 0;
  const link = { href: '', download: '', click() { clicked++; } };
  Object.assign(globalThis, { document: { createElement(tag: string) { assert.equal(tag, 'a'); return link; } } });
  assert.deepEqual(await api.saveProductShareImage('overview'), { kind: 'downloaded', filename: 'starclip-overview.png' });
  assert.equal(clicked, 1); assert.equal(link.download, 'starclip-overview.png'); assert.match(link.href, /^blob:/);
  Object.assign(globalThis, { fetch: async () => new Response('not PNG', { headers: { 'Content-Type': 'text/plain' } }) });
  await assert.rejects(api.saveProductShareImage('overview'), /无效/);
  Object.assign(globalThis, { fetch: async () => new Response('', { status: 404 }) });
  await assert.rejects(api.saveProductShareImage('overview'), /读取失败/);
  assert.equal(clicked, 1);
});
