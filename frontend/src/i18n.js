/// Перевод интерфейса. Словари и три функции вместо библиотеки: языков пять,
/// строк сотня, склонений и чисел вида «5 книг» в них нет — только подписи.
/// Понадобится множественное число (а в польском и украинском их три формы) —
/// тогда i18next с Intl.PluralRules, а не свой велосипед.
///
/// Ключи — короткие имена, значения со вставками — функции.

export const RU = {
  // общее
  toLibrary: "К библиотеке",
  close: "Закрыть",
  loading: "Загрузка",
  loadingBook: "Открываю книгу",
  profile: "Профиль",

  // библиотека
  tabLibrary: "Библиотека",
  tabMine: "Мои книги",
  searchPlaceholder: "Название, автор или серия",
  noBooks: "Книг пока нет",
  more: "Подробнее",
  shelfAdd: "В мои книги",
  shelfRemove: "Убрать из моих книг",
  shelfRemoveShort: "Убрать из моих",
  deleteBook: "Удалить книгу",
  confirmDeleteBook: (title) => `Удалить «${title}»? Отменить нельзя.`,
  deleteBookFailed: "Не удалось удалить книгу",
  shelfFailed: "Не удалось изменить «мои книги»",

  // карточка книги
  series: "Серия",
  published: "Издана",
  language: "Язык",
  source: "Источник",
  addedAt: "Добавлена",

  // внешние источники
  otherSources: "Другие источники",
  importing: "Забираем…",
  importBook: "В библиотеку",
  importFailed: "Не удалось добавить книгу",

  // загрузка книг
  addBooks: "Добавить книги",
  dropHere: "Перетащите книги сюда",
  orPick: "или нажмите, чтобы выбрать на диске",
  eachUpTo: (mb) => `до ${mb} МБ каждая`,
  queued: "в очереди",
  uploaded: "готово",
  badFormat: "формат не читается",
  tooBig: (mb) => `больше ${mb} МБ`,
  uploadFailed: "не удалось загрузить",

  // профиль и настройки
  booksInRead: "Читаю сейчас",
  noBooksInRead: "Ни одной книги пока не открыто",
  loadFailed: "Не удалось загрузить",
  deleteFailed: "Не удалось удалить",

  // статистика
  daysInRow: "дней подряд",
  pagesMonth: "страниц за месяц",
  statReading: "читаю",
  statFinished: "прочитано",

  // цитаты и словарь
  quotes: "Цитаты",
  noQuotes: "Выделите текст в книге — он сохранится сюда",
  openAtQuote: "Открыть книгу на этом месте",
  saveQuote: "В цитаты",
  quoteSaved: "Цитата сохранена",
  dictionary: "Словарь",
  noWords: "Выделите слово в книге — оно сохранится сюда",
  saveWord: "В словарь",
  wordSaved: "Слово в словаре",
  ankiExport: "Карточки для Anki",
  saveFailed: "Не удалось сохранить",

  // выгрузка
  exportAll: "Забрать всё своё",
  exportHint: "закладки, полка, цитаты и словарь одним файлом",
  exportDo: "Выгрузить",
  exportFailed: "Не удалось выгрузить",

  settings: "Настройки",
  logout: "Выйти из аккаунта",
  logoutHint: "на этом устройстве, книги и закладки останутся",
  deleteAccount: "Удалить аккаунт",
  deleteAccountHint: "закладки пропадут, загруженные книги останутся в библиотеке",
  delete: "Удалить",
  confirmDeleteAccount: "Удалить аккаунт? Закладки пропадут, отменить нельзя.",
  deleteAccountFailed: "Не удалось удалить аккаунт",
  interfaceLanguage: "Язык интерфейса",
  languageHint: "по умолчанию — язык браузера",
  theme: "Тема",
  themeHint: "одна на приложение и читалку",

  // читалка
  openFailed: "Не удалось открыть книгу",
  fontSmaller: "Меньше шрифт",
  fontBigger: "Больше шрифт",
  contents: "Оглавление",
  parsing: "Разбираем книгу…",
  picsOn: "Показать иллюстрации",
  picsOff: "Скрыть иллюстрации",
  prevPage: "Назад",
  nextPage: "Дальше",
  part: (n) => `Часть ${n}`,
  backToPage: (n) => `← назад к странице ${n}`,
  position: "Положение в книге",
  themeDay: "День",
  themeSepia: "Сепия",
  themeNight: "Ночь",

  // вход
  signIn: "Вход",
  signUp: "Регистрация",
  username: "Имя",
  email: "Почта",
  emailHint: "Введите почту",
  password: "Пароль",
  passwordHint: "Придумайте пароль, минимум 8 символов",
  noAccount: "Ещё нет аккаунта?",
  goRegister: "Зарегистрироваться",
  haveAccount: "Уже есть аккаунт?",
  goLogin: "Войти",
  loginFailed: "Не удалось войти",
  serverSilent: "Сервер не отвечает",
};

