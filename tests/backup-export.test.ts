import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';

test('native backup returns the chosen path or cancellation and releases its writing listener on every outcome', async () => {
  let writing: (() => void) | undefined;
  let stopped = 0, progress = 0;
  let result: string | null | Error = '/test-only/原创备份.json';
  Object.assign(globalThis, {
    window: { __TAURI_INTERNALS__: {} },
    backupTestEvents: {
      async listen(name: string, callback: () => void) {
        assert.equal(name, 'backup-writing');
        assert.equal(writing, undefined, 'a previous export must leave no listener behind');
        writing = callback;
        return () => { writing = undefined; stopped++; };
      },
    },
    backupTestCore: {
      async invoke(command: string) {
        assert.equal(command, 'save_backup');
        assert(writing, 'progress listener must be registered before opening the save dialog');
        if (result !== null) writing();
        if (result instanceof Error) throw result;
        return result;
      },
    },
  });
  const source = fs.readFileSync(new URL('../src/api.ts', import.meta.url), 'utf8')
    .replace("from './privacy'", `from '${new URL('../src/privacy.ts', import.meta.url).href}'`)
    .replace("from './snippets'", `from '${new URL('../src/snippets.ts', import.meta.url).href}'`)
    .replace("from './i18n/index.ts'", `from '${new URL('../src/i18n/index.ts', import.meta.url).href}'`)
    .replaceAll("import('@tauri-apps/api/core')", 'Promise.resolve(globalThis.backupTestCore)')
    .replaceAll("import('@tauri-apps/api/event')", 'Promise.resolve(globalThis.backupTestEvents)');
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  const { api } = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  const onWriting = () => { progress++; };

  assert.equal(await api.saveBackup(onWriting), '/test-only/原创备份.json');
  assert.equal(progress, 1);
  assert.equal(writing, undefined);
  result = null;
  assert.equal(await api.saveBackup(onWriting), null);
  assert.equal(progress, 1, 'cancellation must not announce writing or success');
  result = new Error('test-only write failure');
  await assert.rejects(api.saveBackup(onWriting), /test-only write failure/);
  assert.equal(writing, undefined, 'a rejected write must also remove the listener');
  result = '/test-only/再次备份.json';
  assert.equal(await api.saveBackup(onWriting), result, 'a failed export must allow retry');
  assert.equal(stopped, 4);
});
