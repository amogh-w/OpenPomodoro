// Eight invented rune-like marks, each a few straight strokes in a 32 by 40 box. They are this project's own: they
// look like carved letters but are not any real alphabet and mean nothing.
export const GLYPHS: readonly string[] = [
  "M16 6 V34 M16 14 L25 23 M16 22 L25 31",
  "M16 6 L25 16 L16 26 L7 16 Z M16 26 V35",
  "M9 8 L23 24 M23 8 L9 24 M16 24 V35",
  "M9 7 L23 15 L9 23 L23 31",
  "M8 20 L16 8 L24 20 M16 8 V35",
  "M16 6 V35 M8 14 H24 M11 25 H21",
  "M16 7 L7 30 M16 7 L25 30 M10 22 H22",
  "M22 7 H11 L11 21 H23 M17 21 V35",
];

export const GLYPH_COUNT = GLYPHS.length;

export function glyphPath(index: number): string {
  return GLYPHS[((index % GLYPH_COUNT) + GLYPH_COUNT) % GLYPH_COUNT] ?? "";
}
