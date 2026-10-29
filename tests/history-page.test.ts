import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';

test('browser history has bounded pages, global search/counts and clamps after deletion without pruning', async () => {
  Object.assign(globalThis, { window: {} });
  const source = fs.readFileSync(new URL('../src/api.ts', import.meta.url), 'utf8')
    .replace("from './privacy'", `from '${new URL('../src/privacy.ts', import.meta.url).href}'`)
    .replace("from './snippets'", `from '${new URL('../src/snippets.ts', import.meta.url).href}'`)
    .replace("from './i18n/index.ts'", `from '${new URL('../src/i18n/index.ts', import.meta.url).href}'`);
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  const { api } = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  for (const clip of (await api.getState()).clips) if (clip.kind === 'image') await api.deleteClip(clip.id);
  await api.importBackup(JSON.stringify({ format: 'starclip', version: 1, groups: [], clips: Array.from({ length: 123 }, (_, index) => ({ id: index + 1, content: `分页原创样例 ${index}`, title: index === 0 ? 'ÉTOILE 100%_' : '原创固定页', kind: 'text', language: null, pinned: false, groupId: null, createdAt: 1, updatedAt: 1, copyCount: 0 })) }));
  const first = await api.getState({ query: '分页原创样例' }), second = await api.getState({ query: '分页原创样例', page: 2 });
  assert.equal(first.clips.length, 50);
  assert.equal(second.clips.length, 50);
  assert.equal(first.history.total, 123);
  assert(first.history.counts.all > first.history.total, 'global categories include records outside current search');
  assert(first.clips.every((clip: { id: number }) => !second.clips.some((other: { id: number }) => other.id === clip.id)));
  const last = await api.getState({ query: '分页原创样例', page: 999 });
  assert.equal(last.history.page, 3); assert.equal(last.clips.length, 23);
  const found = await api.getState({ query: 'étoile 100%_', page: 3 });
  assert.equal(found.history.page, 1); assert.equal(found.history.total, 1);
  assert.equal(found.clips[0].content, '分页原创样例 0', 'case folded literal search reaches records beyond first page');
  const totalBefore = (JSON.parse(await api.exportBackup())).clips.length;
  await api.getState({ page: 2 }); await api.getState({ page: 3 });
  assert.equal((JSON.parse(await api.exportBackup())).clips.length, totalBefore, 'paging leaves permanent disk history intact');
  for (const clip of last.clips) await api.deleteClip(clip.id);
  assert.equal((await api.getState({ query: '分页原创样例', page: 3 })).history.page, 2);
  const empty = await api.getState({ query: '没有这条固定样例' });
  assert.equal(empty.history.page, 1); assert.equal(empty.history.total, 0); assert.deepEqual(empty.clips, []);
});
