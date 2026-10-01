/// Perf-session-only hooks registered by the mounted Tracks view: the S6
/// synthetic fixture and the interaction timers. Empty outside a perf session
/// and cleared when the view unmounts, so normal runs cannot reach the fixture.
export interface PerfHooks {
  loadSyntheticTracks?: (count: number) => void;
  toggleViewSort?: (key: "title" | "duration") => void;
  buildSyntheticQueue?: (count: number) => Promise<void>;
}

export const perfHooks: PerfHooks = {};
