import axios from "axios";
import { useEffect, useState, useCallback, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { AddButton } from "./components/AddButton";
import { BooksList } from "./components/BooksList";
import { SearchingLine } from "./components/SearchingLine";
import { SourceResults } from "./components/SourceResults";
import UserButton from "./components/UserButton";
import UpButton from "./ui/UpButton";
import { Loading } from "./ui/Loading";
import { clearSession } from "./session";

export const Home = () => {
  const [books, setBooks] = useState([]);
  const [found, setFound] = useState([]);
  const [filter, setFilter] = useState("");
  const [tab, setTab] = useState("library");
  const [loading, setLoading] = useState(true);
  const [upButtonShow, setUpButtonShow] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);

  const navigate = useNavigate();
  const token = useMemo(() => JSON.parse(localStorage.getItem("token")), []);

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
      try {
        const response = await axios.get("/api/getAll", {
          params: filter.trim() ? { q: filter.trim() } : {},
          headers: { Authorization: `Bearer ${token}` },
        });
        setBooks(response.data);
      } catch (error) {
        // токен протух за 30 дней жизни — на вход, а не в пустой экран
        if (error.response?.status === 401) {
          clearSession();
          navigate("/login", { replace: true });
        }
      } finally {
        setLoading(false);
      }
    }, 250);
    return () => clearTimeout(timer);
  }, [token, filter, reloadKey, navigate]);

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
    () => (tab === "library" ? books : books.filter((book) => book.mine)),
    [books, tab],
  );

  // Список правит себя сам: сервер уже подтвердил удаление, перезапрашивать нечего.
  const onDeleted = useCallback((filename) => {
    setBooks((bs) => bs.filter((b) => b.filename !== filename));
  }, []);

  // То же с полкой: книга из библиотеки никуда не делась, поменялась только
  // отметка — и в разделе «мои книги» она сама пропадёт из выдачи.
  const onShelf = useCallback((filename, mine) => {
    setBooks((bs) =>
      bs.map((b) => (b.filename === filename ? { ...b, mine } : b)),
    );
  }, []);

  if (loading) return <Loading />;

  return (
    <div>
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
          <button
            className={tab === "library" ? "active" : ""}
            onClick={() => setTab("library")}
          >
            Библиотека
          </button>
          <button
            className={tab === "mine" ? "active" : ""}
            onClick={() => setTab("mine")}
          >
            Мои книги
          </button>
        </div>
      </div>
      <BooksList
        books={shownBooks}
        onDeleted={onDeleted}
        onShelf={onShelf}
      />
      {tab === "library" && (
        <SourceResults
          found={found}
          onImported={reload}
        />
      )}
      {upButtonShow && <UpButton />}
    </div>
  );
};
