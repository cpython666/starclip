import test from 'node:test';
import assert from 'node:assert/strict';
import { getStarPetMood, starPetGroups, starPetMoods } from '../src/starpet.ts';

test('星伴目录保留 32 个稳定状态，分组完整，未知状态安全回到日常陪伴', () => {
  const ids = 'idle greet curious listening sleepy sleeping waking stretch happy delighted love shy surprised puzzled sad crying angry nervous proud relieved thinking searching reading writing drawing focused waiting loading success error celebrate resting'.split(' ');
  assert.deepEqual(starPetMoods.map(mood => mood.id), ids);
  assert.equal(new Set(starPetMoods.map(mood => mood.id)).size, 32);
  assert.deepEqual(starPetGroups.map(group => starPetMoods.filter(mood => mood.group === group.id).length), [8, 12, 12]);
  assert.equal(new Set(starPetMoods.map(({ eyes, mouth, prop, motion }) => [eyes, mouth, prop, motion].join('/'))).size, 32);
  for (const mood of starPetMoods) {
    assert.ok(mood.name.trim() && mood.description.trim());
    assert.match(mood.accent, /^#[0-9a-f]{6}$/i);
    assert.ok(Number.isFinite(mood.tilt) && Math.abs(mood.tilt) <= 15);
    assert.ok(mood.blush >= 0 && mood.blush <= 1);
    assert.equal(getStarPetMood(mood.id), mood);
  }
  assert.equal(getStarPetMood('removed-or-invalid-state'), starPetMoods[0]);
});
