// Проверка словарей: node src/i18n.check.mjs
// Ловит главное, что тут ломается, — забытый перевод и неверный выбор языка.
import assert from "node:assert/strict";
import { DE, EN, LANGS, PL, RU, UK, pick } from "./i18n.js";

const dicts = { ru: RU, uk: UK, pl: PL, en: EN, de: DE };

assert.deepEqual(
  LANGS.map(([code]) => code).sort(),
  Object.keys(dicts).sort(),
  "список языков в настройках разошёлся со словарями",
);

for (const [code, dict] of Object.entries(dicts)) {
  assert.deepEqual(
    Object.keys(dict).sort(),
    Object.keys(RU).sort(),
    `ключи ${code} разошлись с RU`,
  );
  for (const key of Object.keys(RU)) {
    assert.equal(
      typeof dict[key],
      typeof RU[key],
      `${code}.${key}: разный вид значения`,
    );
    assert.notEqual(dict[key], "", `${code}.${key}: пустой перевод`);
  }
}

assert.equal(pick(null, "ru-RU"), "ru");
assert.equal(pick(null, "uk"), "uk");
assert.equal(pick(null, "pl-PL"), "pl");
assert.equal(pick(null, "de-AT"), "de", "регион не мешает выбрать язык");
assert.equal(pick(null, "en-US"), "en");
assert.equal(pick(null, "fr"), "en", "чужой язык — английский");
assert.equal(pick(null, undefined), "en", "язык браузера неизвестен — английский");
assert.equal(pick("en", "ru-RU"), "en", "выбор читателя выше языка браузера");
assert.equal(pick("uk", "en-US"), "uk");
assert.equal(pick("xx", "de"), "de", "мусор в localStorage не выбирает язык");

console.log("i18n ok:", Object.keys(dicts).length, "языка,", Object.keys(RU).length, "ключей");