export const EN = {
  toLibrary: "To the library",
  close: "Close",
  loading: "Loading",
  loadingBook: "Opening the book",
  profile: "Profile",

  tabLibrary: "Library",
  tabMine: "My books",
  searchPlaceholder: "Title, author or series",
  noBooks: "No books yet",
  more: "Details",
  shelfAdd: "Add to my books",
  shelfRemove: "Remove from my books",
  shelfRemoveShort: "Remove from mine",
  deleteBook: "Delete book",
  confirmDeleteBook: (title) => `Delete “${title}”? This cannot be undone.`,
  deleteBookFailed: "Could not delete the book",
  shelfFailed: "Could not update “my books”",

  series: "Series",
  published: "Published",
  language: "Language",
  source: "Source",
  addedAt: "Added",

  otherSources: "Other sources",
  importing: "Fetching…",
  importBook: "Add to library",
  importFailed: "Could not add the book",

  addBooks: "Add books",
  dropHere: "Drop your books here",
  orPick: "or click to pick them on disk",
  eachUpTo: (mb) => `up to ${mb} MB each`,
  queued: "queued",
  uploaded: "done",
  badFormat: "format not supported",
  tooBig: (mb) => `over ${mb} MB`,
  uploadFailed: "upload failed",

  booksInRead: "Currently reading",
  noBooksInRead: "No book opened yet",
  loadFailed: "Could not load",
  deleteFailed: "Could not delete",

  daysInRow: "day streak",
  pagesMonth: "pages this month",
  statReading: "reading",
  statFinished: "finished",

  quotes: "Quotes",
  noQuotes: "Select text in a book — it lands here",
  openAtQuote: "Open the book at this spot",
  saveQuote: "Save quote",
  quoteSaved: "Quote saved",
  dictionary: "Dictionary",
  noWords: "Select a word in a book — it lands here",
  saveWord: "Save word",
  wordSaved: "Saved to the dictionary",
  ankiExport: "Anki cards",
  saveFailed: "Could not save",

  exportAll: "Take everything with you",
  exportHint: "bookmarks, shelf, quotes and dictionary in one file",
  exportDo: "Export",
  exportFailed: "Could not export",

  settings: "Settings",
  logout: "Sign out",
  logoutHint: "on this device; books and bookmarks stay",
  deleteAccount: "Delete account",
  deleteAccountHint: "bookmarks are lost, uploaded books stay in the library",
  delete: "Delete",
  confirmDeleteAccount:
    "Delete the account? Bookmarks will be lost, this cannot be undone.",
  deleteAccountFailed: "Could not delete the account",
  interfaceLanguage: "Interface language",
  languageHint: "browser language by default",
  theme: "Theme",
  themeHint: "shared by the app and the reader",

  openFailed: "Could not open the book",
  fontSmaller: "Smaller text",
  fontBigger: "Larger text",
  contents: "Contents",
  parsing: "Parsing the book…",
  picsOn: "Show illustrations",
  picsOff: "Hide illustrations",
  prevPage: "Back",
  nextPage: "Next",
  part: (n) => `Part ${n}`,
  backToPage: (n) => `← back to page ${n}`,
  position: "Position in the book",
  themeDay: "Day",
  themeSepia: "Sepia",
  themeNight: "Night",

  signIn: "Sign in",
  signUp: "Sign up",
  username: "Username",
  email: "Email",
  emailHint: "Enter your email",
  password: "Password",
  passwordHint: "Create a password, at least 8 characters",
  noAccount: "No account yet?",
  goRegister: "Sign up",
  haveAccount: "Already have an account?",
  goLogin: "Sign in",
  loginFailed: "Could not sign in",
  serverSilent: "The server is not responding",
};

