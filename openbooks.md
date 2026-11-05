# Открытые источники книг и аудиокниг

Проверено 01.11.2026 запросами `curl` с тем же User-Agent, что у сервера
(`leser/0.1 (+https://leser.karavang.com)`), по официальной документации,
условиям использования и исходному коду самих проектов. Здесь только
знание об источниках; что из этого делать и в каком порядке — раздел
«План», решение — в `TODO.md`.

Оговорка о сети: часть сайтов отвечает по-разному в зависимости от страны
сервера. Из сети, где проверялось, НЭБ отдаёт заглушку «Отключите VPN»,
Gallica и HathiTrust — проверку Cloudflare, REST API OAPEN/DOAB — 403,
сайт Верховной Рады не открылся вовсе. Что из этого блокировка по IP, а что
по стране, отсюда не видно.

**Статус на 05.11.2026:** подключены Wolne Lektury (текст и аудио),
Wikisource (язык по письменности запроса: ru, uk, be, bg, sr; en, de, pl, fr,
es, it, pt, cs, nl, sv, la; el, he, ar, fa, hy, ka, hi, bn, ta, th, zh, ja,
ko), TextGrid, ELTeC (15 репозиториев с метаданными: cs, de, en, fr, hu, it,
pl, pt, ro, sl, es, sr, uk) и DraCor (корпуса со свободной лицензией: ar, nl,
de, grc, no, pl, ro, la, ru, uk, yi) — оба в TEI, сервер собирает из него fb2;
Gutenberg качается с зеркала PGLAF, LibriVox открывается только после проверки
автора. Аудио из LibriVox и Wolne Lektury слушается по ссылке, без копии у
нас. Standard Ebooks ждёт договорённости, остальное из плана — пункт 7.

## Главный вывод

1. **Первым — Wolne Lektury.** Единственный найденный источник, где сразу
   есть всё, что нужно Leser: JSON API без ключа, прямые ссылки на epub/fb2
   и **аудиокниги mp3 по главам с длительностью** (та же модель, что
   LibriVox), лицензия указана у каждой книги отдельно, а язык — польский,
   один из пяти языков интерфейса. ~2,5 тыс. произведений, ~500 аудиокниг,
   вычитанный текст с обложками. Работа — одна новая функция в `sources.rs`
   на текст и обобщение структуры `LibriVox` на аудио.
2. **Вторым — Wikisource (ru, uk, pl, de) через ws-export.** Это главный
   открытый источник для русского и украинского: вычитанные тексты, поиск
   через MediaWiki API, готовый epub по одной ссылке. Подводные камни —
   страницы «список редакций» в выдаче и `robots.txt` у ws-export.
3. **До новых источников — две правовые правки в уже подключённых.**
   LibriVox сейчас открывается всем без проверки, а сам LibriVox пишет, что
   его записи — общественное достояние только в США: текст может быть под
   охраной в ЕС, Польше, Украине. Project Gutenberg по своим правилам
   требует от автоматических клиентов качать с зеркал, а не с
   `www.gutenberg.org`.
4. **Третьим — TextGrid «Digitale Bibliothek»** для немецкого: ~94 тыс.
   текстов (корпус zeno.org, который TextGrid получил по лицензии и раздаёт
   под CC BY 3.0), epub собирается по запросу.
5. **Standard Ebooks** — лучшее качество на английском, но каталог (OPDS)
   отдаётся только донорам Patrons Circle. Без договорённости подключать
   нечем.
6. **Русских и украинских аудиокниг на открытой лицензии практически нет**:
   LibriVox — 76 и 4 записи, у остальных права чтеца не отданы. Для ru/uk
   звук реалистичнее получать своей озвучкой (см. `marketknows.md`).
7. **Аналога Wolne Lektury для русского, украинского и белорусского нет**
   (проверено 05.11.2026, раздел «Русский, украинский, белорусский:
   глубже»). Лучший русский текст — 90-томник Толстого на tolstoy.ru, но
   раздавать его без разрешения музея запрещено. Открытые лицензии есть
   только у исследовательских корпусов (DraCor, ELTeC) в TEI. Для всех трёх
   языков основной источник — Wikisource; be добавляется тем же кодом.

## Сводная таблица

Объём — на дату проверки, «наши языки» — ru, uk, pl, de, en.

| источник | наши языки | объём | тип | API / выгрузка | форматы | лицензия / условия | подключить | оценка |
|---|---|---|---|---|---|---|---|---|
| **Wolne Lektury** | pl; de 56, uk 12 записей | ~2,5 тыс. произведений (7,7 тыс. с частями и стихами), ~500 аудиокниг | текст + аудио | JSON API без ключа, OAI-PMH | epub, fb2, mobi, pdf, txt; mp3, ogg | ОД; Licencja Wolnej Sztuki 1.3 или CC BY-SA у современных авторов и у аудио | легко | **1** |
| **Wikisource** | ru, uk, pl, de, en | 0,4–1,3 млн страниц на язык (страница ≠ книга) | текст | MediaWiki API, дампы; epub через ws-export | epub, pdf, mobi | ОД в США и в стране происхождения; переводы участников CC BY-SA 4.0 | средне | **2** |
| Project Gutenberg (есть) | en 62 тыс., de 2,5 тыс., pl 31, ru 5, uk 0 | 79,7 тыс. | текст (+1,1 тыс. аудио) | каталог CSV/RDF, OPDS, gutendex (сторонний) | epub, kindle, html, txt | ОД в США; товарный знак; роботам — только зеркала | есть | поправить |
| LibriVox (есть) | en; de ~850, ru 76, pl 31, uk 4 | ~21,9 тыс. | аудио | JSON API без ключа | mp3 | записи — ОД; тексты — ОД только в США | есть | поправить |
| **TextGrid Digitale Bibliothek** | de | ~94 тыс. текстов, 693 автора | текст | REST-поиск (XML) + сборка epub | TEI, epub | CC BY 3.0 на разметку, сами тексты — ОД | средне | **3** |
| Standard Ebooks | en | ~1,5 тыс. | текст | OPDS/RSS только для доноров; открыт Atom новинок | epub, kepub, azw3 | CC0 на их работу, тексты — ОД в США | после договорённости | 4 |
| Internet Archive | все | миллионы | текст, аудио | advancedsearch, scrape, metadata без ключа | pdf, djvu, epub из OCR, mp3 | права при загрузке не проверяются; CDL закрыта | технически легко, юридически нет | только как хост LibriVox |
| Open Library | все | — | каталог | search.json, ежемесячные дампы | ссылки на IA | API не для массовой выгрузки | — | метаданные |
| az.lib.ru | ru | — | текст | нет, только http | fb2.zip, html | лицензии нет | средне + http | не сейчас |
| ФЭБ | ru | — | текст | нет | html | некоммерческое с указанием источника | трудно | не сейчас |
| OAPEN / DOAB | en, de, … | 59,7 тыс. (OAPEN) | научные книги | OAI-PMH; REST отвечает 403 | pdf, реже epub | CC BY и др. | средне | потом |
| OpenStax / Open Textbook Library | en, pl (OpenStax) | 129 / 2 038 | учебники | CMS API / JSON | pdf / внешние сайты | 80 из 129 OpenStax — CC BY-NC-SA | трудно (pdf) | нет |
| StoryWeaver и др. детские | ru 124, pl 51, de 210, uk 0 | — | книжки-картинки | недокументированный JSON | pdf, epub | CC BY 4.0 | трудно (fixed layout) | нет |
| Faded Page | en; de 124 | 9 189 | текст | нет | epub, mobi, pdf | ОД в Канаде | — | нет |
| tolstoy.ru | ru | 90 томов Толстого | текст | нет, прямые ссылки | epub, fb2, mobi, pdf | «без официального разрешения запрещено» | после разрешения | потом |
| DraCor, ELTeC | ru, uk | 212 + 49 пьес, 50 романов uk | текст | JSON API, GitHub | TEI, txt | CC0 / CC BY 4.0 | трудно (TEI) | потом |

Отвергнутые с причинами — в отдельном разделе ниже; по ru, uk, be —
ещё и в разделе «Русский, украинский, белорусский: глубже».

## Wolne Lektury

