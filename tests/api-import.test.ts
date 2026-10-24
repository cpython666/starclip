import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';

// Load the browser demo without a DOM or native clipboard; fixtures never touch real user data.
test('demo backup import keeps all records and only explicit capture/settings changes apply retention', async () => {
  Object.assign(globalThis, { window: {} });
  const source = fs.readFileSync(new URL('../src/api.ts', import.meta.url), 'utf8')
    .replace("from './snippets'", `from '${new URL('../src/snippets.ts', import.meta.url).href}'`);
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  const { api } = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  const original = await api.getState();
  await api.saveSettings({ ...original.settings, retentionDays: 7 });
  const oldTime = Date.now() - 60 * 86400000;
  const backup = (prefix: string, timestamp: number) => JSON.stringify({
    format: 'starclip', version: 1, groups: [],
    clips: Array.from({ length: 600 }, (_, index) => ({ id: index + 1, content: `${prefix} ${index}`, title: '', kind: 'text', language: null, pinned: false, groupId: null, createdAt: timestamp, updatedAt: timestamp, copyCount: 0 })),
  });
  const notificationReads: Promise<number>[] = [];
  const stop = await api.onChange(() => { notificationReads.push(api.getState().then((value: { clips: unknown[] }) => value.clips.length)); });
  assert.equal(await api.importBackup(backup('保留完整备份', oldTime)), 600);
  const imported = await api.getState();
  assert.equal(imported.clips.length, original.clips.length + 600);
  assert.equal((await api.getState()).clips.length, imported.clips.length, 'reading must not prune');
  const themed = await api.setTheme('forest');
  assert.deepEqual(themed, { ...imported.settings, theme: 'forest' }, 'theme save changes no other preference');
  assert.equal((await api.getState()).clips.length, imported.clips.length, 'changing theme must not run existing retention rules');
  await assert.rejects(api.setTheme('unknown-theme'));
  assert.equal((await api.getState()).settings.theme, 'forest', 'invalid theme leaves the saved preference intact');
  await api.createGroup('通知不应清理记录', '#778899');
  assert.equal((await api.getState()).clips.length, imported.clips.length, 'unrelated change notifications must not prune');
  assert.equal((JSON.parse(await api.exportBackup())).clips.length, imported.clips.length);
  assert((await Promise.all(notificationReads)).every(count => count === imported.clips.length));
  stop();

  const oldClip = imported.clips.find((clip: { content: string }) => clip.content === '保留完整备份 0');
  await api.updateClip(oldClip.id, '元数据标题', null, false);
  assert.equal((await api.getState()).clips.find((clip: { id: number }) => clip.id === oldClip.id).updatedAt, oldTime, 'editing metadata does not renew retention');
  const recaptured = await api.addClip(oldClip.content, '', null);
  assert.equal(recaptured.id, oldClip.id);
  assert.equal(recaptured.title, '元数据标题', 'empty capture title preserves metadata');
  assert(recaptured.updatedAt > oldTime, 'recapturing renews retention');
  assert.equal((await api.getState()).clips.filter((clip: { content: string }) => clip.content.startsWith('保留完整备份')).length, 1, 'capture applies selected time retention');
  const grouped = original.clips.find((clip: { groupId: number | null }) => clip.groupId !== null);
  const groupedAgain = await api.addClip(grouped.content, '', null);
  assert.equal(groupedAgain.groupId, grouped.groupId, 'empty capture group preserves existing group');
  assert.equal(groupedAgain.title, grouped.title);

  await api.saveSettings({ ...original.settings, retentionDays: 0, maxHistory: 500 });
  assert.equal(await api.importBackup(backup('超过五百条', Date.now())), 600);
  assert((await api.getState()).clips.filter((clip: { content: string }) => clip.content.startsWith('超过五百条')).length === 600);
  await api.saveSettings({ ...original.settings, retentionDays: 0, maxHistory: 500 });
  const ordinaryCount = async () => (await api.getState()).clips.filter((clip: { pinned: boolean; groupId: number | null }) => !clip.pinned && clip.groupId === null).length;
  assert.equal(await ordinaryCount(), 500, 'saving settings explicitly applies capacity');
  await api.saveCode(null, '新收藏不会挤出历史', 'const stars = 42;', 'javascript', null);
  assert.equal(await ordinaryCount(), 500, 'new pinned code must not temporarily consume ordinary history capacity');
});
