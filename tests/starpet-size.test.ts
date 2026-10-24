import test from 'node:test';
import assert from 'node:assert/strict';
import { createStarPetSizeWriter } from '../src/starpet-size.ts';

const preferences = (size: number) => ({ enabled: false, mood: 'idle' as const, color: '#FFD66E', size });
const flush = () => new Promise<void>(resolve => setImmediate(resolve));
function slowWriter() {
  let active = 0, maximum = 0;
  const requests: Array<{ size: number; resolve: () => void; reject: (error: Error) => void }> = [];
  const writer = createStarPetSizeWriter(size => {
    maximum = Math.max(maximum, ++active);
    return new Promise<ReturnType<typeof preferences>>((resolve, reject) => {
      requests.push({ size, resolve: () => resolve(preferences(size)), reject });
    }).finally(() => { active--; });
  });
  return { writer, requests, maximum: () => maximum };
}

test('慢响应时仅保留最新待写大小，同轮全部调用得到最终成功值', async () => {
  const { writer, requests, maximum } = slowWriter();
  const saves = [writer.save(80), writer.save(90), writer.save(120)];
  let settled = false;
  void saves[0]!.then(() => { settled = true; });
  assert.deepEqual(requests.map(request => request.size), [80]);
  requests[0]!.resolve(); await flush();
  assert.deepEqual(requests.map(request => request.size), [80, 120]);
  assert.equal(settled, false);
  saves.push(writer.save(140), writer.save(160));
  requests[1]!.resolve(); await flush();
  assert.deepEqual(requests.map(request => request.size), [80, 120, 160]);
  assert.equal(settled, false);
  requests[2]!.resolve();
  assert.deepEqual(await Promise.all(saves), saves.map(() => preferences(160)));
  assert.equal(maximum(), 1);
});

test('中间失败继续最新输入，最终失败统一拒绝，之后仍可重新保存', async () => {
  const { writer, requests, maximum } = slowWriter();
  const first = writer.save(80), last = writer.save(120);
  requests[0]!.reject(new Error('intermediate failure')); await flush();
  assert.deepEqual(requests.map(request => request.size), [80, 120]);
  requests[1]!.resolve();
  assert.deepEqual(await Promise.all([first, last]), [preferences(120), preferences(120)]);

  const failures = [writer.save(140), writer.save(160)];
  const rejected = failures.map(promise => assert.rejects(promise, /last failure/));
  requests[2]!.resolve(); await flush();
  requests[3]!.reject(new Error('last failure'));
  await Promise.all(rejected);
  const retry = writer.save(100);
  requests[4]!.resolve();
  assert.deepEqual(await retry, preferences(100));
  assert.equal(maximum(), 1);
});

test('响应已到达及完成回调的边界输入都不会遗漏，同步异常也可恢复', async () => {
  const { writer, requests } = slowWriter();
  const first = writer.save(80);
  requests[0]!.resolve();
  const boundary = writer.save(110);
  const nextBatch = first.then(() => writer.save(140));
  await flush();
  assert.deepEqual(requests.map(request => request.size), [80, 110]);
  requests[1]!.resolve(); await flush();
  assert.deepEqual(await Promise.all([first, boundary]), [preferences(110), preferences(110)]);
  assert.deepEqual(requests.map(request => request.size), [80, 110, 140]);
  requests[2]!.resolve();
  assert.deepEqual(await nextBatch, preferences(140));

  const throwing = createStarPetSizeWriter(size => {
    if (size === 80) throw new Error('synchronous failure');
    return Promise.resolve(preferences(size));
  });
  await assert.rejects(throwing.save(80), /synchronous failure/);
  assert.deepEqual(await throwing.save(100), preferences(100));
});
