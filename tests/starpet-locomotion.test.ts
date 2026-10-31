import test from 'node:test';
import assert from 'node:assert/strict';
import { createLocomotion, travelGait } from '../src/starpet-locomotion.ts';

test('拖动方向决定步态：横向奔跑、向上攀爬、向下扑腾，慢速和异常值不起步', () => {
  assert.deepEqual(travelGait({ x: .6, y: .1 }), { gait: 'run', speed: Math.hypot(.6, .1), side: 1 });
  assert.equal(travelGait({ x: -.5, y: 0 }).side, -1);
  assert.equal(travelGait({ x: .1, y: -.7 }).gait, 'climb');
  assert.equal(travelGait({ x: 0, y: .7 }).gait, 'fall');
  assert.equal(travelGait({ x: .02, y: .02 }).gait, null);
  assert.equal(travelGait({ x: NaN, y: Infinity }).gait, null);
  assert.equal(travelGait(undefined).gait, null);
  assert.ok(travelGait({ x: 9, y: 0 }).speed <= 1);
});

test('奔跑时身体朝前倾、双脚交替离地，停下后平滑回到自由姿态', () => {
  const motion = createLocomotion();
  let pose = motion.step(0, { x: .8, y: 0 }, null);
  const heights = new Set<string>();
  for (let i = 0; i < 60; i++) {
    pose = motion.step(1 / 60, { x: .8, y: 0 }, null);
    heights.add(pose.limbs.map(([, y]) => y < 206 ? 'up' : 'down').join());
  }
  assert.equal(pose.gait, 'run');
  assert.ok(pose.weight > .9 && pose.tilt > 10 && pose.gazeX > 0);
  assert.equal(pose.limbs.length, 2);
  assert.ok(heights.has('up,down') && heights.has('down,up'), 'feet alternate');
  const left = createLocomotion();
  for (let i = 0; i < 30; i++) pose = left.step(1 / 60, { x: -.8, y: 0 }, null);
  assert.ok(pose.tilt < 0 && pose.gazeX < 0);
  for (let i = 0; i < 120; i++) pose = left.step(1 / 60, { x: 0, y: 0 }, null);
  assert.ok(pose.weight < .05);
});

test('贴边姿态：侧边趴墙伸手、底边趴下、顶边吊挂，并持续有小动作', () => {
  for (const [edge, gait] of [['left', 'cling'], ['right', 'cling'], ['bottom', 'lie'], ['top', 'hang']] as const) {
    const motion = createLocomotion();
    const samples = [];
    for (let i = 0; i < 600; i++) samples.push(motion.step(1 / 60, undefined, edge));
    const pose = samples.at(-1)!;
    assert.equal(pose.gait, gait, edge);
    assert.equal(pose.limbs.length, 2);
    for (const value of [pose.x, pose.y, pose.tilt, pose.sx, pose.sy, ...pose.limbs.flat()]) assert.ok(Number.isFinite(value));
    const tilts = samples.slice(300).map(sample => sample.tilt);
    assert.ok(Math.max(...tilts) - Math.min(...tilts) > 1, `${edge} keeps moving while docked`);
    if (edge === 'left') assert.ok(pose.tilt < 0 && pose.limbs.every(([x]) => x < 40));
    if (edge === 'right') assert.ok(pose.tilt > 0 && pose.limbs.every(([x]) => x > 200));
    if (edge === 'bottom') assert.ok(pose.sy < .95 && pose.y > 5);
    if (edge === 'top') assert.ok(pose.ground === 0 && pose.limbs.every(([, y]) => y < 50));
  }
  // Grabbing the pet off an edge switches straight back to running.
  const motion = createLocomotion();
  for (let i = 0; i < 60; i++) motion.step(1 / 60, undefined, 'right');
  assert.equal(motion.step(1 / 60, { x: -.7, y: 0 }, 'right').gait, 'run');
});
