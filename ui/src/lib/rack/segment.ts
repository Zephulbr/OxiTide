/// 7-segment readouts set in DSEG7 (Track 9.5c). In DSEG, "8" lights every
/// segment and "!" is a blank of the same width, so a readout is two layers:
/// the ghost (every cell fully drawn, faint) and the lit text on top.

const DIGIT = /[0-9-]/;

/// The lit layer: digits, minus and colons as-is; anything else blank.
export function segLit(value: string): string {
  return [...value].map((ch) => (DIGIT.test(ch) || ch === ":" ? ch : "!")).join("");
}

/// The ghost layer: every digit cell as "8", colons kept.
export function segGhost(value: string): string {
  return [...value].map((ch) => (ch === ":" ? ":" : "8")).join("");
}
