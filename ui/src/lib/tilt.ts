/// Pointer math for the sleeve's 3D tilt + glare (Track-style hover effect).
/// Pure so the geometry can be tested without a DOM.

export interface Tilt {
  /// rotateX degrees; cursor above center tilts the top away (positive).
  rx: number;
  /// rotateY degrees; cursor right of center tilts the right edge away.
  ry: number;
  /// Glare position in percentages, mirrored through the card center.
  gx: number;
  gy: number;
  /// Glare opacity 0..1: darkest at the center, full at edges/corners.
  glare: number;
}

export function tiltFromPointer(
  rect: { left: number; top: number; width: number; height: number },
  clientX: number,
  clientY: number,
  maxTilt: number,
): Tilt {
  if (rect.width <= 0 || rect.height <= 0) {
    return { rx: 0, ry: 0, gx: 50, gy: 50, glare: 0 };
  }
  const px = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  const py = Math.min(1, Math.max(0, (clientY - rect.top) / rect.height));
  const nx = px * 2 - 1;
  const ny = py * 2 - 1;
  /// The pivot follows the cursor direction and its magnitude grows linearly
  /// with radial distance, capped at maxTilt. Normalizing by the distance
  /// keeps the combined tilt at maxTilt even in the corners (independent axes
  /// would sum to maxTilt * sqrt(2) there and read as a jump).
  const d = Math.hypot(nx, ny);
  const r = Math.min(1, d);
  const mag = r * maxTilt;
  const ux = d === 0 ? 0 : nx / d;
  const uy = d === 0 ? 0 : ny / d;
  return {
    rx: -uy * mag || 0,
    ry: ux * mag || 0,
    gx: (1 - px) * 100,
    gy: (1 - py) * 100,
    glare: r,
  };
}
