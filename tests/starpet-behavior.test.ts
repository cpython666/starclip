import test from 'node:test';
import assert from 'node:assert/strict';
import { createPetBehavior } from '../src/starpet-behavior.ts';
import type { PetBehavior, PetContext, PetPointer, PetPose } from '../src/starpet-behavior.ts';

function seeded(seed: number) {
  return () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 0x100000000;
  };
}
const away: PetPointer = Object.freeze({ x: 0, y: 0, inside: false });
const left: PetPointer = Object.freeze({ x: -4.5, y: -2.5, inside: true });
const right: PetPointer = Object.freeze({ x: 4.5, y: 2.5, inside: true });
type Director = ReturnType<typeof createPetBehavior>;

function bounded(pose: PetPose) {
  for (const [key, value] of Object.entries(pose)) {
    if (key !== 'behavior') assert.ok(Number.isFinite(value), `${pose.behavior}.${key} must be finite`);
  }
  assert.ok(Math.abs(pose.x) <= 12 && pose.y >= -36 && pose.y <= 8, 'body motion stays close to its resting position');
  assert.ok(Math.abs(pose.tilt) <= 16 && pose.sx >= .8 && pose.sx <= 1.25 && pose.sy >= .7 && pose.sy <= 1.2);
  assert.ok(Math.abs(pose.gazeX) <= 6 && Math.abs(pose.gazeY) <= 4);
  for (const key of ['squint', 'widen', 'wink', 'smile', 'wake', 'energy'] as const) {
    assert.ok(pose[key] >= 0 && pose[key] <= 1, `${key} stays within its expression range`);
  }
  assert.equal(Object.hasOwn(pose, 'mood'), false, 'temporary actions do not request a persisted mood change');
}

function advance(director: Director, seconds: number, context: PetContext, pointer: PetPointer = away) {
  const poses: PetPose[] = [];
  for (let frame = 0; frame < Math.round(seconds / .05); frame++) {
    const pose = director.step(.05, context, pointer);
    bounded(pose); poses.push(pose);
  }
  return poses;
}

function touchContinuously(director: Director, context: PetContext, pointer: PetPointer) {
  const poses: PetPose[] = [];
  for (let press = 0; press < 15; press++) {
    director.touch(context, pointer);
    poses.push(...advance(director, .2, context, pointer));
  }
  return poses;
}

test('自主动作间有停顿且不连续重复，日常陪伴不跳跃，活泼状态可以跳跃', () => {
  for (const context of ['social', 'playful'] as const) {
    const director = createPetBehavior(seeded(0x57a2));
    const sequence: PetBehavior[] = [];
    let previous: PetBehavior = 'rest', restFrames = 0, observingGaze = 0;
    for (const pose of advance(director, 240, context)) {
      if (pose.behavior === 'rest') restFrames++;
      else if (pose.behavior !== previous) {
        assert.equal(previous, 'rest', 'autonomous actions must not run directly into one another');
        assert.ok(restFrames >= 10, 'a pause must be perceptible, not a single frame');
        assert.notEqual(pose.behavior, sequence.at(-1));
        sequence.push(pose.behavior); restFrames = 0;
      }
      if (pose.behavior === 'observe') observingGaze = Math.max(observingGaze, Math.abs(pose.gazeX));
      previous = pose.behavior;
    }
    assert.ok(sequence.length > 20 && new Set(sequence).size >= 4, 'fixed-seed long idle exercises a varied action sequence');
    assert.ok(observingGaze > 1, 'autonomous eyes still move when the pointer has never entered');
    assert.equal(sequence.includes('hop'), context === 'playful');
  }
});