export const UK = {
  toLibrary: "До бібліотеки",
  close: "Закрити",
  loading: "Завантаження",
  loadingBook: "Відкриваю книжку",
  profile: "Профіль",

  tabLibrary: "Бібліотека",
  tabMine: "Мої книжки",
  searchPlaceholder: "Назва, автор або серія",
  noBooks: "Книжок поки немає",
  more: "Докладніше",
  shelfAdd: "До моїх книжок",
  shelfRemove: "Прибрати з моїх книжок",
  shelfRemoveShort: "Прибрати з моїх",
  deleteBook: "Видалити книжку",
  confirmDeleteBook: (title) => `Видалити «${title}»? Скасувати не можна.`,
  deleteBookFailed: "Не вдалося видалити книжку",
  shelfFailed: "Не вдалося змінити «мої книжки»",

  series: "Серія",
  published: "Видано",
  language: "Мова",
  source: "Джерело",
  addedAt: "Додано",

  otherSources: "Інші джерела",
  importing: "Забираємо…",
  importBook: "До бібліотеки",
  importFailed: "Не вдалося додати книжку",

  addBooks: "Додати книжки",
  dropHere: "Перетягніть книжки сюди",
  orPick: "або натисніть, щоб вибрати на диску",
  eachUpTo: (mb) => `до ${mb} МБ кожна`,
  queued: "у черзі",
  uploaded: "готово",
  badFormat: "формат не читається",
  tooBig: (mb) => `більше ${mb} МБ`,
  uploadFailed: "не вдалося завантажити",
  parsing: "Розбираємо книжку…",

  booksInRead: "Читаю зараз",
  noBooksInRead: "Жодної книжки ще не відкрито",
  loadFailed: "Не вдалося завантажити",
  deleteFailed: "Не вдалося видалити",

  daysInRow: "днів поспіль",
  pagesMonth: "сторінок за місяць",
  statReading: "читаю",
  statFinished: "прочитано",

  quotes: "Цитати",
  noQuotes: "Виділіть текст у книжці — він збережеться сюди",
  openAtQuote: "Відкрити книжку на цьому місці",
  saveQuote: "До цитат",
  quoteSaved: "Цитату збережено",
  dictionary: "Словник",
  noWords: "Виділіть слово у книжці — воно збережеться сюди",
  saveWord: "До словника",
  wordSaved: "Слово у словнику",
  ankiExport: "Картки для Anki",
  saveFailed: "Не вдалося зберегти",

  exportAll: "Забрати все своє",
  exportHint: "закладки, полиця, цитати та словник одним файлом",
  exportDo: "Вивантажити",
  exportFailed: "Не вдалося вивантажити",

  settings: "Налаштування",
  logout: "Вийти з акаунта",
  logoutHint: "на цьому пристрої, книжки та закладки залишаться",
  deleteAccount: "Видалити акаунт",
  deleteAccountHint: "закладки зникнуть, завантажені книжки залишаться в бібліотеці",
  delete: "Видалити",
  confirmDeleteAccount: "Видалити акаунт? Закладки зникнуть, скасувати не можна.",
  deleteAccountFailed: "Не вдалося видалити акаунт",
  interfaceLanguage: "Мова інтерфейсу",
  languageHint: "за замовчуванням — мова браузера",
  theme: "Тема",
  themeHint: "спільна для застосунку та читалки",

  openFailed: "Не вдалося відкрити книжку",
  fontSmaller: "Менший шрифт",
  fontBigger: "Більший шрифт",
  contents: "Зміст",
  picsOn: "Показати ілюстрації",
  picsOff: "Сховати ілюстрації",
  prevPage: "Назад",
  nextPage: "Далі",
  part: (n) => `Частина ${n}`,
  backToPage: (n) => `← назад до сторінки ${n}`,
  position: "Місце в книжці",
  themeDay: "День",
  themeSepia: "Сепія",
  themeNight: "Ніч",

  signIn: "Вхід",
  signUp: "Реєстрація",
  username: "Ім’я",
  email: "Пошта",
  emailHint: "Введіть пошту",
  password: "Пароль",
  passwordHint: "Придумайте пароль, щонайменше 8 символів",
  noAccount: "Ще немає акаунта?",
  goRegister: "Зареєструватися",
  haveAccount: "Вже є акаунт?",
  goLogin: "Увійти",
  loginFailed: "Не вдалося увійти",
  serverSilent: "Сервер не відповідає",
};

