import test from 'node:test';
import assert from 'node:assert/strict';
import { actionDuration, actionPose } from '../src/starpet-action.ts';

test('主动动作在两端恢复原姿态，减少动态效果没有空间变换', () => {
  for (const id of Object.keys(actionDuration) as Array<keyof typeof actionDuration>) {
    const start = actionPose(id, 0), end = actionPose(id, actionDuration[id]);
    assert.deepEqual(start, actionPose(null, 0));
    assert.ok(Math.abs(end.y) < 1e-10 && Math.abs(end.sx - 1) < 1e-10 && Math.abs(end.sy - 1) < 1e-10);
    assert.ok(Math.abs(end.rotation % 360) < 1e-10);
    assert.deepEqual(actionPose(id, actionDuration[id] / 2, true), start);
    for (let frame = 0; frame <= 120; frame++) {
      const pose = actionPose(id, actionDuration[id] * frame / 120);
      assert.ok(Object.values(pose).every(Number.isFinite));
      assert.ok(pose.sx >= .72 && pose.sy <= 1.08 && Math.abs(pose.y) <= 7);
    }
  }
  assert.deepEqual(actionPose('twirl', NaN), actionPose(null, 0));
  assert.deepEqual(actionPose('twirl', -1), actionPose(null, 0));
});
