import type { HandwritingOptions } from './types';

export interface HandwritingSnapshot { key: string; options: HandwritingOptions }
export type HandwritingPreviewState = 'idle' | 'waiting' | 'rendering';

// One in-flight request and one replaceable snapshot; no renderer result is
// published after a newer edit, invalid input, or component disposal.
export function createHandwritingPreview(
  render: (options: HandwritingOptions) => Promise<string>,
  callbacks: {
    state: (state: HandwritingPreviewState) => void;
    result: (png: string, key: string) => void;
    error: (error: unknown) => void;
  },
  delay = 350,
) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pending: HandwritingSnapshot | null = null;
  let ready = false, running = false, disposed = false, generation = 0;
  const publishState = () => { if (!disposed) callbacks.state(running ? 'rendering' : pending ? 'waiting' : 'idle'); };
  async function pump() {
    if (disposed || running || !ready || !pending) return;
    const snapshot = pending, version = generation;
    pending = null; ready = false; running = true; publishState();
    try {
      const png = await render(snapshot.options);
      if (!disposed && version === generation) callbacks.result(png, snapshot.key);
    } catch (error) {
      if (!disposed && version === generation) callbacks.error(error);
    } finally {
      running = false; publishState();
      if (!disposed && ready && pending) void pump();
    }
  }
  return {
    update(snapshot: HandwritingSnapshot | null, wait = delay) {
      if (disposed) return;
      generation++; pending = snapshot ? { key: snapshot.key, options: { ...snapshot.options } } : null;
      ready = false; clearTimeout(timer); timer = undefined;
      if (pending) timer = setTimeout(() => { timer = undefined; ready = true; void pump(); }, wait);
      publishState();
    },
    dispose() {
      disposed = true; generation++; pending = null; ready = false;
      clearTimeout(timer); timer = undefined;
    },
  };
}
