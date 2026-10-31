const byOrder = (a, b) =>
  (a.seriesIndex ?? Infinity) - (b.seriesIndex ?? Infinity) ||
  a.title.localeCompare(b.title, undefined, { numeric: true });

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
    .map(([, [name, group]]) => [name, group.sort(byOrder)]);
};

export const topGenres = (books, limit = 12) => {
  const counts = new Map();
  for (const book of books)
    for (const genre of book.genres ?? [])
      counts.set(genre, (counts.get(genre) ?? 0) + 1);
  return [...counts.entries()]
    .sort(([a, x], [b, y]) => y - x || a.localeCompare(b))
    .slice(0, limit)
    .map(([genre]) => genre);
};