export const PL = {
  toLibrary: "Do biblioteki",
  close: "Zamknij",
  loading: "Wczytywanie",
  loadingBook: "Otwieram książkę",
  profile: "Profil",

  tabLibrary: "Biblioteka",
  tabMine: "Moje książki",
  searchPlaceholder: "Tytuł, autor lub cykl",
  noBooks: "Nie ma jeszcze książek",
  more: "Szczegóły",
  shelfAdd: "Do moich książek",
  shelfRemove: "Usuń z moich książek",
  shelfRemoveShort: "Usuń z moich",
  deleteBook: "Usuń książkę",
  confirmDeleteBook: (title) => `Usunąć „${title}”? Tego nie można cofnąć.`,
  deleteBookFailed: "Nie udało się usunąć książki",
  shelfFailed: "Nie udało się zmienić „moich książek”",

  series: "Cykl",
  published: "Wydana",
  language: "Język",
  source: "Źródło",
  addedAt: "Dodana",

  otherSources: "Inne źródła",
  importing: "Pobieramy…",
  importBook: "Do biblioteki",
  importFailed: "Nie udało się dodać książki",

  addBooks: "Dodaj książki",
  dropHere: "Przeciągnij książki tutaj",
  orPick: "albo kliknij, aby wybrać je na dysku",
  eachUpTo: (mb) => `do ${mb} MB każda`,
  queued: "w kolejce",
  uploaded: "gotowe",
  badFormat: "format nieobsługiwany",
  tooBig: (mb) => `ponad ${mb} MB`,
  uploadFailed: "nie udało się wysłać",
  parsing: "Przetwarzamy książkę…",

  booksInRead: "Teraz czytam",
  noBooksInRead: "Żadna książka nie została jeszcze otwarta",
  loadFailed: "Nie udało się wczytać",
  deleteFailed: "Nie udało się usunąć",

  daysInRow: "dni z rzędu",
  pagesMonth: "stron w tym miesiącu",
  statReading: "czytam",
  statFinished: "przeczytane",

  quotes: "Cytaty",
  noQuotes: "Zaznacz tekst w książce — trafi tutaj",
  openAtQuote: "Otwórz książkę w tym miejscu",
  saveQuote: "Do cytatów",
  quoteSaved: "Cytat zapisany",
  dictionary: "Słownik",
  noWords: "Zaznacz słowo w książce — trafi tutaj",
  saveWord: "Do słownika",
  wordSaved: "Słowo w słowniku",
  ankiExport: "Fiszki do Anki",
  saveFailed: "Nie udało się zapisać",

  exportAll: "Zabierz wszystko swoje",
  exportHint: "zakładki, półka, cytaty i słownik w jednym pliku",
  exportDo: "Eksportuj",
  exportFailed: "Nie udało się wyeksportować",

  settings: "Ustawienia",
  logout: "Wyloguj się",
  logoutHint: "na tym urządzeniu; książki i zakładki zostają",
  deleteAccount: "Usuń konto",
  deleteAccountHint: "zakładki przepadną, wgrane książki zostaną w bibliotece",
  delete: "Usuń",
  confirmDeleteAccount: "Usunąć konto? Zakładki przepadną, tego nie można cofnąć.",
  deleteAccountFailed: "Nie udało się usunąć konta",
  interfaceLanguage: "Język interfejsu",
  languageHint: "domyślnie język przeglądarki",
  theme: "Motyw",
  themeHint: "wspólny dla aplikacji i czytnika",

  openFailed: "Nie udało się otworzyć książki",
  fontSmaller: "Mniejsza czcionka",
  fontBigger: "Większa czcionka",
  contents: "Spis treści",
  picsOn: "Pokaż ilustracje",
  picsOff: "Ukryj ilustracje",
  prevPage: "Wstecz",
  nextPage: "Dalej",
  part: (n) => `Część ${n}`,
  backToPage: (n) => `← wróć do strony ${n}`,
  position: "Miejsce w książce",
  themeDay: "Dzień",
  themeSepia: "Sepia",
  themeNight: "Noc",

  signIn: "Logowanie",
  signUp: "Rejestracja",
  username: "Imię",
  email: "E-mail",
  emailHint: "Podaj e-mail",
  password: "Hasło",
  passwordHint: "Wymyśl hasło, minimum 8 znaków",
  noAccount: "Nie masz jeszcze konta?",
  goRegister: "Zarejestruj się",
  haveAccount: "Masz już konto?",
  goLogin: "Zaloguj się",
  loginFailed: "Nie udało się zalogować",
  serverSilent: "Serwer nie odpowiada",
};

