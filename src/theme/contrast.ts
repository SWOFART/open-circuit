/** The relative luminance of a `#rrggbb` color (WCAG 2.2, section 1.4.3). */
export function relativeLuminance(color: string): number {
  if (!/^#[0-9a-f]{6}$/i.test(color)) {
    throw new Error(`${color} is not a #rrggbb color.`);
  }

  const [red, green, blue] = [1, 3, 5].map((start) => {
    const channel = Number.parseInt(color.slice(start, start + 2), 16) / 255;
    return channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];

  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

/** The contrast ratio of two `#rrggbb` colors, from 1 (the same) to 21 (black on white). */
export function contrastRatio(first: string, second: string): number {
  const [light, dark] = [
    relativeLuminance(first),
    relativeLuminance(second),
  ].sort((a, b) => b - a) as [number, number];

  return (light + 0.05) / (dark + 0.05);
}