test('睡眠长待机保持安静，摸醒只是轻动；专注与切换到睡眠都不会接着跳', () => {
  const sleeper = createPetBehavior(seeded(10));
  for (const pose of advance(sleeper, 300, 'sleep')) {
    assert.equal(pose.behavior, 'rest');
    assert.deepEqual([pose.x, pose.y, pose.tilt, pose.gazeX, pose.gazeY, pose.wake], [0, 0, 0, 0, 0, 0]);
  }
  sleeper.touch('sleep', left);
  assert.equal(sleeper.step(.05, 'sleep', left).behavior, 'stir');
  sleeper.touch('sleep', left);
  const stirring = advance(sleeper, 3.5, 'sleep', left);
  assert.ok(stirring.some(pose => pose.wake > .5));
  assert.ok(stirring.every(pose => ['stir', 'rest'].includes(pose.behavior) && pose.x === 0 && pose.y === 0));
  assert.equal(stirring.at(-1)!.behavior, 'rest');
  const repeatedWake = touchContinuously(sleeper, 'sleep', left);
  assert.ok(repeatedWake.some(pose => pose.wake > .8), 'pressing every 200 ms must not trap waking in anticipation');
  assert.ok(repeatedWake.every(pose => ['stir', 'rest'].includes(pose.behavior) && pose.x === 0 && pose.y === 0));

  const focused = createPetBehavior(seeded(99));
  const working = advance(focused, 120, 'focus');
  focused.touch('focus', right); focused.step(.1, 'focus', right); focused.touch('focus', right);
  working.push(...advance(focused, 4, 'focus', right));
  assert.ok(working.some(pose => pose.behavior === 'nod'));
  assert.ok(working.every(pose => ['rest', 'observe', 'nod'].includes(pose.behavior) && pose.y >= 0));
  const repeatedNods = touchContinuously(focused, 'focus', right);
  assert.ok(repeatedNods.some(pose => pose.y > 3 && pose.sy < .975), 'repeated presses still produce a visible downward nod');
  assert.ok(repeatedNods.every(pose => ['nod', 'rest'].includes(pose.behavior) && pose.y >= 0));

  const lively = createPetBehavior(seeded(0x57a2));
  let hopped = false;
  for (let frame = 0; frame < 4000; frame++) {
    if (lively.step(.05, 'playful', away).behavior === 'hop') { hopped = true; break; }
  }
  assert.ok(hopped);
  const asleep = lively.step(.05, 'sleep', away);
  bounded(asleep); assert.equal(asleep.behavior, 'rest'); assert.equal(asleep.y, 0);
});

test('第一次触碰会亲昵回应，快速连摸变成怕痒，停顿后恢复轻柔回应', () => {
  const director = createPetBehavior(seeded(37));
  director.touch('social', left);
  const first = advance(director, .5, 'social', left);
  assert.ok(first.every(pose => pose.behavior === 'nuzzle'));
  assert.ok(first.some(pose => pose.x < -1 && pose.tilt < -1 && pose.smile > .5));
  director.touch('social', left);
  const repeated = advance(director, 2, 'social', away);
  assert.equal(repeated[0]!.behavior, 'tickle');
  assert.ok(repeated.every(pose => ['tickle', 'rest'].includes(pose.behavior) && pose.y === 0));
  assert.ok(repeated.some(pose => pose.tilt < -1) && repeated.some(pose => pose.tilt > 1));
  assert.ok(repeated.some(pose => pose.smile > .8 && pose.squint > .8));
  assert.equal(repeated.at(-1)!.behavior, 'rest');
  director.touch('social', right);
  const later = advance(director, .5, 'social', right);
  assert.ok(later.every(pose => pose.behavior === 'nuzzle'));
  assert.ok(later.some(pose => pose.x > 1 && pose.tilt > 1));
  const continuousTickles = touchContinuously(director, 'social', right);
  assert.ok(continuousTickles.every(pose => ['tickle', 'rest'].includes(pose.behavior) && pose.y === 0));
  assert.ok(continuousTickles.some(pose => pose.tilt < -1) && continuousTickles.some(pose => pose.tilt > 1),
    'continuous tickling must advance through both sides of the laugh instead of restarting its first sway');
  assert.deepEqual(left, { x: -4.5, y: -2.5, inside: true });
  assert.deepEqual(right, { x: 4.5, y: 2.5, inside: true });
});

test('鼠标离开后视线短暂停留再消退，独立于自主动作且异常帧时间不产生无效数值', () => {
  const director = createPetBehavior(seeded(73));
  const hovering = advance(director, 2, 'focus', left);
  assert.ok(hovering.every(pose => pose.gazeX === left.x && pose.gazeY === left.y));
  assert.equal(hovering.at(-1)!.behavior, 'rest', 'finish the arrival nod before measuring attention');
  const leaving = advance(director, 2.5, 'focus', away);
  assert.ok(leaving.every(pose => pose.behavior === 'rest'), 'do not mistake autonomous eye motion for fading attention');
  assert.equal(leaving[9]!.gazeX, left.x, 'attention is retained for a moment after leaving');
  assert.ok(Math.abs(leaving[29]!.gazeX) > 0 && Math.abs(leaving[29]!.gazeX) < Math.abs(left.x));
  for (let frame = 1; frame < leaving.length; frame++) {
    assert.ok(Math.abs(leaving[frame]!.gazeX) <= Math.abs(leaving[frame - 1]!.gazeX) + 1e-12);
    assert.ok(Math.abs(leaving[frame]!.gazeY) <= Math.abs(leaving[frame - 1]!.gazeY) + 1e-12);
  }
  assert.equal(leaving.at(-1)!.gazeX, 0); assert.equal(leaving.at(-1)!.gazeY, 0);
  for (const seconds of [NaN, Infinity, -Infinity, -1, 0, 1e6]) bounded(director.step(seconds, 'focus', away));
});
