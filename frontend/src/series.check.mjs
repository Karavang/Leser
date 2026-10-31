import assert from "node:assert/strict";
import { seriesGroups } from "./series.js";

const book = (title, series) => ({ title, series });
const groups = seriesGroups([
  book("Книга 10", "Ведьмак"),
  book("Без серии", null),
  book("Книга 2", "ведьмак "),
  book("Азкабан", "Гарри Поттер"),
]);
assert.deepEqual(
  groups.map(([name, bs]) => [name, bs.map((b) => b.title)]),
  [
    ["Ведьмак", ["Книга 2", "Книга 10"]],
    ["Гарри Поттер", ["Азкабан"]],
    ["", ["Без серии"]],
  ],
);
