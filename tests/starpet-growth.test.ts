import test from 'node:test';
import assert from 'node:assert/strict';
import { availableActions, changeDemoGrowth, emptyGrowth, growthArchive, growthCatalog, levelForXp, levelProgress, parseGrowthArchive, xpForLevel } from '../src/starpet-growth.ts';

test('成长目录与等级闭环：原有表情无关，首日三种独立互动45经验、获得眼镜且不自动穿戴', () => {
  let state = changeDemoGrowth(emptyGrowth(), { kind: 'enabled', enabled: true }, '2026-10-26');
  for (const action of ['greet', 'highfive', 'stretch'] as const) state = changeDemoGrowth(state, { kind: 'action', action }, '2026-10-26');
  assert.equal(state.xp, 45);
  assert.equal(state.interactionDays, 1);
  assert.equal(state.playDays, 1);
  assert.equal(levelForXp(state.xp), 2);
  assert.deepEqual(state.unlockedItems, ['welcome-star', 'moon-glasses']);
  assert.ok(Object.values(state.equipment).every(item => item === null));
  const repeated = changeDemoGrowth(state, { kind: 'action', action: 'highfive' }, '2026-10-26');
  assert.deepEqual(repeated, state);
  assert.equal(levelForXp(39), 1);
  assert.equal(levelForXp(40), 2);
  assert.equal(levelForXp(2469), 19);
  assert.equal(xpForLevel(20), 2470);
  assert.equal(levelProgress(2470).percent, 100);
  assert.equal(levelForXp(10_000_000), 20);
  assert.equal(availableActions(emptyGrowth()).length, 3);
  assert.equal(new Set(growthCatalog.items.map(item => item.id)).size, 10);
});

test('日历回退和养成关闭不授奖，累计天数无需连续，已获物品可卸下但不可越级穿戴', () => {
  let state = emptyGrowth();
  assert.deepEqual(changeDemoGrowth(state, { kind: 'action', action: 'greet' }, '2026-10-01'), state);
  state = changeDemoGrowth(state, { kind: 'enabled', enabled: true }, '2026-10-01');
  for (const day of ['01', '03', '05', '07', '09', '11', '13']) {
    state = changeDemoGrowth(state, { kind: 'action', action: 'highfive' }, `2026-10-${day}`);
    state = changeDemoGrowth(state, { kind: 'action', action: 'greet' }, `2026-10-${day}`);
  }
  assert.equal(state.interactionDays, 7);
  assert.ok(state.achievements.includes('seven-days') && state.achievements.includes('play-five'));
  assert.deepEqual(changeDemoGrowth(state, { kind: 'action', action: 'stretch' }, '2026-10-12'), state);
  assert.throws(() => changeDemoGrowth(state, { kind: 'equip', slot: 'back', itemId: 'night-cape' }));
  assert.throws(() => changeDemoGrowth(state, { kind: 'equip', slot: 'back', itemId: 'moon-glasses' }));
  state = changeDemoGrowth(state, { kind: 'equip', slot: 'face', itemId: 'moon-glasses' });
  assert.ok(state.achievements.includes('first-outfit'));
  state = changeDemoGrowth(state, { kind: 'enabled', enabled: false });
  const disabled = changeDemoGrowth(state, { kind: 'action', action: 'greet' }, '2026-10-26');
  assert.deepEqual(disabled, state);
  state = changeDemoGrowth(state, { kind: 'equip', slot: 'face', itemId: null });
  assert.equal(state.equipment.face, null);
  assert.ok(state.achievements.includes('first-outfit'));
});

test('成长存档导入严格验证类型、日期、重复ID、装备槽和体积，解析不改变来源数据', () => {
  const state = changeDemoGrowth(emptyGrowth(), { kind: 'enabled', enabled: true });
  assert.deepEqual(parseGrowthArchive(growthArchive(state)), state);
  const archive = JSON.parse(growthArchive(state));
  for (const patch of [
    { xp: -1 }, { xp: 1.5 }, { xp: 10_000_001 }, { enabled: 'true' }, { revision: Number.MAX_SAFE_INTEGER + 1 },
    { day: '2026-02-30' }, { day: '2026-02-29' }, { playDays: 1 },
    { unlockedItems: ['missing'] }, { unlockedItems: ['moon-glasses', 'moon-glasses'] },
    { completedTasks: ['greet', 'greet'] }, { version: 2 }, { unrelated: 'private text' },
    { equipment: { back: 'moon-glasses', face: null, hand: null, charm: null }, unlockedItems: ['moon-glasses'] },
  ]) assert.throws(() => parseGrowthArchive(JSON.stringify({ ...archive, growth: { ...archive.growth, ...patch } })));
  assert.throws(() => parseGrowthArchive(JSON.stringify({ format: 'starclip', version: 1, clips: [] })));
  assert.throws(() => parseGrowthArchive(' '.repeat(65537)));
  assert.deepEqual(state, changeDemoGrowth(emptyGrowth(), { kind: 'enabled', enabled: true }));
});
