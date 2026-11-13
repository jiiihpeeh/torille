// Shared underline model for the spelling, grammar and style layers.
//
// The textarea overlay paints all three layers into one mirror, so the layers
// are partitioned into non-overlapping regions first. A region can belong to
// more than one layer (a misspelling that also has a style finding, for
// example); the CSS order decides which decoration wins.

export type UnderlineLayer = 'spelling' | 'grammar' | 'style';

export type Underline = {
  start: number;
  end: number;
  layer: UnderlineLayer;
  /** `error`, `warning` or `suggestion`; only meaningful for grammar. */
  severity?: string;
};

export type UnderlineRegion = {
  start: number;
  end: number;
  /** Stable CSS class names, sorted. Empty for plain text. */
  classes: string[];
};

/** The CSS class for one layer hit. */
export function underlineClass(underline: Underline): string {
  if (underline.layer === 'spelling') return 'misspell';
  if (underline.layer === 'style') return 'style-suggestion';
  switch (underline.severity) {
    case 'error': return 'grammar-error';
    case 'warning': return 'grammar-warning';
    default: return 'grammar-suggestion';
  }
}

/**
 * Split `[0, length)` into regions that carry the classes of every hit
 * covering them. Adjacent regions with the same classes are merged.
 *
 * Ranges are clamped to the text; a hit that starts and ends at the same
 * point is ignored.
 */
export function partitionUnderlines(
  length: number,
  underlines: Underline[],
): UnderlineRegion[] {
  if (length <= 0) return [];

  const boundaries = new Set<number>([0, length]);
  for (const underline of underlines) {
    const start = Math.max(0, Math.min(length, underline.start));
    const end = Math.max(0, Math.min(length, underline.end));
    if (end <= start) continue;
    boundaries.add(start);
    boundaries.add(end);
  }
  const sorted = [...boundaries].sort((a, b) => a - b);

  const regions: UnderlineRegion[] = [];
  for (let index = 0; index < sorted.length - 1; index++) {
    const start = sorted[index];
    const end = sorted[index + 1];
    if (end <= start) continue;
    const classes = new Set<string>();
    for (const underline of underlines) {
      if (underline.start <= start && end <= underline.end) {
        classes.add(underlineClass(underline));
      }
    }
    const classList = [...classes].sort();
    const previous = regions[regions.length - 1];
    if (previous && previous.end === start && sameClasses(previous.classes, classList)) {
      previous.end = end;
    } else {
      regions.push({ start, end, classes: classList });
    }
  }
  return regions;
}

function sameClasses(left: string[], right: string[]): boolean {
  if (left.length !== right.length) return false;
  return left.every((value, index) => value === right[index]);
}
