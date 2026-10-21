import axios from "axios";
import { useEffect, useState, useCallback, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { AddButton } from "./components/AddButton";
import { BooksList } from "./components/BooksList";
import { ContinueReading } from "./components/ContinueReading";
import { SearchingLine } from "./components/SearchingLine";
import { SourceResults } from "./components/SourceResults";
import UserButton from "./components/UserButton";
import UpButton from "./ui/UpButton";
import { BusyLine, Loading } from "./ui/Loading";
import { expireSession } from "./session";
import { t } from "./i18n";
import { downloadedIds, useOffline } from "./offline";

const icon = (d) => (
  <svg
    aria-hidden="true"
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d={d} />
  </svg>
);
const STAR = icon("M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z");
const SHELF = icon("M4 5a2 2 0 0 1 2-2h5v18H6a2 2 0 0 1-2-2zM13 3h5a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-5z");
const PERSON = icon("M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21c1.5-4 4.5-6 8-6s6.5 2 8 6");

export const Home = () => {
  const [books, setBooks] = useState([]);
  const [found, setFound] = useState([]);
  const [filter, setFilter] = useState("");
  const [chosen, setTab] = useState(null);
  const [hasShelf, setHasShelf] = useState(false);
  const searching = Boolean(filter.trim());
  const tab = chosen ?? (hasShelf && !searching ? "mine" : "library");
  const [loading, setLoading] = useState(true);
  // повторные запросы — поиск, вкладка, возврат — идут поверх показанного списка
  const [busy, setBusy] = useState(false);
  const [upButtonShow, setUpButtonShow] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);

  const navigate = useNavigate();
  const token = useMemo(() => JSON.parse(localStorage.getItem("token")), []);

  const offline = useOffline() === "offline";

  const reload = useCallback(() => setReloadKey((n) => n + 1), []);

  const handleScroll = useCallback(() => {
    setUpButtonShow(window.scrollY > 64);
  }, []);

  useEffect(() => {
    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, [handleScroll]);

  // Ищет бэк: по названию, автору и серии сразу, в любом языке. Здесь только
  // пауза перед запросом, чтобы не слать его на каждую нажатую букву.
  useEffect(() => {
    const timer = setTimeout(async () => {
      setBusy(true);
      try {
        const response = await axios.get("/api/getAll", {
          params: filter.trim() ? { q: filter.trim() } : {},
          headers: { Authorization: `Bearer ${token}` },
        });
        let list = response.data;
        if (offline) {
          const have = await downloadedIds();
          list = list.filter((book) => have.has(String(book.id)));
        }
        setBooks(list);
        if (!filter.trim()) setHasShelf(list.some((book) => book.mine));
      } catch (error) {
        // токен протух за 30 дней жизни — на вход, а не в пустой экран
        if (error.response?.status === 401) {
          expireSession();
          navigate("/login", { replace: true });
        }
      } finally {
        setLoading(false);
        setBusy(false);
      }
    }, 250);
    return () => clearTimeout(timer);
  }, [token, filter, reloadKey, navigate, offline]);

  // Внешние источники — отдельным запросом, а не вместе со своей библиотекой:
  // сеть чужая, отвечает медленнее и может не ответить вовсе, а свои книги
  // должны появиться сразу. Без запроса в поиске искать снаружи нечего.
  useEffect(() => {
    const q = filter.trim();
    if (!q) {
      setFound([]);
      return;
    }
    let stale = false;
    const timer = setTimeout(async () => {
      try {
        const response = await axios.get("/api/searchSources", {
          params: { q },
          headers: { Authorization: `Bearer ${token}` },
        });
        // ответ на устаревший запрос не должен перебивать свежий
        if (!stale) setFound(response.data);
      } catch {
        if (!stale) setFound([]);
      }
    }, 400);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [token, filter]);

  // Каталог общий, книгу мог добавить и другой читатель. Перечитываем при
  // возврате на вкладку: это ловит и свои загрузки, и чужие, и не стоит
  // ни одного запроса, пока вкладка не на виду.
  // ponytail: не живая лента — книга, добавленная прямо сейчас в соседнем окне,
  // появится при следующем переключении. Понадобится живое — SSE на /events,
  // бэк уже знает момент вставки в put_new_one.
  useEffect(() => {
    const onBack = () => document.visibilityState === "visible" && reload();
    window.addEventListener("focus", onBack);
    document.addEventListener("visibilitychange", onBack);
    return () => {
      window.removeEventListener("focus", onBack);
      document.removeEventListener("visibilitychange", onBack);
    };
  }, [reload]);

  // Раздел — единственное, что осталось фильтровать на клиенте: mine приходит
  // с бэка вместе с книгой, отдельный запрос ради этого не нужен.
  const shownBooks = useMemo(
    () =>
      tab === "library"
        ? books
        : books
            .filter((book) => book.mine)
            .sort((a, b) =>
              searching ? 0 : (b.readAt ?? "").localeCompare(a.readAt ?? ""),
            ),
    [books, tab, searching],
  );
  const recent = useMemo(
    () =>
      books
        .filter((book) => book.readAt)
        .sort((a, b) => b.readAt.localeCompare(a.readAt))
        .slice(0, 3),
    [books],
  );
  const pick = (next) => () => {
    setTab(next);
    window.scrollTo(0, 0);
  };
  const sections = [
    ["mine", t("tabMine")],
    ["library", t("tabLibrary")],
  ];

  // Список правит себя сам: сервер уже подтвердил удаление, перезапрашивать нечего.
  const onDeleted = useCallback((id) => {
    setBooks((bs) => bs.filter((b) => b.id !== id));
  }, []);

  // То же с полкой: книга из библиотеки никуда не делась, поменялась только
  // отметка — и в разделе «мои книги» она сама пропадёт из выдачи.
  const onChange = useCallback((id, fields) => {
    setBooks((bs) =>
      bs.map((b) => (b.id === id ? { ...b, ...fields } : b)),
    );
  }, []);

  return (
    <div className="home">
      <nav className="sideNav" aria-label={t("tabLibrary")}>
        <b>Leser</b>
        {sections.map(([key, label]) => (
          <button
            key={key}
            className={tab === key ? "active" : ""}
            aria-current={tab === key ? "page" : undefined}
            onClick={pick(key)}
          >
            {label}
          </button>
        ))}
        <button onClick={() => navigate("/me")}>{t("quotes")}</button>
        <button onClick={() => navigate("/me")}>{t("dictionary")}</button>
        <button onClick={() => navigate("/me")}>{t("profile")}</button>
      </nav>
      {busy && !loading && <BusyLine />}
      {/* Всё, кроме списка, остаётся на экране при прокрутке. Одна липкая
          обёртка вместо двух: иначе второй пришлось бы вручную считать
          отступ на высоту первой. */}
      <div className="topBar">
        <div className="searchAndAdd">
          <SearchingLine setFilter={setFilter} />
          <AddButton onAdded={reload} />
          <UserButton />
        </div>
        <div className="tabs">
          {sections.map(([key, label]) => (
            <button
              key={key}
              className={tab === key ? "active" : ""}
              onClick={pick(key)}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      {tab === "mine" && !searching && <ContinueReading books={recent} />}
      {loading ? (
        <Loading />
      ) : (
        <BooksList
          books={shownBooks}
          onDeleted={onDeleted}
          onChange={onChange}
        />
      )}
      {tab === "library" && (
        <SourceResults
          found={found}
          onImported={reload}
        />
      )}
      {upButtonShow && <UpButton />}
      <nav className="bottomNav" aria-label={t("tabLibrary")}>
        {sections.map(([key, label]) => (
          <button
            key={key}
            className={tab === key ? "active" : ""}
            aria-current={tab === key ? "page" : undefined}
            onClick={pick(key)}
          >
            {key === "mine" ? STAR : SHELF}
            {label}
          </button>
        ))}
        <button onClick={() => navigate("/me")}>
          {PERSON}
          {t("profile")}
        </button>
      </nav>
    </div>
  );
};
