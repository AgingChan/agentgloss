export interface LineRange {
  start_line: number;
  end_line: number;
}

export function annotatedElements(root: ParentNode): HTMLElement[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      "[data-start-line][data-end-line]",
    ),
  );
}

export function selectionLineRange(
  root: HTMLElement,
  range: Range,
): LineRange | null {
  if (range.collapsed || !range.toString().trim()) {
    return null;
  }
  if (!root.contains(range.startContainer) && !root.contains(range.endContainer)) {
    return null;
  }
  const candidates = annotatedElements(root).filter((element) =>
    range.intersectsNode(element),
  );
  if (candidates.length === 0) {
    return null;
  }
  const leaves = candidates.filter(
    (element) =>
      !candidates.some(
        (other) => other !== element && element.contains(other),
      ),
  );
  let start = Number.POSITIVE_INFINITY;
  let end = 0;
  for (const element of leaves) {
    const elementStart = Number(element.dataset.startLine);
    const elementEnd = Number(element.dataset.endLine);
    if (!Number.isFinite(elementStart) || !Number.isFinite(elementEnd)) {
      continue;
    }
    start = Math.min(start, elementStart);
    end = Math.max(end, elementEnd);
  }
  if (!Number.isFinite(start) || end < start) {
    return null;
  }
  return { start_line: start, end_line: end };
}

export function itemLineRange(item: {
  anchor: { line_start: number; line_end: number };
}): LineRange {
  return {
    start_line: item.anchor.line_start,
    end_line: Math.max(item.anchor.line_start, item.anchor.line_end - 1),
  };
}

export function findTargetElement(
  root: HTMLElement,
  range: LineRange,
): HTMLElement | null {
  let best: HTMLElement | null = null;
  let bestWidth = Number.POSITIVE_INFINITY;
  for (const element of annotatedElements(root)) {
    const start = Number(element.dataset.startLine);
    const end = Number(element.dataset.endLine);
    if (end < range.start_line || start > range.end_line) {
      continue;
    }
    const width = end - start;
    if (width < bestWidth) {
      best = element;
      bestWidth = width;
    }
  }
  return best;
}