export const DE = {
  toLibrary: "Zur Bibliothek",
  close: "Schließen",
  loading: "Wird geladen",
  loadingBook: "Buch wird geöffnet",
  profile: "Profil",

  tabLibrary: "Bibliothek",
  tabMine: "Meine Bücher",
  searchPlaceholder: "Titel, Autor oder Reihe",
  noBooks: "Noch keine Bücher",
  more: "Details",
  shelfAdd: "Zu meinen Büchern",
  shelfRemove: "Aus meinen Büchern entfernen",
  shelfRemoveShort: "Aus meinen entfernen",
  deleteBook: "Buch löschen",
  confirmDeleteBook: (title) => `„${title}“ löschen? Das lässt sich nicht rückgängig machen.`,
  deleteBookFailed: "Buch konnte nicht gelöscht werden",
  shelfFailed: "„Meine Bücher“ konnten nicht geändert werden",

  series: "Reihe",
  published: "Erschienen",
  language: "Sprache",
  source: "Quelle",
  addedAt: "Hinzugefügt",

  otherSources: "Andere Quellen",
  importing: "Wird geholt…",
  importBook: "In die Bibliothek",
  importFailed: "Buch konnte nicht hinzugefügt werden",

  addBooks: "Bücher hinzufügen",
  dropHere: "Bücher hierher ziehen",
  orPick: "oder klicken, um sie auf der Festplatte zu wählen",
  eachUpTo: (mb) => `bis ${mb} MB pro Datei`,
  queued: "in der Warteschlange",
  uploaded: "fertig",
  badFormat: "Format nicht lesbar",
  tooBig: (mb) => `über ${mb} MB`,
  uploadFailed: "Upload fehlgeschlagen",
  parsing: "Buch wird verarbeitet…",

  booksInRead: "Ich lese gerade",
  noBooksInRead: "Noch kein Buch geöffnet",
  loadFailed: "Konnte nicht geladen werden",
  deleteFailed: "Konnte nicht gelöscht werden",

  daysInRow: "Tage in Folge",
  pagesMonth: "Seiten diesen Monat",
  statReading: "lese ich",
  statFinished: "gelesen",

  quotes: "Zitate",
  noQuotes: "Text im Buch markieren — er landet hier",
  openAtQuote: "Buch an dieser Stelle öffnen",
  saveQuote: "Als Zitat speichern",
  quoteSaved: "Zitat gespeichert",
  dictionary: "Wörterbuch",
  noWords: "Wort im Buch markieren — es landet hier",
  saveWord: "Ins Wörterbuch",
  wordSaved: "Wort im Wörterbuch",
  ankiExport: "Karten für Anki",
  saveFailed: "Konnte nicht gespeichert werden",

  exportAll: "Alles Eigene mitnehmen",
  exportHint: "Lesezeichen, Regal, Zitate und Wörterbuch in einer Datei",
  exportDo: "Exportieren",
  exportFailed: "Export fehlgeschlagen",

  settings: "Einstellungen",
  logout: "Abmelden",
  logoutHint: "auf diesem Gerät; Bücher und Lesezeichen bleiben",
  deleteAccount: "Konto löschen",
  deleteAccountHint: "Lesezeichen gehen verloren, hochgeladene Bücher bleiben in der Bibliothek",
  delete: "Löschen",
  confirmDeleteAccount: "Konto löschen? Lesezeichen gehen verloren, das lässt sich nicht rückgängig machen.",
  deleteAccountFailed: "Konto konnte nicht gelöscht werden",
  interfaceLanguage: "Sprache der Oberfläche",
  languageHint: "Standard ist die Browsersprache",
  theme: "Design",
  themeHint: "gilt für App und Leseansicht",

  openFailed: "Buch konnte nicht geöffnet werden",
  fontSmaller: "Kleinere Schrift",
  fontBigger: "Größere Schrift",
  contents: "Inhalt",
  picsOn: "Illustrationen zeigen",
  picsOff: "Illustrationen ausblenden",
  prevPage: "Zurück",
  nextPage: "Weiter",
  part: (n) => `Teil ${n}`,
  backToPage: (n) => `← zurück zu Seite ${n}`,
  position: "Position im Buch",
  themeDay: "Tag",
  themeSepia: "Sepia",
  themeNight: "Nacht",

  signIn: "Anmelden",
  signUp: "Registrieren",
  username: "Name",
  email: "E-Mail",
  emailHint: "E-Mail eingeben",
  password: "Passwort",
  passwordHint: "Passwort ausdenken, mindestens 8 Zeichen",
  noAccount: "Noch kein Konto?",
  goRegister: "Registrieren",
  haveAccount: "Schon ein Konto?",
  goLogin: "Anmelden",
  loginFailed: "Anmeldung fehlgeschlagen",
  serverSilent: "Der Server antwortet nicht",
};

