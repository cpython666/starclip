import type { StarPetPreferences } from './starpet-api';

export function createStarPetSizeWriter(write: (size: number) => Promise<StarPetPreferences>) {
  let pending: number | undefined;
  let running = false;
  let waiters: Array<{ resolve: (value: StarPetPreferences) => void; reject: (reason: unknown) => void }> = [];

  async function drain() {
    running = true;
    let result: StarPetPreferences | undefined, failure: unknown, failed = false;
    while (pending !== undefined) {
      const size = pending;
      pending = undefined;
      try { result = await write(size); failed = false; }
      catch (error) { failure = error; failed = true; }
    }
    const settled = waiters;
    waiters = [];
    // Reset before resolving: a completion handler can immediately start a new batch.
    running = false;
    for (const waiter of settled) {
      if (failed) waiter.reject(failure);
      else waiter.resolve(result!);
    }
  }

  return {
    save(size: number): Promise<StarPetPreferences> {
      return new Promise((resolve, reject) => {
        pending = size;
        waiters.push({ resolve, reject });
        if (!running) void drain();
      });
    },
  };
}
