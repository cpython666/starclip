import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';

const root = new URL('../public/handwriting-fonts/', import.meta.url);
const catalog = JSON.parse(readFileSync(new URL('catalog.json', root), 'utf8'));

test('every offered font ships a small pixel-only preview and its source-specific usage notice, without font binaries', () => {
  assert.ok(catalog.length > 2, 'the gallery must include the expanded font catalog');
  assert.equal(new Set(catalog.map(item => item.id)).size, catalog.length);
  let previewBytes = 0;
  for (const item of catalog) {
    assert.ok(item.author.trim(), `missing author: ${item.id}`);
    assert.ok(item.tags.length > 0 && item.tags.every(tag => ['手写体', '趣味体', '印刷体', '等宽'].includes(tag)));
    assert.ok(Number.isInteger(item.bytes) && item.bytes > 0 && item.bytes <= 16 * 1024 * 1024);
    assert.match(item.sha256, /^[a-f0-9]{64}$/);
    const download = new URL(item.url);
    assert.equal(download.origin, 'https://raw.githubusercontent.com');
    assert.equal(decodeURIComponent(download.pathname.split('/').at(-1)!), item.file);
    assert.match(download.pathname, /^\/(google\/fonts|cpython666\/HandwritingGenerator)\/[a-f0-9]{40}\//);
    assert.equal(item.previewUrl, `/handwriting-fonts/${item.id}.png`);
    const image = readFileSync(new URL(`${item.id}.png`, root));
    assert.equal(image.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
    assert.equal(image.readUInt32BE(16), 600);
    assert.equal(image.readUInt32BE(20), 200);
    assert.ok(image.length < 40 * 1024, `preview too large: ${item.id}`);
    let offset = 8;
    let lastChunk = '';
    while (offset < image.length) {
      assert.ok(offset + 12 <= image.length, `truncated PNG chunk: ${item.id}`);
      const length = image.readUInt32BE(offset);
      const type = image.subarray(offset + 4, offset + 8).toString('ascii');
      assert.ok(['IHDR', 'PLTE', 'IDAT', 'IEND'].includes(type), `unexpected embedded PNG data: ${item.id}/${type}`);
      assert.notEqual(lastChunk, 'IEND', `data after PNG end: ${item.id}`);
      offset += length + 12;
      lastChunk = type;
    }
    assert.equal(offset, image.length);
    assert.equal(lastChunk, 'IEND');
    previewBytes += image.length;
    if (download.pathname.startsWith('/google/fonts/')) {
      assert.equal(item.license, 'SIL OFL 1.1');
      assert.equal(item.licenseUrl, `/handwriting-fonts/${item.id}-OFL.txt`);
      const license = readFileSync(new URL(`${item.id}-OFL.txt`, root), 'utf8');
      assert.match(license, /SIL OPEN FONT LICENSE Version 1\.1/);
      assert.match(license, /PERMISSION & CONDITIONS/);
      assert.ok(license.split('\n')[0].includes(item.author));
      assert.match(download.pathname, /\/ofl\/[^/]+\/[^/]+\.ttf$/);
      assert.equal(item.sourceUrl, item.url.replace('raw.githubusercontent.com/google/fonts/', 'github.com/google/fonts/tree/').slice(0, -item.file.length - 1));
    } else {
      assert.equal(item.license, '作者授权（Starclip 按需提供）');
      assert.equal(item.licenseUrl, '/handwriting-fonts/ORIGINAL-FONTS-AUTHORIZATION.txt');
      const notice = readFileSync(new URL('ORIGINAL-FONTS-AUTHORIZATION.txt', root), 'utf8');
      assert.match(notice, /授权星笺（Starclip）.*按需提供/);
      assert.match(notice, /来源于互联网/);
      assert.ok(notice.includes(item.file));
      assert.doesNotMatch(notice, /SIL OPEN FONT LICENSE|MIT License/);
      assert.equal(item.sourceUrl, item.url.replace('raw.githubusercontent.com/cpython666/HandwritingGenerator/', 'github.com/cpython666/HandwritingGenerator/blob/'));
    }
  }
  assert.ok(previewBytes < 400 * 1024, 'the complete preview catalog should stay lightweight');
  assert.equal(readdirSync(root, { recursive: true }).some(name => /\.(ttf|otf|woff2?|ttc)$/i.test(String(name))), false);
});

test('all nine original fonts retain their filenames, confirmed source display and handwriting tag', () => {
  const originals = catalog.filter(item => item.url.includes('/cpython666/HandwritingGenerator/'));
  assert.deepEqual(new Set(originals.map(item => item.file)), new Set([
    'hand.ttf', '上首鸿志手写体.ttf', '品如手写体.ttf', '张维镜手写楷书.ttf', '手写大象体.ttf',
    '未知手写体_1.ttf', '李国夫董事长手写体.ttf', '薛文轩钢笔楷体.ttf', '青叶手写体.ttf',
  ]));
  for (const item of originals) {
    assert.equal(item.author, '来源于互联网');
    assert.deepEqual(item.tags, ['手写体']);
    assert.ok(item.url.includes('/00160ae98e9512529f0180a8036fbf4c3ecd3bc7/fonts/'));
  }
  for (const item of catalog.filter(item => /^(ma-shan-zheng|zhi-mang-xing|long-cang|liu-jian-mao-cao|lxgwwenkai)/.test(item.id))) {
    assert.ok(item.tags.includes('手写体'), `missing handwriting tag: ${item.id}`);
  }
});
