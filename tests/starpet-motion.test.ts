import test from 'node:test';
import assert from 'node:assert/strict';
import { blinkOpenness, sampleHop, starOutline, stepHop, stepSpring } from '../src/starpet-motion.ts';

test('星芽视线弹簧与帧率无关，回中稳定，异常时间不传播到 SVG', () => {
  const simulate = (fps: number) => {
    let state = { value: 0, velocity: 0 };
    for (let i = 0; i < fps / 2; i++) state = stepSpring(state, 5.5, 1 / fps);
    return state;
  };
  const expected = simulate(60);
  for (const fps of [30, 120, 144]) {
    const actual = simulate(fps);
    assert.ok(Math.abs(actual.value - expected.value) < 1e-10);
    assert.ok(Math.abs(actual.velocity - expected.velocity) < 1e-10);
  }
  const returned = stepSpring(expected, 0, 3);
  assert.ok(Math.abs(returned.value) < 1e-10);
  assert.deepEqual(stepSpring(expected, 0, 0), expected);
  assert.deepEqual(stepSpring(expected, 0, NaN), { value: 0, velocity: 0 });
});

test('星芽动作具有蓄力、起跳、落地和回弹，眨眼节奏变化且变形有界', () => {
  assert.ok(sampleHop(.14).sy < .9 && sampleHop(.14).y > 0);
  assert.ok(sampleHop(.42).y <= -22);
  assert.ok(sampleHop(.67).sx > 1.1 && sampleHop(.67).sy < .85);
  assert.ok(sampleHop(.82).y < 0);
  assert.deepEqual(sampleHop(1.08), sampleHop(0));
  assert.deepEqual(sampleHop(Infinity), sampleHop(0));
  assert.ok(blinkOpenness(.1) < .1);
  assert.equal(blinkOpenness(.5), 1);
  assert.ok(blinkOpenness(2.7 + .1) < .1);
  assert.equal(blinkOpenness(2.7 * 2 + .1), 1);
  assert.ok(blinkOpenness(2.7 + 4.6 + .38) < .1);
  for (let time = 0; time < 30; time += .017) {
    const openness = blinkOpenness(time), pose = sampleHop(time);
    assert.ok(openness >= .059 && openness <= 1);
    assert.ok(pose.sx >= .92 && pose.sx <= 1.18 && pose.sy >= .79 && pose.sy <= 1.13);
  }
  assert.equal(starOutline(100, -100), starOutline(6, -5));
  assert.equal(starOutline(NaN, Infinity), starOutline(0, 0));
});

test('连点重播与接回日常跳跃保留当前姿态和速度，快速衔接仍保留清楚的起落', () => {
  const resting = { y: { value: 0, velocity: 0 }, sx: { value: 1, velocity: 0 }, sy: { value: 1, velocity: 0 } };
  let state = resting, highest = 0, widest = 1, lowestScale = 1;
  for (let i = 0; i < 78; i++) {
    state = stepHop(state, i / 60, 1 / 60);
    highest = Math.min(highest, state.y.value);
    widest = Math.max(widest, state.sx.value);
    lowestScale = Math.min(lowestScale, state.sy.value);
  }
  assert.ok(highest < -20 && widest > 1.1 && lowestScale < .88);
  for (const nextAge of [.42, 0, .42, 0, .42]) {
    const before = state;
    assert.deepEqual(stepHop(before, nextAge, 0), before);
    const immediate = stepHop(before, nextAge, 1e-6);
    for (const axis of ['y', 'sx', 'sy'] as const) {
      assert.ok(Math.abs(immediate[axis].value - before[axis].value) < .001);
    }
    state = stepHop(before, nextAge, 1 / 60);
    assert.ok(Math.abs(state.y.value - before.y.value) < 6);
    assert.notEqual(state.y.value, sampleHop(nextAge).y);
    for (let i = 1; i < 12; i++) state = stepHop(state, nextAge + i / 60, 1 / 60);
  }
});
