/// The newest desktop play. Every play `api.ts` sends claims one, and so does
/// a page as soon as its play is clicked (`lib/contextPlay.ts`), so a play
/// still waiting for its list knows when any other one came after it. Kept
/// apart from `contextPlay.ts`, which imports `api.ts`.

let latest = 0;

/// Claim the next play. The returned check turns false once a newer play is
/// claimed: a play still waiting for its list must not replace it.
export function claimPlay(): () => boolean {
  const id = ++latest;
  return () => id === latest;
}