/// Языки интерфейса: код, как он пишется в `lang`, и название на себе самом —
/// «Deutsch» в списке понятнее, чем «Немецкий», кто бы его ни читал.
export const LANGS = [
  ["ru", "Русский"],
  ["uk", "Українська"],
  ["pl", "Polski"],
  ["en", "English"],
  ["de", "Deutsch"],
];

const DICTS = { ru: RU, uk: UK, pl: PL, en: EN, de: DE };

/// Выбранный язык, иначе язык браузера, иначе английский. `ru-RU` и `de-AT` —
/// это `ru` и `de`: регион нам безразличен, словарь один на язык.
export const pick = (saved, browser) => {
  if (DICTS[saved]) return saved;
  const base = String(browser || "")
    .toLowerCase()
    .split(/[-_]/)[0];
  return DICTS[base] ? base : "en";
};

export const lang = pick(
  globalThis.localStorage?.getItem("lang"),
  globalThis.navigator?.language,
);

const dict = DICTS[lang] ?? EN;

/// Строка по ключу. Нет перевода — берём английский, нет и его — сам ключ:
/// на экране это видно сразу, а падать из-за подписи к кнопке приложение
/// не должно.
export const t = (key, ...args) => {
  const value = dict[key] ?? EN[key] ?? key;
  return typeof value === "function" ? value(...args) : value;
};

/// ponytail: перезагрузка вместо контекста и перерисовки всего дерева —
/// язык меняют раз в жизни, а страница читателя и так лежит в localStorage.
export const setLang = (next) => {
  localStorage.setItem("lang", next);
  location.reload();
};
