export type Entry = { name: string; path: string };
export type IndexedPage = { image: Entry; index: number };
export type PageCell =
  | { kind: "page"; key: string; page: IndexedPage }
  | { kind: "spacer"; key: string };

export function matchesFind(name: string, find: string): boolean {
  return name.toLocaleLowerCase().includes(find.toLocaleLowerCase());
}

export function pagesForPrinting(images: Entry[], selected: Set<string> | null): IndexedPage[] {
  return images
    .map((image, index) => ({ image, index }))
    .filter(({ image }) => selected === null || selected.has(image.path));
}

export function pageCells(images: Entry[], find: string, layout: "two-page" | "grid", coverOnRight: boolean): PageCell[] {
  const pages = pagesForPrinting(images, null).filter(({ image }) => matchesFind(image.name, find));
  if (layout === "grid") {
    return pages.map((page) => ({ kind: "page", key: `page:${page.image.path}`, page }));
  }

  const cells: PageCell[] = [];
  let previousSpread = -1;
  let previousColumn = -1;
  for (const page of pages) {
    const position = page.index + (coverOnRight ? 1 : 0);
    const spread = Math.floor(position / 2);
    const column = position % 2;
    if (spread !== previousSpread) {
      if (previousSpread !== -1 && previousColumn === 0) {
        cells.push({ kind: "spacer", key: `gap:${previousSpread}:right` });
      }
      if (column === 1) cells.push({ kind: "spacer", key: `gap:${spread}:left` });
    }
    cells.push({ kind: "page", key: `page:${page.image.path}`, page });
    previousSpread = spread;
    previousColumn = column;
  }
  if (previousSpread !== -1 && previousColumn === 0) {
    cells.push({ kind: "spacer", key: `gap:${previousSpread}:right` });
  }
  return cells;
}
