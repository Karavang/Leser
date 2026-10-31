import assert from "node:assert/strict";
import { seriesGroups, topGenres } from "./classify.js";

const book = (title, series, seriesIndex, genres = []) => ({
  title,
  series,
  seriesIndex,
  genres,
});
const groups = seriesGroups([
  book("Книга 10", "Ведьмак"),
  book("Без серии", null),
  book("Книга 2", "ведьмак "),
  book("Узник Азкабана", "Гарри Поттер", 3),
  book("Тайная комната", "Гарри Поттер", 2),
]);
assert.deepEqual(
  groups.map(([name, bs]) => [name, bs.map((b) => b.title)]),
  [
    ["Ведьмак", ["Книга 2", "Книга 10"]],
    ["Гарри Поттер", ["Тайная комната", "Узник Азкабана"]],
    ["", ["Без серии"]],
  ],
);

assert.deepEqual(
  topGenres(
    [
      book("a", null, null, ["роман", "фэнтези"]),
      book("b", null, null, ["фэнтези"]),
      book("c", null, null),
    ],
    5,
  ),
  ["фэнтези", "роман"],
);
