import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { createHandwritingPreview } from '../src/handwriting-preview.ts';
import type { HandwritingOptions } from '../src/types.ts';

const options = (text: string): HandwritingOptions => ({ text, font: '/fixture/font.ttf', background: 'solid:#ffffff:1200:1600', fontSize: 30, ink: '#24324a', padding: 100 });
const snapshot = (text: string) => ({ key: text, options: options(text) });
const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

test('handwriting debounce coalesces edits, renders only one request and never publishes stale results', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const calls: string[] = [], results: string[] = [], errors: unknown[] = [];
  const jobs: ReturnType<typeof deferred<string>>[] = [];
  const scheduler = createHandwritingPreview(value => {
    calls.push(value.text); const job = deferred<string>(); jobs.push(job); return job.promise;
  }, { state() {}, result: (png, key) => results.push(`${key}:${png}`), error: reason => errors.push(reason) });
  t.after(() => scheduler.dispose());
  scheduler.update(snapshot('第一稿'));
  t.mock.timers.tick(200);
  scheduler.update(snapshot('第二稿'));
  t.mock.timers.tick(349);
  assert.deepEqual(calls, []);
  t.mock.timers.tick(1);
  assert.deepEqual(calls, ['第二稿']);
  scheduler.update(snapshot('第三稿'));
  t.mock.timers.tick(350);
  scheduler.update(snapshot('最终稿'));
  t.mock.timers.tick(350);
  assert.equal(calls.length, 1, 'new edits must not overlap the current request');
  jobs[0]!.resolve('obsolete'); await flush();
  assert.deepEqual(results, []);
  assert.deepEqual(calls, ['第二稿', '最终稿'], 'only the latest waiting snapshot renders');
  jobs[1]!.resolve('current'); await flush();
  assert.deepEqual(results, ['最终稿:current']);
  assert.deepEqual(errors, []);
});

test('handwriting invalidation, explicit retry and disposal do not leak delayed work', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const jobs: ReturnType<typeof deferred<string>>[] = [], results: string[] = [], errors: unknown[] = [];
  const scheduler = createHandwritingPreview(() => { const job = deferred<string>(); jobs.push(job); return job.promise; },
    { state() {}, result: png => results.push(png), error: reason => errors.push(reason) });
  scheduler.update(snapshot('空白或超长之前'));
  t.mock.timers.tick(350);
  scheduler.update(null);
  jobs[0]!.resolve('invalid'); await flush();
  assert.deepEqual(results, []);
  t.mock.timers.tick(5000);
  assert.equal(jobs.length, 1, 'invalid input schedules nothing');
  scheduler.update(snapshot('重试样例'));
  t.mock.timers.tick(350);
  jobs[1]!.reject(new Error('fixture failure')); await flush();
  assert.equal(errors.length, 1);
  t.mock.timers.tick(5000);
  assert.equal(jobs.length, 2, 'failure must not trigger automatic retries');
  scheduler.update(snapshot('重试样例'), 0);
  t.mock.timers.tick(0);
  assert.equal(jobs.length, 3, 'explicit retry may start immediately');
  scheduler.update(snapshot('卸载后的草稿'));
  scheduler.dispose();
  jobs[2]!.resolve('after close'); await flush();
  t.mock.timers.tick(5000);
  assert.deepEqual(results, []);
  assert.equal(jobs.length, 3, 'unmount cancels timer and ignores the in-flight result');
  scheduler.update(snapshot('不可复活'));
  t.mock.timers.tick(5000);
  assert.equal(jobs.length, 3);
});

test('native handwriting render, asset discovery and application selection share one queue across panels', async t => {
  const priorWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const calls: string[] = [], jobs: ReturnType<typeof deferred<unknown>>[] = [];
  Object.assign(globalThis, {
    window: { __TAURI_INTERNALS__: {} },
    __handwritingInvoke(command: string) { calls.push(command); const job = deferred<unknown>(); jobs.push(job); return job.promise; },
  });
  t.after(() => {
    if (priorWindow) Object.defineProperty(globalThis, 'window', priorWindow);
    else Reflect.deleteProperty(globalThis, 'window');
    Reflect.deleteProperty(globalThis, '__handwritingInvoke');
  });
  const source = fs.readFileSync(new URL('../src/api.ts', import.meta.url), 'utf8')
    .replace("from './privacy'", `from '${new URL('../src/privacy.ts', import.meta.url).href}'`)
    .replace("from './snippets'", `from '${new URL('../src/snippets.ts', import.meta.url).href}'`)
    .replace("return (await import('@tauri-apps/api/core')).invoke<T>(command, args);", 'return globalThis.__handwritingInvoke(command, args);');
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  const { api } = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
  const oldPanelRender = api.renderHandwriting(options('关闭前的原创样例'));
  const newPanelAssets = api.handwritingAssets();
  const choose = api.chooseHandwritingApp();
  await flush();
  assert.deepEqual(calls, ['render_handwriting']);
  const rejected = assert.rejects(oldPanelRender, /fixture failure/);
  jobs[0]!.reject(new Error('fixture failure')); await rejected; await flush();
  assert.deepEqual(calls, ['render_handwriting', 'handwriting_assets'], 'failed render must release queue for reopened panel');
  jobs[1]!.resolve({ available: true }); await newPanelAssets; await flush();
  assert.deepEqual(calls, ['render_handwriting', 'handwriting_assets', 'choose_handwriting_app']);
  jobs[2]!.resolve({ available: true }); await choose;
});
