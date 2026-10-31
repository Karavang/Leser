const byTitle = (a, b) =>
  a.title.localeCompare(b.title, undefined, { numeric: true });

// ponytail: inside a series books go by title until series_index exists; then sort by it first.
export const seriesGroups = (books) => {
  const groups = new Map();
  for (const book of books) {
    const name = book.series?.trim() ?? "";
    const key = name.toLowerCase();
    if (!groups.has(key)) groups.set(key, [name, []]);
    groups.get(key)[1].push(book);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => (a === "") - (b === "") || a.localeCompare(b))
    .map(([, [name, group]]) => [name, group.sort(byTitle)]);
};
