/// Stylized wave (spec §10.2): a soft deterministic line derived from the
/// track id — deliberately not real peak data (that upgrade is deferred).

/// Pins both ends of the wave to its baseline: no amplitude over the first and
/// last `flat` share of the width, easing up to full over the next `ramp`
/// share. The floating player bar uses it so the wave meets its rounded
/// corners on the bar edge.
export interface WaveTaper {
  flat: number;
  ramp: number;
}

export function taperAt(x: number, taper?: WaveTaper): number {
  if (!taper) return 1;
  const edge = Math.min(x, 1 - x);
  if (edge <= taper.flat) return 0;
  const u = Math.min(1, (edge - taper.flat) / Math.max(taper.ramp, 1e-6));
  return u * u * (3 - 2 * u);
}

export function waveSamples(seed: number, count: number, taper?: WaveTaper): number[] {
  const p1 = (seed % 628) / 100;
  const p2 = (seed % 314) / 50;
  const p3 = (seed % 157) / 25;
  const out: number[] = [];
  for (let i = 0; i < count; i++) {
    const x = count <= 1 ? 0 : i / (count - 1);
    const v =
      Math.sin(x * 6.3 + p1) * 0.5 +
      Math.sin(x * 11.9 + p2) * 0.3 +
      Math.sin(x * 23.7 + p3) * 0.2;
    out.push(Math.min(1, Math.abs(v)) * taperAt(x, taper));
  }
  return out;
}

export function wavePath(
  seed: number,
  width: number,
  height: number,
  count = 96,
  taper?: WaveTaper,
): string {
  const samples = waveSamples(seed, count, taper);
  const step = count <= 1 ? 0 : width / (count - 1);
  return samples
    .map((v, i) => {
      const y = height - v * height * 0.9 - height * 0.05;
      return `${i === 0 ? "M" : "L"}${(i * step).toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
}

/// The area below the wave, closed along the bottom edge, normalized to the
/// unit box so it can be used as an SVG `clipPath` with objectBoundingBox
/// (the seek bar's glass cap). Same shape as `${wavePath(seed, 800, 14)}
/// L800,12.6 L0,12.6 Z` scaled by 1/800 x 1/12.6.
export function waveFillPath(seed: number, count = 96, taper?: WaveTaper): string {
  const samples = waveSamples(seed, count, taper);
  const points = samples.map((v, i) => {
    const x = count <= 1 ? 0 : i / (count - 1);
    const y = 13.3 / 12.6 - v;
    return `${i === 0 ? "M" : "L"}${x.toFixed(4)},${y.toFixed(4)}`;
  });
  return `${points.join(" ")} L1,1 L0,1 Z`;
}
