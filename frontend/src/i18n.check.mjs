// Проверка словаря: node src/i18n.check.mjs
// Ловит главное, что тут ломается, — забытый перевод и неверный выбор языка.
import assert from "node:assert/strict";
import { EN, RU, pick } from "./i18n.js";

assert.deepEqual(
  Object.keys(RU).sort(),
  Object.keys(EN).sort(),
  "ключи RU и EN разошлись",
);
for (const key of Object.keys(RU)) {
  assert.equal(typeof RU[key], typeof EN[key], `${key}: разный вид значения`);
}

assert.equal(pick(null, "ru-RU"), "ru");
assert.equal(pick(null, "ru"), "ru");
assert.equal(pick(null, "en-US"), "en");
assert.equal(pick(null, "de"), "en");
assert.equal(pick(null, undefined), "en", "язык браузера неизвестен — английский");
assert.equal(pick("en", "ru-RU"), "en", "выбор читателя выше языка браузера");
assert.equal(pick("ru", "en-US"), "ru");
assert.equal(pick("xx", "de"), "en", "мусор в localStorage не выбирает язык");

console.log("i18n ok");
