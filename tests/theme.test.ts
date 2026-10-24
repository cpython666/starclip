import test from 'node:test';
import assert from 'node:assert/strict';
import { cancelThemeTransition, createThemeWriter, revealTheme, themeCircle } from '../src/theme.ts';
import type { Settings } from '../src/types.ts';

test('theme circle covers all corners, including a trigger on the viewport edge', () => {
  for (const origin of [{ x: 1230, y: 30 }, { x: -100, y: 1200 }, undefined]) {
    const { x, y, radius } = themeCircle(origin, 1280, 720);
    assert.ok(x >= 0 && x <= 1280 && y >= 0 && y <= 720);
    for (const corner of [[0, 0], [0, 720], [1280, 0], [1280, 720]]) assert.ok(Math.hypot(corner[0]! - x, corner[1]! - y) <= radius);
  }
});

test('theme writes stay serial after failure and settings save can await the latest selection', async () => {
  const calls: string[] = [];
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  const writer = createThemeWriter(async theme => {
    calls.push(theme);
    if (theme === 'forest') { await gate; throw new Error('disk unavailable'); }
    return { theme } as Settings;
  });
  const first = writer.save('forest');
  const failure = assert.rejects(first, /disk unavailable/);
  const second = writer.save('rose');
  const last = writer.save('light');
  await Promise.resolve();
  assert.deepEqual(calls, ['forest']);
  release();
  await writer.idle(); await failure;
  assert.deepEqual(calls, ['forest', 'rose', 'light']);
  assert.equal((await second).theme, 'rose');
  assert.equal((await last).theme, 'light');
});

test('theme reveal applies immediately without native support or with reduced motion; fast switches keep only the latest update', async () => {
  let reduced = false, updates = '', starts = 0, animations = 0, skipped = 0;
  const events = new EventTarget();
  const doc: Record<string, unknown> = { hidden: false, documentElement: { animate: () => { animations++; } }, addEventListener: events.addEventListener.bind(events), removeEventListener: events.removeEventListener.bind(events) };
  Object.assign(globalThis, { document: doc, window: { innerWidth: 1280, innerHeight: 720, matchMedia: () => ({ matches: reduced }) } });
  revealTheme(() => { updates += 'fallback '; });
  assert.equal(updates, 'fallback ');
  doc.startViewTransition = (update: () => Promise<void>) => {
    starts++;
    let finish!: () => void;
    return { ready: Promise.resolve().then(update), finished: new Promise<void>(resolve => { finish = resolve; }), skipTransition: () => { skipped++; finish(); } };
  };
  reduced = true;
  revealTheme(() => { updates += 'reduced '; });
  assert.equal(starts, 0);
  reduced = false;
  revealTheme(() => { updates += 'stale '; });
  revealTheme(() => { updates += 'latest '; }, { x: 1200, y: 30 });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(updates, 'fallback reduced latest ');
  assert.equal(starts, 2); assert.equal(animations, 1); assert.equal(skipped, 1);
  events.dispatchEvent(new Event('pointermove'));
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(skipped, 2);
  events.dispatchEvent(new Event('pointermove'));
  assert.equal(skipped, 2, 'input interruption listeners are removed with the transition');
  revealTheme(() => { updates += 'touch '; });
  await new Promise(resolve => setImmediate(resolve));
  events.dispatchEvent(new Event('pointerdown'));
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(skipped, 3, 'touch also ends the decorative animation immediately');
  for (const event of ['pointerdown', 'keydown', 'wheel']) events.dispatchEvent(new Event(event));
  assert.equal(skipped, 3, 'all input interruption listeners are cleaned up');
  cancelThemeTransition();
});
