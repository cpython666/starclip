import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
let sequence = 0;
async function load(native: boolean, invoke: (command: string, args?: unknown) => Promise<unknown>) {
  Object.assign(globalThis, { window: native ? { __TAURI_INTERNALS__: {} } : {}, aiApiInvoke: invoke });
  const mockUrl = 'data:text/javascript;base64,' + Buffer.from(`export const invoke=(...args)=>globalThis.aiApiInvoke(...args); // ${sequence++}`).toString('base64');
  const source = fs.readFileSync(new URL('../src/ai-api.ts', import.meta.url), 'utf8').replace("'@tauri-apps/api/core'", `'${mockUrl}'`);
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  return (await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'))).aiApi;
}
test('AI wrapper stays off in browser and native calls use explicit record IDs without swallowing refusal', async () => {
  const calls: Array<[string, unknown]> = [];
  const browser = await load(false, async () => { throw new Error('must not invoke'); });
  assert.equal((await browser.state()).enabled, false); assert.equal((await browser.state()).available, false);
  await assert.rejects(browser.enable(true), /桌面版/);
  const request = { name: '原创测试连接', clipIds: [1, 4], fields: ['content'], maxResults: 2, expiresHours: 1 };
  const native = await load(true, async (command, args) => { calls.push([command, args]); if (command === 'create_ai_connection') throw new Error('原创测试：记录不可授权'); return { enabled: false }; });
  await native.state(); await native.enable(false); await assert.rejects(native.create(request), /记录不可授权/); await native.revoke('fixture'); await native.config('fixture'); await native.clearAudit();
  assert.deepEqual(calls, [['get_ai_state', undefined], ['set_ai_enabled', { enabled: false }], ['create_ai_connection', { request }], ['revoke_ai_connection', { id: 'fixture' }], ['get_ai_connection_config', { id: 'fixture' }], ['clear_ai_audit', undefined]]);
});