Польская библиотека фонда Wolne Lektury, исходный код сайта открыт
(AGPL-3.0, [github.com/fnp/wolnelektury](https://github.com/fnp/wolnelektury)).

**Эндпоинты** (все проверены, ключ не нужен):

- Документированный API v1 ([wolnelektury.pl/api](https://wolnelektury.pl/api/)):
  `GET https://wolnelektury.pl/api/books/` — весь каталог одним списком
  (7 693 записи, 5 МБ); `…/api/parent_books/` — 2 539 произведений
  верхнего уровня, из них 493 с аудио; `…/api/audiobooks/` — 1 564;
  `…/api/books/{slug}/` — карточка со ссылками на файлы и `media`.
- API v2, которым пользуется мобильное приложение. На странице API
  не описан, маршруты видны в исходниках
  ([api/urls.py](https://github.com/fnp/wolnelektury/blob/master/src/api/urls.py),
  [catalogue/api/urls2.py](https://github.com/fnp/wolnelektury/blob/master/src/catalogue/api/urls2.py)):
  - `GET https://wolnelektury.pl/api/2/books/?search=lalka` →
    `{"member":[{"slug":"lalka","title":"Lalka","language":"pol","authors":[…],…}],"totalItems":6}`.
    Поиск только по названию (`search_fields = ['title']` в
    [views.py](https://github.com/fnp/wolnelektury/blob/master/src/catalogue/api/views.py)),
    пагинация `limit`/`offset`, фильтр `has_audio=true` (1 702 записи).
  - Поиск по автору в два шага: `GET …/api/2/search/?q=prus` →
    `{"author":[{"id":1462,"name":"Bolesław Prus",…}]}`, затем
    `GET …/api/2/books/?tag=1462` → 39 книг.
  - `GET …/api/2/books/{slug}/` — `epub`, `fb2`, `mobi`, `pdf`, `txt`,
    `xml`, `cover`, `language`, `translators`, `abstract`, `preview`,
    `has_mp3_file`, `has_sync_file`.
  - `GET …/api/2/books/{slug}/media/mp3/` →
    `[{"url":".../20-000-mil-podmorskiej-zeglugi_001_skala-uciekajaca.mp3","part_name":"Skała uciekająca","artist":"Mateusz Weber","director":"Dominik Lehman","duration":1081.0,"size":8676280},…]`
    — ровно то, что Leser сейчас берёт из LibriVox (`Section`: название,
    адрес, секунды).
  - `GET …/api/2/books/{slug}/sync/` — синхронизация текста со звуком
    (у проверенной книги пусто). Для будущей подсветки слов — интересно.
- OAI-PMH: `https://wolnelektury.pl/oaipmh/`. Лицензия одной книги —
  `?verb=GetRecord&metadataPrefix=oai_dc&identifier=oai:wolnelektury.pl:lalka`
  → `<dc:rights>Domena publiczna - Bolesław Prus zm. 1912</dc:rights>`
  за 0,07 с. Полный `ListRecords` — 14 МБ и больше двух минут.

**Хосты для `ALLOWED_HOSTS`:** `wolnelektury.pl` — и API, и файлы
(`/media/book/epub/…`, `/media/book/mp3/…`) отдаются оттуда напрямую,
без редиректов на CDN.

**Объём и языки** (по выгрузке OAI-PMH): pol 7 461, lit 85, ger 56,
eng 48, fre 28, ukr 12, heb 3. Права по полю `dc:rights`: общественное
достояние — ~5,8 тыс., Licencja Wolnej Sztuki 1.3 (Free Art License) —
~1,76 тыс. (современные поэты: Joanna Mueller, Justyna Bargielska…),
CC BY-SA 3.0/4.0 — ~100.

**Правовое.** [Zasady wykorzystania](https://wolnelektury.pl/info/zasady-wykorzystania/):
всё на сайте либо в ОД, либо под CC BY-SA или Licencja Wolnej Sztuki;
скачивать и раздавать можно. Но сноски, разметка мотивов и
**аудиокниги — их творческая работа** под свободной лицензией: при
распространении надо сохранить авторов, указать лицензию и изменения,
производное раздавать под той же лицензией. В ID3-тегах mp3 так и
записано: `TCOP: 2020 Fundacja Wolne Lektury. Licensed to the public under
http://artlibre.org/licence/lal/pl/`, чтец и режиссёр — в тегах и в API.

**Как ложится на Leser:**

- Текст: поиск → `Found { ext: "epub", download_url: epub }` → обычный
  `/import`. Расширение берётся из пути, `.epub` на месте.
- Аудио: `media/mp3` один в один заменяет `sections` LibriVox, фоновая
  загрузка по дорожкам в `audio.rs` подходит без изменений.
- Открытие всем: `legal.rs` на переводах ответит «нет» (Верн, язык `pl`,
  язык автора `fr`), хотя Wolne Lektury прямо пишут, что перевод в ОД.
  Основание лучше брать у источника — `dc:rights` из OAI-PMH — и класть
  в `public_basis`. Брать его должен сервер по адресу файла, а не клиент:
  `/import` принимает от клиента только `url` и `page_url`.

**Подводные камни:**

- v2 не документирован и может поменяться; v1 документирован, но
  поиска в нём нет — только полный список на 5 МБ.
- У книг, разбитых на тома, у родителя бывает только epub/mobi/pdf
  (у «Lalka» `fb2` и `txt` — `null`), у томов — все форматы.
- `preview: true` — ранний доступ для жертвователей. Таких книг сейчас
  нет (`/api/preview/` пуст), но фильтровать их надо.
- Поиск v2 — стандартный `SearchFilter` DRF по названию, то есть
  подстрока без учёта регистра: «lalka» находит и «Dziewczynka i lalka»,
  а «Prus» — «Wycieczka kolarza na Mazury Pruskie».

## Wikisource (Викитека, Вікіджерела, Wikiźródła)

**Объём** (`meta=siteinfo&siprop=statistics`, статьи — это главы, стихи
и словарные статьи, не книги): ru 624 817, uk 416 158, pl 1 342 702,
de 656 799, en 1 136 936.

**Эндпоинты** (проверены):

- Поиск: `GET https://ru.wikisource.org/w/api.php?action=query&list=search&srsearch=Капитанская дочка&format=json`
  → `totalhits: 681`, первым — «Капитанская дочка (Пушкин)».
- Отсеять страницы версий: `prop=pageprops|categories` — у uk
  «Кайдашева сім'я» `"pageprops":{"disambiguation":"","wikibase_item":"Q12107694"}`
  и категория «Сторінки версій».
- Epub: `GET https://ws-export.wmcloud.org/?format=epub-3&lang=de&page=Die_Verwandlung_(Franz_Kafka)&fonts=none`
  → 200, `application/epub+zip`, 64 КБ за 1,2 с; в epub `dc:creator`,
  `dc:language`, `dc:publisher`, `dc:date`, `dc:source`. Параметры
  (`page`, `format`, `lang`, `images`, `fonts`, `credits`) — из исходников
  [ExportController.php](https://gitlab.wikimedia.org/toolforge-repos/wsexport/-/blob/main/src/Controller/ExportController.php);
  тот же адрес Wikisource сам ставит в ссылку «Скачать EPUB» на каждой
  странице. Описание инструмента —
  [Wikisource:WS Export](https://wikisource.org/wiki/Wikisource:WS_Export).
  В проверке uk-книга собралась за 3,1 с, «Pan Tadeusz (wyd. 1921)» —
  за 3,5 с.
- Массово: дампы, например
  `https://dumps.wikimedia.org/ruwikisource/latest/ruwikisource-latest-pages-articles.xml.bz2`
  (отвечает 200).

**Хост для `ALLOWED_HOSTS`:** `ws-export.wmcloud.org` (отдаёт файл сразу,
без редиректа). К `*.wikisource.org` ходит только поиск, в список его
добавлять не нужно.

**Правовое.** Правило ru.wikisource
([Викитека:Авторские права](https://ru.wikisource.org/wiki/Викитека:Авторские_права)):
текст должен быть свободен и по законам США, и по законам страны
происхождения. То есть это ОД в США плюс РФ, а не в Польше или Украине.
Там же лежат переводы участников под CC BY-SA 4.0. ws-export пишет в
`dc:rights` «CC BY-SA 3.0» и GFDL — это лицензия сайта, а не статус текста.

**Правила для роботов:**
[Wikimedia User-Agent Policy](https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy)
требует описательный User-Agent с контактом (у Leser он есть);
[Robot policy](https://wikitech.wikimedia.org/wiki/Robot_policy) для
Action API без авторизации — один запрос одновременно и меньше 5 в секунду,
уважать `robots.txt` и `Retry-After`. **У ws-export `robots.txt` —
`User-agent: * / Disallow: /`.** Скачивание по нажатию пользователя — это
не обход, но предзагрузку и массовую конвертацию через ws-export делать
нельзя; для массового — дампы или своя сборка epub из API.

**Подводные камни:**

- Первым в поиске часто стоит страница версий: ru «Капитанская дочка
  (Пушкин)» даёт epub с заголовком «Список редакций:Капитанская дочка»
  и двумя изданиями внутри, de «Die Verwandlung» — epub на 19 КБ с
  неоднозначностью. Фильтр по `pageprops.disambiguation`, издания брать
  по ссылкам со страницы версий (`prop=links`).
- Для ru и uk ws-export вшивает FreeSerif — +7,7 МБ к каждому epub.
  `fonts=none` для них не помогает: неизвестное имя шрифта превращается
  в шрифт вики по умолчанию
  ([FontProvider.php](https://gitlab.wikimedia.org/toolforge-repos/wsexport/-/blob/main/src/FontProvider.php)).
  Для de шрифт не вшивается.
- `dc:creator` приходит как «автор Александр Сергеевич Пушкин» — с
  префиксом «автор». `legal::authors` отдаст в Wikidata строку с этим
  словом, и проверка не найдёт человека. Это безопасно, но книга не
  откроется сама.
- Таймаут клиента — 30 с (`sources::client`). На проверенных книгах
  хватало; на многотомниках не проверено.
- Качество разное: есть вычитанные по сканам страницы (ProofreadPage) и
  просто набранные. Можно ли отфильтровать по статусу вычитки через API —
  не проверено.

## Project Gutenberg (уже подключён — что поправить)

**Правила для роботов.** [Robot Access](https://www.gutenberg.org/policy/robot_access.html):
«The Project Gutenberg website is intended for human users only. Any
perceived use of automated tools… will result in a temporary or permanent
block of your IP address». Исключения — зеркала, `robot/harvest` с паузой
и каталог. [Terms of Use](https://www.gutenberg.org/policy/terms_of_use.html):
кто качает много (сноска: «more than ~100 per day») или автоматически —
с зеркал; OPDS-клиенты обязаны слать User-Agent с контактом и
запрашивать не больше, чем браузер; блокировки ставятся автоматически по
объёму трафика.

**Что проверено:**

- Каталог: `https://www.gutenberg.org/cache/epub/feeds/pg_catalog.csv`
  (21 МБ, обновляется раз в неделю), `rdf-files.tar.bz2` — ежедневно;
  страница [Offline Catalogs](https://www.gutenberg.org/ebooks/offline_catalogs.html)
  прямо просит строить базы на них, а не обходить сайт. Данные каталога —
  ОД.
- По CSV от 25.10.2026: 79 674 позиции, текстов 78 423, звука 1 114;
  языки текстов: en 62 081, fr 4 252, de 2 454, pl 31, ru 5, uk 0.
- Зеркало PGLAF со сгенерированными epub:
  `https://gutenberg.pglaf.org/cache/epub/2554/pg2554-images-3.epub` → 200.
  Список зеркал — [MIRRORS.ALL](https://www.gutenberg.org/dirs/MIRRORS.ALL).
- OPDS `https://www.gutenberg.org/ebooks/search.opds/?query=tolstoy` → 200;
  XML-ленты OPDS собираются закрыть в 2027 году, OPDS2 пока по запросу.
- gutendex.com — **сторонний** проект
  ([garethbjohnson/gutendex](https://github.com/garethbjohnson/gutendex),
  MIT), не Project Gutenberg. Живой: `count` 79 702.

**Правовое.** [Permission How-To](https://www.gutenberg.org/policy/permission.html):
большинство книг — ОД **в США**; «No permission is needed for non-commercial
use… you can freely redistribute any eBook… with or without the “Project
Gutenberg” trademark». За коммерческое использование названия — роялти по
лицензии. Terms of Use: вне США статус проверять самому.

**Что поменять:** ссылки на файлы переписывать с `www.gutenberg.org` на
`gutenberg.pglaf.org/cache/epub/{id}/pg{id}-images-3.epub` (одна строка в
`gutenberg()` плюс хост в `ALLOWED_HOSTS`). В README фразу «Gutenberg —
общественное достояние» уточнить: в США.

## LibriVox (уже подключён — что поправить)

**Объём по языкам** (коллекция `librivoxaudio` на archive.org,
`advancedsearch`): всего 21 921; deu 848 (+2 ger), rus 76, pol 31, ukr 4.
В самом API LibriVox фильтра по языку нет ([API info](https://librivox.org/api/info));
примерно с 26.09.2026 `limit` не больше 500.

**Правовое.** [Copyright, Public Domain & LibriVox](https://librivox.org/pages/public-domain/):
«LibriVox records only texts that are in the public domain (in the USA…),
and all our recordings are public domain (definitely in the USA, and maybe
in your country as well)… if you are outside the USA, we recommend that you
check the copyright status of the work in your country». Права чтеца
отданы, права на **текст** — только американские.

**Что поменять:** `audio.rs` создаёт книгу LibriVox сразу с `public = true`
и `legal_checked_at = now()`, а `legal::check` аудио пропускает
(`ext <> audio::EXT`). Нужна та же проверка автора по Wikidata, что у
текста; права чтеца при этом уже чистые.

## TextGrid Repository — «Digitale Bibliothek»

Корпус немецкой литературы от начала книгопечатания до начала XX века:
оцифровка издательства Editura (zeno.org), на которую TextGrid получил
лицензию и открыл.
[Страница проекта](https://textgridrep.org/project/TGPR-372fe6dc-57f2-6cd4-01b5-2c4bbefcfd3c):
литература — «rund 94 000 Texte von 693 Autor*innen» (много коротких
стихотворений), тексты в основном из учебных изданий.

**Эндпоинты** (проверены):

- `GET https://textgridlab.org/1.0/tgsearch-public/search?q=Effi%20Briest&filter=project.id:TGPR-372fe6dc-57f2-6cd4-01b5-2c4bbefcfd3c`
  → XML, `hits="7"`: издание `textgrid:n18s.0`
  (`text/tg.edition+tg.aggregation+xml`), произведение, сам TEI и биографии.
- `GET https://textgridlab.org/1.0/aggregator/epub/textgrid:n18s.0` →
  200, `application/epub+zip`, 279 КБ за 2,1 с, `dc:creator` «Fontane,
  Theodor», `dc:rights` «Creative Commons Attribution».

**Хост:** `textgridlab.org`.

**Правовое.** Там же, раздел «Lizenzierung»: на оцифровку и разметку у
Editura были смежные права, TextGrid получил лицензию с условием называть
Editura и раздаёт всё под CC BY 3.0 DE; «Die reinen Texte ohne Annotation
und Metadaten sind allgemeinfrei». Значит, при показе — атрибуция
TextGrid/Editura.

**Подводные камни:** `dc:title` у собранного epub — название сборника
(«Romane»), а не книги: заголовок надо брать из выдачи поиска, а
`store_book` берёт его из файла. В выдаче вперемешку издания, TEI-файлы и
биографии — фильтр по `format`. Путь `…/textgrid:n18s.0` кончается на
`.0`, `book_ext` его не узнает и по умолчанию поставит `epub` — это
правильно.

## Standard Ebooks

Вычитанные и заново свёрстанные книги на английском, ~1,5 тыс. (129
страниц каталога по 12; 1 545 репозиториев в
[github.com/standardebooks](https://github.com/standardebooks)).
Подвал сайта: «Content produced by or for Standard Ebooks L3C is dedicated
to the public domain via the CC0 1.0».

**Доступ** (проверено): `https://standardebooks.org/feeds/opds` → 401.
[Страница лент](https://standardebooks.org/feeds): «Access to our other
feeds is a benefit of membership in our Patrons Circle», открыты только
Atom/RSS новинок (`/feeds/atom/new-releases` → 200). Члены Patrons
Circle могут «parse and process the feeds to use our ebooks in your
personal software projects»; корпоративным спонсорам доступ дают на время
спонсорства. Поиск `https://standardebooks.org/ebooks?query=dostoevsky`
есть только в HTML.

**Скачивание:** `…/downloads/{file}.epub` без параметра отдаёт **HTML**
«Your Download Has Started!» с `<meta http-equiv="refresh">` — Leser
сохранил бы страницу как книгу. С `?source=download` → 200,
`application/epub+zip`. Хост — `standardebooks.org`. На каждой странице
спрятана ссылка-ловушка «Following this link will ban your IP for 24
hours»; [`robots.txt`](https://standardebooks.org/robots.txt) закрывает
загрузки от SEO-ботов и ИИ-агентов, но не от прочих.

**Правовое:** ОД **в США** — каталог включает книги, свободные только там:
«A Farewell to Arms» Хемингуэя (1929, ОД в США с 2025) есть в каталоге,
а Хемингуэй умер в 1961-м, и в ЕС книга под охраной до конца 2031 года.
На страницах будущих книг сайт пишет прямо: «will enter the U.S. public
domain in 10 years».

**Вывод:** нужно написать им о некоммерческой самохостной библиотеке и
попросить доступ к OPDS, либо делать каталог из GitHub (исходники, не
готовые epub). Скрейпить HTML-поиск — нет.

## Internet Archive и Open Library

**Эндпоинты** (проверены, ключ не нужен):
`https://archive.org/advancedsearch.php?q=…&fl[]=identifier&output=json`,
`https://archive.org/services/search/v1/scrape?q=…`,
`https://archive.org/metadata/{id}` — описание в
[документации](https://archive.org/developers/index-apis.html). Файлы
лежат на `ia8xxxxx.us.archive.org` — суффикс `.archive.org` в Leser уже
разрешён.

**Почему не как общий источник книг:**

- Права при загрузке никто не проверяет. Поиск «анна каренина» в текстах
  выдаёт, например, клавир балета Родиона Щедрина (родился в 1932-м —
  заведомо под охраной) и том «Библиотеки всемирной литературы» 1976 года
  с советскими предисловием и комментариями, у которых свой срок охраны.
- Книги библиотечной выдачи (коллекции `inlibrary`, `printdisabled`)
  помечены `"access-restricted-item": "true"`, файлы — `"private": "true"`,
  скачивание даёт 302 на вход. Фильтровать по этим полям обязательно.
- Epub там — результат OCR (у тома БВЛ 21 МБ), не вычитанный текст.
- Условия использования ограничивают использование «noninfringing or fair
  use» ([пост IA об условиях, 2014](https://blog.archive.org/?p=9040)).
  Текущую страницу условий проверить не удалось: `archive.org/about/terms`
  без JavaScript не отдаёт текст.

**Open Library** — каталог поверх IA. `GET https://openlibrary.org/search.json?q=Толстой&ebook_access=public`
→ 5 190 «произведений», но поле `ia` смешивает издания на разных языках.
[Developer API](https://openlibrary.org/developers/api): 1 запрос/с без
User-Agent с контактом, 3 запроса/с с ним; «Please do not use our APIs to
bulk download metadata» — для этого ежемесячные дампы. Годится разве что
для обогащения метаданных (это уже делает `enrich`).

## Русские источники

- **az.lib.ru** («Классика» Мошкова): у текстов есть `*.fb2.zip`, но
  сайт только по http (на 443 соединение отклоняется), а Leser принимает
  только https. Лицензии нет; тексты набраны волонтёрами с разных изданий,
  в том числе советских переводов. Технически возможно (zip распаковать,
  разрешить http для одного хоста), юридически — только с проверкой
  `legal.rs`. Не сейчас.
- **ФЭБ** (feb-web.ru): «Участники проекта не возражают против
  некоммерческого использования любых материалов ФЭБ… При перепечатке
  необходимо сослаться» — условия хорошие, но тексты только в HTML по
  страницам академических изданий, `robots.txt` закрывает `/download/`.
  С 05.11.2026 отвечает и по https (`https://feb-web.ru/feb/tolstoy/default.asp`
  → 200). Без своей сборки epub не подключить.
- **ru.wikisource** — см. выше; для русского это основной вариант.
- **НЭБ** (rusneb.ru): из сети проверки — заглушка «Отключите VPN» (403).
  Для сервера вне России недоступна; контент — сканы.

## Украинские источники

- **uk.wikisource** — 416 тыс. статей, см. выше. Основной вариант.
- **Чтиво** (chtyvo.org.ua) **закрыто**: на главной «Після більш ніж 20
  років безперервної роботи ми ухвалили рішення завершити роботу
  chtyvo.org.ua… Сайт більше не буде активно підтримуватися».
- **УкрЛіб** (ukrlib.com.ua): «При використанні матеріалів сайту,
  посилання на УкрЛіб обов'язкове» — лицензии нет, рядом современные
  авторы, скачивание — `printitzip.php` (zip с текстом в windows-1251),
  API нет.
- **«Культура України»** (elib.nlu.org.ua, Парламентская библиотека):
  14 363 документа, сканы.
- **«Е-бібліотека „Україна“»** однозначно опознать не удалось; поиск
  выдаёт «Культуру України» и «Україніку» (сканы). Не проверено.
- В Wolne Lektury 12 записей на украинском, в LibriVox 4, в Gutenberg 0.

## Русский, украинский, белорусский: глубже

Проверено 05.11.2026 тем же `curl` и User-Agent. Вопрос — есть ли для
ru, uk, be аналог Wolne Lektury: вычитанная классика, открытая лицензия,
прямые epub/fb2, API или выгрузка, лучше со звуком.

**Вывод: такого источника нет ни для одного из трёх языков.** Всё, что
нашлось, — либо качественный текст без права раздавать, либо открытая
лицензия без готовых книг.

1. **Основным остаётся Wikisource** (ru, uk, а теперь и be — тот же код
   с `lang=be`). Это единственный источник с вычиткой по сканам, API и
   готовым epub.
2. **tolstoy.ru** — лучший по качеству русский текст: 90-томник Толстого,
   вычитанный в три круга, с прямыми ссылками на epub и fb2. Но в подвале
   сайта — «Использование любых находящихся на сайте материалов без
   официального разрешения запрещено», а в файлы вшиты советские
   комментарии под охраной. Только с письменного разрешения музея.
3. **DraCor** (rus 212 пьес, ukr 49) и **ELTeC-ukr** (50 романов) —
   открытые лицензии, JSON API или GitHub, но TEI вместо epub и только
   драма или выборка романов. Потом, если будет своя сборка epub из TEI.
4. **Звука с отданными правами чтеца для ru, uk, be, кроме LibriVox, нет**
   (ru 76, uk 4, be 0 — `advancedsearch` по коллекции `librivoxaudio`).

| язык | источник | объём | тип | API / выгрузка | форматы | лицензия / условия | оценка |
|---|---|---|---|---|---|---|---|
| ru, uk, be | **Wikisource** | ru 625 тыс. статей, uk 417 тыс., be 31,5 тыс. | текст | MediaWiki API, дампы, ws-export | epub (со шрифтом +7,7 МБ) | ОД в США и в стране проекта | **подключать** (be — к п. 4 плана) |
| ru | tolstoy.ru | 91 том + отдельные произведения | текст | нет, прямые ссылки `/upload/iblock/…` | epub, fb2, mobi, pdf, docx | «без официального разрешения запрещено»; комментарии Н. К. Гудзия († 1965) | потом, с разрешения |
| ru, uk | DraCor (RusDraCor, UDraCor) | 212 и 49 пьес | текст (драма) | JSON API без ключа, GitHub | TEI, txt | CC0 | потом |
| uk | ELTeC-ukr | 50 романов 1841–1920 | текст | GitHub | TEI | разметка CC BY 4.0, тексты «в ОД», но есть автор † 1961 | потом |
| ru | ELTeC-rus | 20 файлов, не выпущен | текст | GitHub | TEI | не указана | нет |
| ru | Wikilivres.ru | 13 305 статей | текст | `api.php` отвечает 429 | html | ОД «или под свободной лицензией», рядом современные авторы | нет |
| ru | imwerden.de | 19 210 публикаций | сканы | OPDS `https://imwerden.de/opds/` | pdf | лицензии нет, книги 1988–2025 годов | нет |
| be | Беларуская Палічка (knihi.com) | — | текст, аудио | нет | epub, zip, mp3 | «спасылка… абавязковая», современные авторы | нет |
| be | Kamunikat.org | — | сканы, аудио | нет | pdf, epub, mobi | лицензии нет, современные издания | нет |
| uk | «Слухай» (sluhay.com.ua) | сотни записей (теги: поэзия 165, проза 61, ЗНО 61) | аудио | нет, ссылок на mp3 в HTML нет | поток | лицензии нет | нет |

### Wikisource для ru, uk, be — что добавилось

**Вычитка по сканам** (`meta=proofreadinfo` → категории уровней качества,
затем `prop=categoryinfo`; числа — страницы сканов, не книги):

| вики | вычитано | проверено | не вычитано | индексов (книг-сканов) |
|---|---|---|---|---|
| ru | 127 382 | 15 246 | 132 952 | 2 894 |
| uk | 67 771 | 67 299 | 4 902 | 5 234 |
| be | 18 532 | 1 632 | 56 972 | 1 878 |
| pl (для сравнения) | 216 532 | 424 618 | 344 683 | — |

Имена категорий у каждой вики свои («Вычитана»/«Вичитані
сторінки»/«Вычытаная»), брать их надо из `meta=proofreadinfo`, а не
зашивать. Уровень качества есть только у страниц `Страница:` (ns 104
у ru и be, 250 у uk); у произведения в основном пространстве уровня нет.
Отфильтровать «вычитанные книги» одним запросом нельзя — только через
индекс, из которого произведение собрано.

**uk.wikisource — 417 тыс. статей, но это в основном словари.** В
случайной выборке из 15 статей 9 — словарные статьи («Словник української
мови (1927)/дерен»), ещё 3 — заготовки. Слов во всём тексте 32 млн против
805 млн у ru (`cirrussearch-article-words` из `siprop=statistics`). Для
поиска книг на uk нужно отсекать словари по префиксу заголовка.

**be.wikisource (Вікікрыніцы).** 31 500 статей, 17 млн слов. Поиск
`srsearch=Новая зямля Колас` → 139, первым — «Новая зямля (Колас)» с
категорией «Спісы рэдакцый» (страница версий, как у ru и uk). ws-export:
`https://ws-export.wmcloud.org/?format=epub-3&lang=be&page=Новая_зямля_(1923)&fonts=none`
→ 200, `application/epub+zip`, 4,6 МБ за 3,8 с, и из 9 МБ распакованного
7,7 МБ — тот же FreeSerif, что у ru и uk. Правило
[Вікікрыніцы:Аўтарскае права](https://be.wikisource.org/wiki/Вікікрыніцы:Аўтарскае_права):
тексты должны быть свободны «паводле законаў ЗША… і паводле законаў
Рэспублікі Беларусь». Пример ловушки: Якуб Колас умер в 1956 году —
в ЕС «Новая зямля» под охраной до конца 2026 года, на be.wikisource она
лежит законно. `legal.rs` с правилом «70 лет» до 01.01.2027 её не откроет —
это правильно.

### tolstoy.ru — «Весь Толстой в один клик»

Государственный музей Л. Н. Толстого, «Ясная Поляна» и ABBYY;
[страница проекта](https://tolstoy.ru/projects/tolstoy-in-one-click/):
больше трёх тысяч волонтёров вычитали 46 тыс. страниц 90-томного
собрания (1928–1958) в три круга, тома выложены в pdf, fb2, epub.

**Проверено:**

- Список томов — `https://tolstoy.ru/creativity/90-volume-collection-of-the-works/?page=N`,
  карточки `…/1000/` … — всего 91 (90 томов и указатель). На карточке тома
  прямые ссылки: `/upload/iblock/c55/c55f44c627bac83e2119180085be32e2.epub`,
  `….fb2`, `….mobi`, `….pdf`.
- Отдельные произведения — `https://tolstoy.ru/creativity/fiction/1071/`
  («Война и мир»): `/upload/iblock/dac/voina-i-mir.epub`, `.fb2`, `.mobi`,
  `.pdf`, `.docx`.
- `GET https://tolstoy.ru/upload/iblock/144/anna-karenina.fb2` → 200,
  `application/download`, 6 МБ, валидный FictionBook 2.0 с `lang` `ru`,
  жанром `prose_rus_classic` и обложкой.
- API, OPDS и поиска по каталогу нет (`/search/` закрыт в `robots.txt`).

**Почему не подключать сейчас:**

- Подвал каждой страницы: «© 2026—2013 Tolstoy.ru. Все права защищены.
  Использование любых находящихся на сайте материалов без официального
  разрешения запрещено». Лицензии нет ни на сайте, ни в fb2.
- В fb2 «Анны Карениной» вместе с романом — текстологический комментарий
  за подписью Н. К. Гудзия, а он умер в 1965 году. Комментарий под охраной
  в ЕС и в России до конца 2035 года. Текст Толстого свободен, файл
  целиком — нет.
- Сайт — государственный российский музей. Ограничение из `TODO.md`
  («ничего российского») сформулировано про платежи и TTS, но если
  понимать его шире — это тоже против.

**Что можно:** написать музею (`info@tolstoy.ru` указан в самих fb2) и
попросить разрешения раздавать файлы в некоммерческой библиотеке.
Комментарии всё равно пришлось бы вырезать.

### DraCor — корпуса пьес

[dracor.org](https://dracor.org/) — проект корпусов драмы, данные на
GitHub. `GET https://dracor.org/api/v1/corpora?include=metrics` (ответ
сжат, нужен `--compressed`): rus «Russian Drama Corpus» — 212 пьес, CC0;
u «Ukrainian Drama Corpus» — 49, CC0.

- `GET https://dracor.org/api/v1/corpora/rus` — список с `authors`,
  `wikidataId`, `yearNormalized`, `source` (у «Ревизора» — ilibrary.ru).
- `GET …/corpora/rus/plays/gogol-revizor/tei` → 200, XML 77 КБ;
  `…/txt` → 200, текст 68 КБ. Epub нет.
- Годы — 1747–1947 (rus), 1819–1929 (u). Есть Булгаков († 1940, ОД в ЕС
  с 2011), Бабель († 1940), Афиногенов († 1941); в u — Микола Куліш,
  расстрелянный и посмертно реабилитированный — тот случай, когда по
  украинскому закону 1993 года срок считается от реабилитации (см.
  «Правовые ловушки», п. 3). CC0 у DraCor — на их работу, статус пьесы
  надо проверять, как у всех.

Оценка: потом. Только драма, а для Leser нужна своя сборка epub из TEI.

### ELTeC — European Literary Text Collection

Корпуса романов COST Action «Distant Reading»,
[github.com/COST-ELTeC](https://github.com/COST-ELTeC).

- **ELTeC-ukr**: 50 романов 1841–1920 (Квітка-Основ'яненко, Куліш,
  Вовчок, Франко, Кобилянська…), TEI, метаданные —
  `https://raw.githubusercontent.com/COST-ELTeC/ELTeC-ukr/HEAD/ELTeC-ukr_metadata.tsv`.
  README: «All texts included in this collection are in the public domain.
  The textual markup is provided with a Creative Commons Attribution
  International 4.0 licence». Но в метаданных есть Михайло Яцків († 1961) —
  в ЕС и в Украине под охраной до конца 2031 года.
  «Public domain» тут — по меркам проекта, не по закону страны сервера.
- **ELTeC-rus**: 20 файлов в `level0`, README из одного заголовка, релиза
  нет. Не годится.

Оценка ELTeC-ukr: потом — мало, и тоже TEI вместо epub.

### Проверено и отвергнуто

- **rvb.ru** (Русская виртуальная библиотека) — академические собрания с
  комментариями, только HTML; лицензии нет, в подвале «© Электронная
  публикация — РВБ, 2026».
- **ilibrary.ru** — только HTML, «©1996—2026 Алексей Комаров. Подборка
  произведений, оформление, программирование»; лицензии нет.
- **lib.ru** (основная библиотека Мошкова) теперь отвечает по https, но
  раздел «Русская классика» `/LITRA/` смешивает классику с советскими и
  современными авторами, тексты в koi8-r, лицензии нет, `Crawl-delay: 3`.
  az.lib.ru по-прежнему только http (443 — connection refused).
- **imwerden.de** — 19 210 публикаций и настоящий OPDS
  (`https://imwerden.de/opds/new.php`), но это pdf-сканы, в «новых» —
  книги 1988–2025 годов (переводы Камю, «Новая газета»); лицензии нет.
- **Wikilivres.ru** — вики «для произведений… которые по каким-либо
  причинам не могут быть опубликованы в других вики-проектах», в списке
  авторов — современные (Айги, Аверинцев). `api.php` из сети проверки
  отвечает 429 на первый же запрос.
- **ЛитРес** — бесплатные книги ОД есть, но это российская компания с
  ценами в рублях (ограничение `TODO.md`), партнёрский API — по договору,
  `robots.txt` закрывает `/download_book/`.
- **Электронная библиотека ИРЛИ** (lib.pushkinskijdom.ru) — научные
  сборники, только http.
- **«Старое радио»** (staroeradio.ru) — 36 510 советских радиозаписей, прав
  на раздачу нет, права исполнителей не отданы.
- **ukrclassic.com.ua** — школьная библиотека, скачивание docx/fb2/pdf, но
  «Використання матеріалів дозволено лише при розміщенні посилання на
  сайт», рядом современные авторы.
- **chytanka.com.ua** — детские книги-сканы: «Усі матеріали на нашому сайті
  розміщено виключно з метою ознайомлення».
- **izbornyk.org.ua** — источники по истории IX–XVIII веков, HTML, только
  http.
- **diasporiana.org.ua** — 26 987 pdf-сканов изданий эмиграции, в том
  числе XX века; лицензии нет.
- **javalibre.com.ua** — 403 от nginx на любой запрос с User-Agent Leser;
  условия прочитать не удалось.
- **«Слухай»** (sluhay.com.ua) — аудиокниги на украинском, читают актёры и
  волонтёры, сделано на средства общественного бюджета Киева
  ([texty.org.ua](https://texty.org.ua/fragments/90211/Zapushheno_pershu_onlajnbiblioteku_audioknyg_ukrajinskoju-90211/));
  лицензии нет, mp3 в HTML не отдаются.
- **Суспільне** (suspilne.media) — 403; условия архива радио не проверены.
- **Беларуская Палічка** (knihi.com; knihi.by не резолвится) — epub и
  mp3, но «пры выкарыстаньні матэрыялаў спасылка на Беларускую Палічку
  абавязковая» и современные авторы с чтецами.
- **Kamunikat.org** — в основном современные издания и периодика в pdf;
  в `robots.txt` — условия Content-Signal для сбора контента.
- Пиратские библиотеки (Флибуста, Coollib, e-reading, rulit) — отвечают,
  но не рассматриваются.

### Другие языки (кратко, без проверки источников)

Задача сменилась до того, как дошли до источников. Что успели посчитать:

- **Project Gutenberg**, тексты по языкам (`pg_catalog.csv`, 05.11.2026):
  fr 4 252, fi 3 690, nl 1 122, it 1 111, es 914, pt 656, hu 655, zh 443,
  sv 257, el 221, eo 145, la 144, ca 97, da 85, tl 60, no 23, ja 21, cs 12,
  bg 6, he 6, ro 5, sl 5, sr 4, et 1, ar 1, fa 1, ko 1; be 0.
- **LibriVox** (`advancedsearch`, коллекция `librivoxaudio`; поле `language`
  заполнено разными кодами, «fre» и «fra» — разные записи): spa 565,
  fre 275 + fra 40, nld 210, por 107, ita 89, jpn 37, ell 34, zho 25,
  heb 24, fin 16, lat 16, swe 13, cat 11, dan 10, epo 10, bul 4, tgl 4,
  ara 3, kor 3, ron 3, hin 2, hun 2, tam 2; est, lit, slk, slv, tur, vie,
  bel, kat, hye — 0.
- **ELTeC** есть для cze, deu, eng, fra, gre, hrv, hun, ita, lav, lit,
  nor, pol, por, rom, slv, spa, srp, swe, geo и др.; **DraCor** с CC0 —
  ger, dutch, pol, ro, yi, ar и др.
- Не проверены: Aozora Bunko, Projekt Runeberg, Liber Liber, Biblioteca
  Virtual Miguel de Cervantes, DBNL, MEK, Chitanka, Project Ben-Yehuda,
  Ganjoor и остальные кандидаты по другим языкам.

## Немецкие источники, кроме TextGrid

- **de.wikisource** — 657 тыс. статей, сильная вычитка по сканам, ws-export
  без вшитого шрифта.
- **Project Gutenberg** — 2,5 тыс. книг на немецком (уже есть).
- **LibriVox** — ~850 немецких записей (уже есть).
- Projekt Gutenberg-DE, Zeno.org, DTA — в отвергнутых.

## Научные, учебные и детские

- **OAPEN** (`https://library.oapen.org/oai/request`) — OAI-PMH работает,
  59 665 записей; на первой странице выдачи en 56, ger 15. REST
  (`/rest/search`, описан у
  [OAPEN](https://oapen.org/resources/15635975-metadata)) из сети
  проверки отвечает «You address is not allowed to access this API», так же
  у **DOAB** (`directory.doabooks.org/rest/search`). Монографии в pdf под
  CC BY и др. Для художественной библиотеки — потом.
- **OpenStax** (`https://openstax.org/apps/cms/api/v2/pages/?type=books.Book&fields=title,license_name,high_resolution_pdf_url`):
  129 книг, из них 80 под CC BY-NC-SA, 38 под CC BY; есть польские
  («Fizyka dla szkół wyższych», «Psychologia»). Только pdf с формулами —
  перевёрстка в Leser их не вытянет.
- **Open Textbook Library** (`https://open.umn.edu/opentextbooks/textbooks.json`):
  2 038 учебников на английском, ссылки ведут на десятки внешних сайтов,
  в `robots.txt` `Crawl-delay: 120`.
- **StoryWeaver** (`https://storyweaver.org.in/api/v1/books-search?languages[]=Russian`,
  не документирован): ru 124, pl 51, de 210, uk 0; CC BY 4.0
  ([About](https://storyweaver.org.in/about)). Книжки-картинки — fixed
  layout, читалке Leser не по профилю. Нужен ли вход для скачивания — не
  проверено.

## Аудио: что есть кроме LibriVox

| источник | языки | лицензия | вывод |
|---|---|---|---|
| Wolne Lektury | pl | Licencja Wolnej Sztuki 1.3 / CC BY-SA, чтец и режиссёр указаны | **подключать** |
| Project Gutenberg | en (1 049), de 25, ru 4 | как у PG | мало, потом |
| Internet Archive | все | не проверяется | нет |
| Vorleser.net | de | [AGB](https://www.vorleser.net/agb) § 3–4: только для частного некоммерческого использования; «öffentliche Wiedergabe… im… Internet» — только с письменного разрешения | нет |
| Wikimedia Commons | pl 106, de 57, ru 4, uk 1 файл в «Audiobooks in …» | CC | мало |

Для русского и украинского открытых аудиокниг с отданными правами чтеца,
кроме LibriVox, не найдено.

## Правовые ловушки

1. **ОД в США ≠ ОД в ЕС, Польше, Украине, России.** В США с 01.01.2026
   свободны книги, изданные до 1931 года
   ([Duke, Public Domain Day 2026](https://web.law.duke.edu/cspd/publicdomainday/2026/)),
   независимо от года смерти автора. В ЕС — 70 лет после смерти
   ([Директива 2006/116/EC, ст. 1](https://www.legislation.gov.uk/eudr/2006/116/article/1)).
   Хемингуэй в Standard Ebooks и в Gutenberg («A farewell to arms» #75201,
   «The Sun Also Rises» #67138 — через gutendex) — ОД в США, под охраной
   в ЕС до 2032 года. Leser сейчас сам такую книгу не откроет (Wikidata:
   † 1961), но импортировать её даёт любому. Так же устроены Faded Page (ОД в Канаде) и Project Gutenberg
   Australia (ОД в Австралии).
2. **Россия — свои удлинения.** По правилу ru.wikisource на основе
   ст. 1281 ГК РФ и ст. 6 № 231-ФЗ: +4 года для участников и тружеников
   войны, при посмертной реабилитации срок считается с даты реабилитации,
   при посмертной публикации — с даты публикации. `legal.rs` считает
   свободными в 2026 году всех умерших до 1955-го включительно: для
   российских авторов, воевавших и умерших в 1952–1955, и для
   реабилитированных посмертно это неверно — если сервер работает по
   российскому праву.
3. **Украина.** В законе 1993 года (ст. 28) срок для посмертно
   реабилитированных — 70 лет после реабилитации; как это в законе 2022
   года, проверить не удалось (zakon.rada.gov.ua не открылся). Не проверено.
4. **Переводы.** У перевода свой срок — по переводчику. `legal.rs`
   поступает осторожно и переводы не открывает вовсе. Wolne Lektury и
   LibriVox указывают статус перевода прямо — это основание можно брать у
   источника.
5. **Издания поверх ОД.** Предисловия и комментарии в советских изданиях
   (том БВЛ на IA) охраняются отдельно. В ЕС впервые опубликованное после
   истечения срока охраняется 25 лет
   ([ст. 4](https://www.legislation.gov.uk/eudr/2006/116/article/4)),
   научные и критические издания страны могут охранять до 30 лет
   ([ст. 5](https://www.legislation.gov.uk/eudr/2006/116/article/5)).
6. **Права чтеца.** Исполнение охраняется 50 лет с исполнения, а если
   запись опубликована — от публикации, для фонограмм после поправки
   2011/77/EU до 70 лет
   ([ст. 3](https://www.legislation.gov.uk/eudr/2006/116/article/3)).
   Поэтому аудио открывается только там, где чтец права отдал: LibriVox
   (посвящение в ОД) и Wolne Lektury (свободная лицензия). Но у LibriVox
   права на сам текст — только американские (см. выше).
7. **CC BY, CC BY-SA, Free Art License.** Указывать автора, лицензию и
   изменения ([CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/),
   [Licence Art Libre](https://artlibre.org/licence/lal/en/)); производное —
   под той же лицензией. Для Leser это значит: хранить файл как есть
   (страница атрибуции внутри), в карточке показывать источник и лицензию.
   Перевёрстка pdf и `rda → fb2` — уже переработка. **Озвучка TTS текста
   Wolne Lektury с их сносками — производное под BY-SA/FAL**: звук придётся
   раздавать под той же лицензией. Это касается плана платной озвучки из
   `marketknows.md`. И CC BY-NC (большинство OpenStax) запрещает
   коммерческое использование.
8. **Товарный знак Project Gutenberg.** Некоммерческое распространение
   свободно, с названием или без; коммерческое использование названия —
   за роялти. Если у Leser появится платная подписка, у книг из PG
   заголовок с товарным знаком придётся убирать.
9. **Права на базу данных и запреты роботов.** Zeno.org прямо ссылается
   на право изготовителя базы данных (§§ 87a ff. UrhG) и запрещает роботов;
   Projekt Gutenberg-DE разрешает только частное использование. Это
   запреты по договору и по базе данных, а не по авторскому праву на
   текст, но нарушать их не стоит.

**Что это значит для автоматического `public`:**

- Проверку по Wikidata оставить по умолчанию для всех источников, включая
  Wikisource и Standard Ebooks: их ОД — американское.
- Добавить второе основание — «лицензия источника»: сервер сам получает
  её у Wolne Lektury (`dc:rights`) или TextGrid (CC BY 3.0) и пишет в
  `public_basis` строку вида «Wolne Lektury: Domena publiczna — Bolesław
  Prus zm. 1912» или «Licencja Wolnej Sztuki 1.3». От клиента основание
  не принимать.
- LibriVox: открывать только после проверки автора текста, как у книг.
- Для `lang = ru` при желании добавить запас в 4 года (участники войны)
  или открывать без оговорок только умерших не позже 1940 года — так
  граница проведена в правиле ru.wikisource. Реабилитацию по Wikidata
  надёжно не увидеть — не проверено.
- В комментарии `TERM` в `legal.rs` записано «Poland, Ukraine and the
  rest of the EU». Это верно для страны сервера, а не для читателей из
  других стран.

## План подключения

Объём — в терминах текущего кода.

1. **Правовые правки (полдня).**
   - `audio.rs`: LibriVox создавать с `public = false` и запускать
     проверку автора; `legal::check` научить аудио (сейчас его отсекает
     `ext <> audio::EXT`). Тест на книгу с автором, умершим меньше 70 лет
     назад.
   - `sources.rs`: ссылки Gutenberg переписывать на
     `gutenberg.pglaf.org/cache/epub/{id}/pg{id}-images-3.epub`, хост — в
     `ALLOWED_HOSTS`, строка в тесте `only_https_and_only_known_hosts`.
   - README: «Gutenberg — ОД в США», «LibriVox — записи свободны, текст
     проверяется».
2. **Wolne Lektury, текст (1 день).** В `sources.rs` функция
   `wolnelektury(client, q)`: `api/2/books/?search=` плюс
   `api/2/search/?q=` → `?tag=` для авторов, отсев `preview`, результат —
   `Found { source: "Wolne Lektury", ext: "epub" }`; третьей веткой в
   `tokio::join!` внутри `search`. Хост `wolnelektury.pl`. В `import`
   после сохранения: если хост — wolnelektury.pl, взять `dc:rights` через
   OAI `GetRecord` по slug и проставить `public`/`public_basis`.
3. **Wolne Lektury, аудио (1–2 дня).** Обобщить `LibriVox`/`Section` в
   «аудиокнигу с дорожками» и `FoundAudio.id` — в `wl:{slug}`/`lv:{id}`,
   `searchAudio` опрашивает оба источника, `importAudio` берёт дорожки из
   `api/2/books/{slug}/media/mp3/` (`part_name`, `url`, `duration`).
   Чтеца и режиссёра — в описание: это атрибуция по лицензии. `fetch`,
   `download`, `pull` не меняются.
4. **Wikisource ru/uk/pl/de (1–2 дня).** `wikisource(client, q, lang)`:
   поиск через `api.php` с `prop=pageprops`, отсев неоднозначностей; язык
   — по языку интерфейса и по алфавиту запроса (кириллица → ru, uk).
   `download_url` — ссылка ws-export с `format=epub-3`. Хост
   `ws-export.wmcloud.org`; для этого запроса таймаут больше 30 с. В
   разборе epub убрать префикс «автор »/«автор: » из `dc:creator`. По
   желанию — написать мейнтейнерам ws-export (Phabricator, тег
   `ws-export`) о таком использовании, раз у них `Disallow: /`.
5. **TextGrid, немецкий (1 день).** Поиск TG-search (XML, `quick-xml`
   уже в зависимостях) с фильтром проекта, только
   `text/tg.edition+tg.aggregation+xml`; epub через aggregator. Нужен
   способ передать название из выдачи: в `ImportBody` его нет. Хост
   `textgridlab.org`. Атрибуция CC BY.
6. **Standard Ebooks (после ответа от них).** При доступе к OPDS —
   разбор acquisition-ссылок и `?source=download`, хост
   `standardebooks.org`. Без доступа — не подключать.
7. **Потом, по спросу:** OAPEN через OAI-PMH (pdf, нон-фикшн), az.lib.ru
   (если появится https: распаковка `fb2.zip`), аудио Project Gutenberg.

## Отвергнутые

| источник | причина | первоисточник |
|---|---|---|
| Feedbooks Public Domain | закрыт: каталог OPDS стал «Cantook Market» (B2B), `www.feedbooks.com` отдаёт проверку Cloudflare; по [Википедии](https://en.wikipedia.org/wiki/Feedbooks), с 2024 домен ведёт на Cantook | `catalog.feedbooks.com/catalog/index.atom` |
| HathiTrust | Bibliographic API отвечает, но `babel.hathitrust.org` и `www` закрыты проверкой Cloudflare; страницу Data API прочитать не удалось | `catalog.hathitrust.org/api/volumes/brief/oclc/424023.json` |
| Projekt Gutenberg-DE | «Die Nutzung der… Texte ist ausschließlich zum privaten Gebrauch gestattet» | [Nutzungsbedingungen](https://projekt-gutenberg.org/nutzungsbedingungen/) |
| Zeno.org | «Es dürfen keine Roboter, Spider oder andere automatischen Geräte eingesetzt werden»; база данных по §§ 87a ff. UrhG. Тот же корпус открыт в TextGrid | [Nutzungsbedingungen](http://www.zeno.org/Zeno/-/Nutzungsbedingungen) |
| Deutsches Textarchiv | исследовательский корпус TEI в исторической орфографии; загрузки (`/book/download_txt/…`) закрыты JS-проверкой с cookie, обходить её не стоит | [API](https://www.deutschestextarchiv.de/api) |
| Gallica (BnF) | SRU из сети проверки заблокирован Cloudflare; французский, в основном сканы | `gallica.bnf.fr/SRU` |
| Europeana | нужен ключ (`invalid_apikey`), агрегатор ссылок на сканы | `api.europeana.eu/record/v2/search.json` |
| Polona, Президентская библиотека, «Культура України» | сканы, не текст; API не проверялся подробно | — |
| НЭБ | вне России — заглушка «Отключите VPN» | `rusneb.ru` |
| Чтиво | проект закрыт | chtyvo.org.ua |
| УкрЛіб | лицензии нет, только «посилання обов'язкове», API нет | ukrlib.com.ua |
| Faded Page | ОД в Канаде, API нет; 9 189 книг (en 8 738, fr 266, de 124) | [fadedpage.com](https://www.fadedpage.com/) |
| Project Gutenberg Australia | ОД в Австралии, API нет, с 01.01.2025 новые книги не добавляются | [gutenberg.net.au](https://gutenberg.net.au/) |
| Vorleser.net | только частное использование | [AGB](https://www.vorleser.net/agb) |
| Zeit für Geschichten | домен не резолвится | — |
| Loyal Books | перепаковывает LibriVox и Gutenberg — нового нет | — |
| Lit2Go | `etc.usf.edu/lit2go/` отвечает 500; условия не проверены | — |
| Global Digital Library | `api.digitallibrary.io` не резолвится, сайт переделан | — |
| African Storybook, Bloom Library | книжки-картинки; API не проверялся подробно | — |
| tolstoy.ru (пока) | «Использование любых… материалов без официального разрешения запрещено»; в fb2 комментарии Н. К. Гудзия († 1965) | подвал [tolstoy.ru](https://tolstoy.ru/) |
| rvb.ru, ilibrary.ru, lib.ru | только HTML или txt, лицензии нет, © составителя | см. раздел «глубже» |
| imwerden.de, Wikilivres.ru | pdf-сканы или вики с современными авторами; у Wikilivres API отвечает 429 | `imwerden.de/opds/`, `wikilivres.ru/api.php` |
| ЛитРес | российская компания, рубли; `/download_book/` закрыт в `robots.txt` | `litres.ru/robots.txt` |
| ukrclassic, chytanka, diasporiana, izbornyk | «лише при розміщенні посилання», «виключно з метою ознайомлення», сканы, HTML | см. раздел «глубже» |
| javalibre.com.ua | 403 на User-Agent Leser | — |
| «Слухай», «Старое радио», Суспільне | аудио без лицензии; Суспільне — 403 | sluhay.com.ua, staroeradio.ru |
| Беларуская Палічка, Kamunikat | «спасылка… абавязковая», современные авторы, pdf | knihi.com, kamunikat.org |
| Unglue.it | OPDS работает (`https://unglue.it/api/opds/`), но это агрегатор тех же DOAB/Gutenberg со ссылками на десятки хостов | — |

## Что осталось непроверенным

- Текущий текст условий Internet Archive (страница требует JavaScript).
- Нужен ли вход для скачивания в StoryWeaver; API African Storybook,
  Bloom Library, Polona, Президентской библиотеки.
- Почему OAPEN/DOAB REST отвечают 403 — блокировка IP или новая политика.
- Норма о реабилитированных авторах в украинском законе 2022 года.
- Время сборки ws-export на многотомниках («Война и мир»).
- Статус вычитки Wikisource: у страниц сканов он есть (категории из
  `meta=proofreadinfo`), у произведений — нет; фильтровать книги по нему
  можно только через индекс. Стоит ли — не решено.
- Ответит ли музей Толстого на просьбу о разрешении; условия архива
  радио Суспільного (сайт отвечает 403); условия javalibre.com.ua (403).
- Год реабилитации Миколы Куліша и как реабилитация считается в законе
  Украины 2022 года (UDraCor).
- Источники по другим языкам (Aozora, Runeberg, Liber Liber, Chitanka,
  Ben-Yehuda и др.) — задача сменилась, не проверялись.
- «Е-бібліотека „Україна“» — не опознана.

## Источники

- Wolne Lektury — API: https://wolnelektury.pl/api/
- Wolne Lektury — Zasady wykorzystania: https://wolnelektury.pl/info/zasady-wykorzystania/
- Wolne Lektury — исходный код: https://github.com/fnp/wolnelektury
- Wikisource — WS Export: https://wikisource.org/wiki/Wikisource:WS_Export
- ws-export — исходный код: https://gitlab.wikimedia.org/toolforge-repos/wsexport
- Викитека — Авторские права: https://ru.wikisource.org/wiki/Викитека:Авторские_права
- Wikimedia User-Agent Policy: https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy
- Wikimedia Robot policy: https://wikitech.wikimedia.org/wiki/Robot_policy
- MediaWiki API:Etiquette: https://www.mediawiki.org/wiki/API:Etiquette
- Project Gutenberg — Robot Access: https://www.gutenberg.org/policy/robot_access.html
- Project Gutenberg — Terms of Use: https://www.gutenberg.org/policy/terms_of_use.html
- Project Gutenberg — Permission How-To: https://www.gutenberg.org/policy/permission.html
- Project Gutenberg — Offline Catalogs: https://www.gutenberg.org/ebooks/offline_catalogs.html
- Project Gutenberg — зеркала: https://www.gutenberg.org/dirs/MIRRORS.ALL
- gutendex: https://github.com/garethbjohnson/gutendex
- LibriVox — API info: https://librivox.org/api/info
- LibriVox — Copyright, Public Domain: https://librivox.org/pages/public-domain/
- TextGrid — Digitale Bibliothek: https://textgridrep.org/project/TGPR-372fe6dc-57f2-6cd4-01b5-2c4bbefcfd3c
- Standard Ebooks — Ebook Feeds: https://standardebooks.org/feeds
- Internet Archive — APIs: https://archive.org/developers/index-apis.html
- Internet Archive — пост об условиях использования: https://blog.archive.org/?p=9040
- Open Library — Developer API: https://openlibrary.org/developers/api
- OAPEN — метаданные и API: https://oapen.org/resources/15635975-metadata
- StoryWeaver — About: https://storyweaver.org.in/about
- Feedbooks — Википедия: https://en.wikipedia.org/wiki/Feedbooks
- Projekt Gutenberg-DE — Nutzungsbedingungen: https://projekt-gutenberg.org/nutzungsbedingungen/
- Zeno.org — Nutzungsbedingungen: http://www.zeno.org/Zeno/-/Nutzungsbedingungen
- Deutsches Textarchiv — APIs: https://www.deutschestextarchiv.de/api
- Vorleser.net — AGB: https://www.vorleser.net/agb
- Duke — Public Domain Day 2026: https://web.law.duke.edu/cspd/publicdomainday/2026/
- Директива 2006/116/EC (срок охраны): https://www.legislation.gov.uk/eudr/2006/116/contents
- CC BY-SA 4.0: https://creativecommons.org/licenses/by-sa/4.0/
- Licence Art Libre 1.3: https://artlibre.org/licence/lal/en/
- Вікікрыніцы — Аўтарскае права: https://be.wikisource.org/wiki/Вікікрыніцы:Аўтарскае_права
- ProofreadPage API (meta=proofreadinfo): https://ru.wikisource.org/w/api.php?action=help&modules=query%2Bproofreadinfo
- tolstoy.ru — «Весь Толстой в один клик»: https://tolstoy.ru/projects/tolstoy-in-one-click/
- tolstoy.ru — 90-томное собрание: https://tolstoy.ru/creativity/90-volume-collection-of-the-works/
- DraCor — API: https://dracor.org/doc/api
- DraCor — корпуса: https://dracor.org/api/v1/corpora?include=metrics
- ELTeC — организация на GitHub: https://github.com/COST-ELTeC
- ELTeC-ukr — README и лицензия: https://github.com/COST-ELTeC/ELTeC-ukr
- ELTeC-rus: https://github.com/COST-ELTeC/ELTeC-rus
- imwerden.de — OPDS: https://imwerden.de/opds/
- Wikilivres.ru: https://wikilivres.ru/
- rvb.ru — о проекте: https://rvb.ru/about/
- ilibrary.ru: https://ilibrary.ru/
- ukrclassic.com.ua: https://ukrclassic.com.ua/
- chytanka.com.ua: https://chytanka.com.ua/
- «Слухай» — о запуске: https://texty.org.ua/fragments/90211/Zapushheno_pershu_onlajnbiblioteku_audioknyg_ukrajinskoju-90211/
- Беларуская Палічка: https://knihi.com/
- Kamunikat.org: https://kamunikat.org/
