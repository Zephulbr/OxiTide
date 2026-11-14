/// Whether the next track change was asked for by the user (v1.1 spec §12,
/// A-4). The desktop `api` notes it on the commands that start or skip a
/// track (play from a list, next, previous, a queue jump); the engine's next
/// `now_playing` takes it, so automatic advances stay unannounced. No imports:
/// `api.ts` depends on this module.

/// How long a request waits for its `now_playing` (a stream can take a while
/// to resolve); a failed command clears it at once.
export const TRACK_INTENT_MS = 10_000;

let until = 0;

export function noteTrackIntent(now = Date.now()): void {
  until = now + TRACK_INTENT_MS;
}

export function clearTrackIntent(): void {
  until = 0;
}

/// True once per request, while it is fresh.
export function takeTrackIntent(now = Date.now()): boolean {
  const fresh = now <= until;
  until = 0;
  return fresh;
}

/// Wraps a command that changes the track: notes the intent before it runs
/// and drops it when the command fails.
export function userTrackChange<T>(run: () => Promise<T>): Promise<T> {
  noteTrackIntent();
  // `run` is called synchronously so commands keep their call order.
  return Promise.resolve(run()).catch((error: unknown) => {
    clearTrackIntent();
    throw error;
  });
}
