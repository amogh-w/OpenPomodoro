// Eight invented rune-like marks, each a few straight strokes in a 32 by 40 box. Each is centred in it: the middle of
// its bounding box is at 16, 20 (glyphs.test.ts checks). They are this project's own: they look like carved letters
// but are not any real alphabet and mean nothing.
export const GLYPHS: readonly string[] = [
  "M11.5 6 V34 M11.5 14 L20.5 23 M11.5 22 L20.5 31",
  "M16 5.5 L25 15.5 L16 25.5 L7 15.5 Z M16 25.5 V34.5",
  "M9 6.5 L23 22.5 M23 6.5 L9 22.5 M16 22.5 V33.5",
  "M9 8 L23 16 L9 24 L23 32",
  "M8 18.5 L16 6.5 L24 18.5 M16 6.5 V33.5",
  "M16 5.5 V34.5 M8 13.5 H24 M11 24.5 H21",
  "M16 8.5 L7 31.5 M16 8.5 L25 31.5 M10 23.5 H22",
  "M21 6 H10 L10 20 H22 M16 20 V34",
];

export const GLYPH_COUNT = GLYPHS.length;

export function glyphPath(index: number): string {
  return GLYPHS[((index % GLYPH_COUNT) + GLYPH_COUNT) % GLYPH_COUNT] ?? "";
}
