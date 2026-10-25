import test from 'node:test';
import assert from 'node:assert/strict';
import { blinkOpenness, cubicBezierEase, sampleHop, starOutline, stepSpring } from '../src/starpet-motion.ts';

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
  const longPause = stepSpring(expected, 0, 1e6);
  assert.ok(longPause.value === 0 && longPause.velocity === 0);
});

test('三次贝塞尔缓动端点与分阶段弧线连续，横向去而复返', () => {
  assert.equal(cubicBezierEase(0), 0);
  assert.equal(cubicBezierEase(.25), 5 / 32);
  assert.equal(cubicBezierEase(.5), .5);
  assert.equal(cubicBezierEase(.75), 27 / 32);
  assert.equal(cubicBezierEase(1), 1);
  assert.equal(cubicBezierEase(-1), 0);
  assert.equal(cubicBezierEase(2), 1);
  assert.equal(cubicBezierEase(NaN), 0);
  assert.ok(cubicBezierEase(1e-6) < 1e-10);
  assert.ok(1 - cubicBezierEase(1 - 1e-6) < 1e-10);
  assert.equal(sampleHop(.14).x, -1);
  assert.equal(sampleHop(.14).y, 5);
  assert.equal(sampleHop(.59).x, 1);
  assert.equal(sampleHop(.59).y, 0);
  const crest = sampleHop(.14 + (.59 - .14) / 2);
  // The cubic's midpoint departs from the endpoint chord, rather than easing along a line.
  assert.ok(Math.abs(crest.x - 5.25) < 1e-10);
  assert.ok(Math.abs(crest.y + 28.625) < 1e-10);
  assert.equal(sampleHop(0).x, 0);
  assert.equal(sampleHop(1.08).x, 0);
  for (const boundary of [0, .14, .27, .42, .59, .67, .82, 1.08]) {
    const before = sampleHop(boundary - 1e-6), after = sampleHop(boundary + 1e-6);
    for (const axis of ['x', 'y', 'sx', 'sy'] as const) {
      assert.ok(Math.abs(after[axis] - before[axis]) < .001);
    }
  }
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
    assert.ok(Math.abs(pose.x) < 8 && pose.y >= -32 && pose.y <= 5);
    assert.ok(pose.sx >= .92 && pose.sx <= 1.18 && pose.sy >= .79 && pose.sy <= 1.13);
  }
  assert.equal(starOutline(100, -100), starOutline(6, -5));
  assert.equal(starOutline(NaN, Infinity), starOutline(0, 0));
});
