/// Stylized wave (spec §10.2): a soft deterministic line derived from the
/// track id — deliberately not real peak data (that upgrade is deferred).

export function waveSamples(seed: number, count: number): number[] {
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
    out.push(Math.min(1, Math.abs(v)));
  }
  return out;
}

export function wavePath(seed: number, width: number, height: number, count = 96): string {
  const samples = waveSamples(seed, count);
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
export function waveFillPath(seed: number, count = 96): string {
  const samples = waveSamples(seed, count);
  const points = samples.map((v, i) => {
    const x = count <= 1 ? 0 : i / (count - 1);
    const y = 13.3 / 12.6 - v;
    return `${i === 0 ? "M" : "L"}${x.toFixed(4)},${y.toFixed(4)}`;
  });
  return `${points.join(" ")} L1,1 L0,1 Z`;
}
