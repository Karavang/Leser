/// Перевод интерфейса. Словарь и три функции вместо библиотеки: языка два,
/// строк полторы сотни, склонений и множественного числа в них нет.
/// Понадобится третий язык или числа вида «5 книг» — тогда i18next.
///
/// Ключи — короткие имена, значения со вставками — функции.

export const RU = {
  // общее
  toLibrary: "К библиотеке",
  close: "Закрыть",
  loading: "Загрузка",
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

/// Выбранный язык, иначе язык браузера. Русский — только если браузер русский,
/// всё остальное, включая незаданный язык, читает интерфейс по-английски.
export const pick = (saved, browser) => {
  if (saved === "ru" || saved === "en") return saved;
  return String(browser || "").toLowerCase().startsWith("ru") ? "ru" : "en";
};

export const lang = pick(
  globalThis.localStorage?.getItem("lang"),
  globalThis.navigator?.language,
);

const dict = lang === "ru" ? RU : EN;

/// Строка по ключу. Незнакомый ключ возвращается как есть — на экране это
/// видно сразу, а падать из-за подписи к кнопке приложение не должно.
export const t = (key, ...args) => {
  const value = dict[key] ?? key;
  return typeof value === "function" ? value(...args) : value;
};

/// ponytail: перезагрузка вместо контекста и перерисовки всего дерева —
/// язык меняют раз в жизни, а страница читателя и так лежит в localStorage.
export const setLang = (next) => {
  localStorage.setItem("lang", next);
  location.reload();
};
