/** How often the on-screen clock is refreshed: four times a second normally, once a second to save power. */
export function tickInterval(reduceMotion: boolean): number {
  return reduceMotion ? 1000 : 250;
}
