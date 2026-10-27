import test from 'node:test';
import assert from 'node:assert/strict';
import { downloadProgress, updateInProgress, updatePhaseText } from '../src/update-state.ts';
import type { UpdateStatus } from '../src/update-api.ts';

const base: UpdateStatus = { revision: 0, currentVersion: '0.2.0', configured: true, phase: 'idle', downloaded: 0 };
test('update states distinguish unchecked, no release, errors and restart; download handles unknown totals', () => {
  assert.match(updatePhaseText(base), /按需检查/);
  assert.match(updatePhaseText({ ...base, phase: 'up-to-date' }), /已检查，当前没有更新/);
  assert.match(updatePhaseText({ ...base, configured: false }), /尚未配置/);
  assert.equal(updatePhaseText({ ...base, phase: 'error', error: '测试网络错误' }), '测试网络错误');
  assert.equal(updateInProgress({ ...base, phase: 'backing-up' }), true);
  assert.equal(updateInProgress({ ...base, phase: 'error' }), false);
  assert.equal(updateInProgress({ ...base, phase: 'ready' }), false);
  assert.match(updatePhaseText({ ...base, phase: 'ready', available: { version: '0.3.0' } }), /0\.3\.0.*已下载并验证.*等待安装/);
  assert.match(updatePhaseText({ ...base, phase: 'installed' }), /重启/);
  assert.match(updatePhaseText({ ...base, phase: 'restart-required' }), /未能启动.*重启/);
  assert.equal(updateInProgress({ ...base, phase: 'restart-required' }), false);
  assert.equal(downloadProgress({ ...base, downloaded: 25, total: 100 }), 25);
  assert.equal(downloadProgress({ ...base, downloaded: 25 }), undefined);
  assert.equal(downloadProgress({ ...base, downloaded: 25, total: 0 }), undefined);
  assert.equal(downloadProgress({ ...base, downloaded: 25, total: Infinity }), undefined);
  assert.equal(downloadProgress({ ...base, downloaded: 125, total: 100 }), 100);
  assert.equal(downloadProgress({ ...base, downloaded: -1, total: 100 }), 0);
});
