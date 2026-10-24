import type { Settings } from './types';

export interface ThemeOrigin { x: number; y: number }
export function themeCircle(origin: ThemeOrigin | undefined, width: number, height: number) {
  const x = Math.max(0, Math.min(width, origin?.x ?? width / 2));
  const y = Math.max(0, Math.min(height, origin?.y ?? height / 2));
  return { x, y, radius: Math.ceil(Math.hypot(Math.max(x, width - x), Math.max(y, height - y))) };
}

// Theme writes are independent of the full settings save and never run retention.
export function createThemeWriter(write: (theme: Settings['theme']) => Promise<Settings>) {
  let queue: Promise<unknown> = Promise.resolve();
  return {
    save(theme: Settings['theme']) {
      const result = queue.then(() => write(theme));
      queue = result.catch(() => {});
      return result;
    },
    idle: () => queue,
  };
}

interface ThemeTransition { ready: Promise<void>; finished: Promise<void>; skipTransition(): void }
let active: ThemeTransition | undefined;
let generation = 0;
export function cancelThemeTransition() { generation++; active?.skipTransition(); active = undefined; }

export function revealTheme(change: () => void | Promise<void>, origin?: ThemeOrigin) {
  cancelThemeTransition();
  const revision = generation;
  const doc = document as Document & { startViewTransition?: (update: () => Promise<void>) => ThemeTransition };
  let applied = false;
  const apply = async () => { if (!applied && revision === generation) { applied = true; await change(); } };
  if (!doc.startViewTransition || window.matchMedia('(prefers-reduced-motion: reduce)').matches || document.hidden) { void apply(); return; }
  try {
    const transition = doc.startViewTransition(apply);
    active = transition;
    // Root snapshots suspend hit-testing. Finish before a subsequent mouse/keyboard
    // action instead of making that action wait for the decorative reveal.
    const interrupt = () => transition.skipTransition();
    const inputEvents = ['pointermove', 'pointerdown', 'keydown', 'wheel'] as const;
    for (const event of inputEvents) document.addEventListener(event, interrupt, { capture: true, passive: true });
    void transition.ready.then(() => {
      if (revision !== generation) return;
      const { x, y, radius } = themeCircle(origin, window.innerWidth, window.innerHeight);
      try {
        document.documentElement.animate({ clipPath: [`circle(0px at ${x}px ${y}px)`, `circle(${radius}px at ${x}px ${y}px)`] }, {
          duration: 420, easing: 'cubic-bezier(.2,.75,.25,1)', fill: 'both', pseudoElement: '::view-transition-new(root)',
        });
      } catch { transition.skipTransition(); }
    }).catch(() => { void apply(); });
    void transition.finished.catch(() => {}).finally(() => {
      for (const event of inputEvents) document.removeEventListener(event, interrupt, { capture: true });
      if (active === transition) active = undefined;
    });
  } catch { void apply(); }
}
