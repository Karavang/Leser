import axios from "axios";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { clearSession } from "../session";
import LogoutButton from "./LogoutButton";
import { lang, setLang, t } from "../i18n";
import { applyTheme, theme as saved, THEMES } from "../theme";

/// Сколько дней показывает график. Месяц — это ещё видно на телефоне
/// и уже достаточно, чтобы отличить «читаю» от «читал когда-то».
const CHART_DAYS = 30;

/// Файл в браузер. Одна функция на выгрузку кабинета и на карточки для Anki:
/// разница между ними только в том, что за данные и как называется файл.
const download = (data, name, type) => {
  const url = URL.createObjectURL(
    data instanceof Blob ? data : new Blob([data], { type }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
};

const UserPage = () => {
  const navigate = useNavigate();
  const user = JSON.parse(localStorage.getItem("user"));
  const auth = { headers: { Authorization: `Bearer ${user.token}` } };
  const [books, setBooks] = useState([]);
  const [quotes, setQuotes] = useState([]);
  const [words, setWords] = useState([]);
  const [stats, setStats] = useState(null);
  const [theme, setTheme] = useState(saved);
  const [error, setError] = useState(null);

  // Кого удалять, сервер берёт из токена — тело запроса не нужно.
  // Закладки уезжают вместе с аккаунтом, загруженные книги остаются в библиотеке.
  const deleteAccount = async () => {
    if (!confirm(t("confirmDeleteAccount"))) return;
    try {
      await axios.delete("/api/account", auth);
      clearSession();
      navigate("/login", { replace: true });
    } catch (e) {
      setError(e.response?.data?.message ?? t("deleteAccountFailed"));
    }
  };

  useEffect(() => {
    // Четыре списка кабинета одним заходом: они независимы, и ждать их
    // по очереди значило бы складывать четыре задержки сети.
    Promise.all(
      ["/api/booksInRead", "/api/quotes", "/api/words", "/api/stats"].map(
        (url) => axios.get(url, auth),
      ),
    )
      .then(([b, q, w, s]) => {
        setBooks(b.data);
        setQuotes(q.data);
        setWords(w.data);
        setStats(s.data);
      })
      .catch((e) => setError(e.response?.data?.message ?? t("loadFailed")));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [user.token]);

  /// Сервер отдаёт только те дни, когда читали. График без пропущенных дней
  /// врал бы: неделя подряд и неделя через день выглядели бы одинаково.
  const chart = useMemo(() => {
    if (!stats) return [];
    const pages = new Map(stats.days.map((d) => [d.day, d.pages]));
    const last = new Date(`${stats.today}T00:00:00Z`);
    return Array.from({ length: CHART_DAYS }, (_, i) => {
      const day = new Date(last);
      day.setUTCDate(last.getUTCDate() - (CHART_DAYS - 1 - i));
      const key = day.toISOString().slice(0, 10);
      return { day: key, pages: pages.get(key) || 0 };
    });
  }, [stats]);

  const peak = Math.max(1, ...chart.map((d) => d.pages));
  const month = chart.reduce((sum, d) => sum + d.pages, 0);

  const forget = (what, id, setter) =>
    axios
      .delete(`/api/${what}/${id}`, auth)
      .then(() => setter((list) => list.filter((x) => x.id !== id)))
      .catch((e) => setError(e.response?.data?.message ?? t("deleteFailed")));

  const exportAll = () =>
    axios
      .get("/api/export", { ...auth, responseType: "blob" })
      .then((r) => download(r.data, "leser.json"))
      .catch((e) => setError(e.response?.data?.message ?? t("exportFailed")));

  // Anki читает столбцы через табуляцию: слово и пример. Пробелы в контексте
  // уже схлопнуты читалкой, так что ломать столбцы там нечему.
  const toAnki = () =>
    download(
      words.map((w) => `${w.word}\t${w.context}`).join("\n"),
      "leser-anki.tsv",
      "text/tab-separated-values;charset=utf-8",
    );

  return (
    <div className="userPage">
      <div
        className="closeButton"
        onClick={() => navigate("/")}
        title={t("toLibrary")}
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          height="40px"
          viewBox="0 -960 960 960"
          width="40px"
        >
          <path d="M226.67-186.67h140v-246.66h226.66v246.66h140v-380L480-756.67l-253.33 190v380ZM160-120v-480l320-240 320 240v480H526.67v-246.67h-93.34V-120H160Zm320-352Z" />
        </svg>
      </div>
      <div className="userDetails">
        <h1>{user.username}</h1>
        <h2>{user.email}</h2>

        {stats && (
          <div className="stats">
            <div className="statNumbers">
              <b>
                {stats.streak}
                <small>{t("daysInRow")}</small>
              </b>
              <b>
                {month}
                <small>{t("pagesMonth")}</small>
              </b>
              <b>
                {stats.reading}
                <small>{t("statReading")}</small>
              </b>
              <b>
                {stats.finished}
                <small>{t("statFinished")}</small>
              </b>
            </div>
            {/* столбики — это div'ы с высотой в процентах: библиотека графиков
                ради тридцати чисел не окупается */}
            <div className="statChart">
              {chart.map((d) => (
                <i
                  key={d.day}
                  title={`${d.day} · ${d.pages}`}
                  style={{ height: `${Math.round((d.pages / peak) * 100)}%` }}
                />
              ))}
            </div>
          </div>
        )}

        <h3>{t("booksInRead")}</h3>
        {books.length === 0 && <p className="empty">{t("noBooksInRead")}</p>}
        <ul>
          {books.map((book) => (
            <li
              key={book.id}
              onClick={() => navigate(`/book/${book.filename}`)}
            >
              <h4>{book.title}</h4>
              <p>{book.author}</p>
            </li>
          ))}
        </ul>

        <details className="notes">
          <summary>
            {t("quotes")}
            <span>{quotes.length}</span>
          </summary>
          {quotes.length === 0 && <p className="empty">{t("noQuotes")}</p>}
          {quotes.map((q) => (
            <blockquote key={q.id}>
              <p
                // Книги может уже не быть в библиотеке — цитата остаётся,
                // но открывать её тогда негде.
                className={q.filename ? "goto" : ""}
                onClick={() =>
                  q.filename &&
                  navigate(`/book/${q.filename}?at=${q.position}`)
                }
                title={q.filename ? t("openAtQuote") : undefined}
              >
                {q.text}
              </p>
              <cite>{q.title}</cite>
              <button
                title={t("delete")}
                onClick={() => forget("quotes", q.id, setQuotes)}
              >
                ×
              </button>
            </blockquote>
          ))}
        </details>

        <details className="notes">
          <summary>
            {t("dictionary")}
            <span>{words.length}</span>
          </summary>
          {words.length === 0 && <p className="empty">{t("noWords")}</p>}
          {words.length > 0 && (
            <button className="secondary" onClick={toAnki}>
              {t("ankiExport")}
            </button>
          )}
          <ul className="wordList">
            {words.map((w) => (
              <li key={w.id}>
                <b lang={w.lang || undefined}>{w.word}</b>
                <span>{w.context}</span>
                <button
                  title={t("delete")}
                  onClick={() => forget("words", w.id, setWords)}
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        </details>

        {/* details вместо кнопки с useState: открытие/закрытие, клавиатура
            и доступность уже есть в браузере, писать нечего */}
        <details className="settings">
          <summary>
            <svg
              xmlns="http://www.w3.org/2000/svg"
              height="20px"
              width="20px"
              viewBox="0 -960 960 960"
            >
              <path d="m388-80-20-126q-19-7-40-19t-37-25l-118 54-93-164 108-79q-2-9-2.5-20.5T185-480q0-9 .5-20.5T188-521L80-600l93-164 118 54q16-13 37-25t40-18l20-127h184l20 126q19 7 40.5 18.5T669-710l118-54 93 164-108 77q2 10 2.5 21.5t.5 21.5q0 10-.5 21t-2.5 21l108 78-93 164-118-54q-16 13-36.5 25.5T592-206L572-80H388Zm92-270q54 0 92-38t38-92q0-54-38-92t-92-38q-54 0-92 38t-38 92q0 54 38 92t92 38Z" />
            </svg>
            {t("settings")}
          </summary>

          <div className="settingsRow">
            <span>
              {t("theme")}
              <small>{t("themeHint")}</small>
            </span>
            <div className="themePick">
              {THEMES.map((name) => (
                <button
                  key={name}
                  className={`swatch ${name} ${theme === name ? "active" : ""}`}
                  title={t(`theme${name[0].toUpperCase()}${name.slice(1)}`)}
                  aria-pressed={theme === name}
                  onClick={() => {
                    applyTheme(name);
                    setTheme(name);
                  }}
                />
              ))}
            </div>
          </div>

          <div className="settingsRow">
            <span>
              {t("interfaceLanguage")}
              <small>{t("languageHint")}</small>
            </span>
            {/* select, а не свои кнопки: список языков — это список,
                и на телефоне его рисует сама система */}
            <select
              className="langPick"
              value={lang}
              onChange={(e) => setLang(e.target.value)}
            >
              <option value="ru">Русский</option>
              <option value="en">English</option>
            </select>
          </div>

          <div className="settingsRow">
            <span>
              {t("exportAll")}
              <small>{t("exportHint")}</small>
            </span>
            <button
              className="secondary"
              onClick={exportAll}
            >
              {t("exportDo")}
            </button>
          </div>

          <div className="settingsRow">
            <span>
              {t("logout")}
              <small>{t("logoutHint")}</small>
            </span>
            <LogoutButton />
          </div>

          <div className="settingsRow">
            <span>
              {t("deleteAccount")}
              <small>{t("deleteAccountHint")}</small>
            </span>
            <button
              className="deleteAccount"
              onClick={deleteAccount}
            >
              {t("delete")}
            </button>
          </div>
        </details>

        {error && <p className="error">{error}</p>}
      </div>
    </div>
  );
};

export default UserPage;
