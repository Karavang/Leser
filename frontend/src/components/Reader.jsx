import axios from "axios";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { Loading } from "../ui/Loading.jsx";

/// Читалка одна на все форматы: сервер отдаёт книгу главами с разметкой
/// (`/api/read`), и epub с fb2 отсюда неотличимы.
///
/// Страницы делает сам браузер: колоночная раскладка режет текст по высоте
/// экрана, а «перелистнуть» — это прокрутить ровно на ширину окна. Своей
/// разбивки на страницы нет и не нужно: браузер уже умеет верстать колонки.

const FONTS = [16, 18, 20, 22, 25, 28, 32];
const THEMES = { day: "День", sepia: "Сепия", night: "Ночь" };

/// Где читатель остановился — доля книги, а не номер страницы: страницы
/// разные при разном шрифте и размере окна, доля одна и та же.
const savedSpot = (filename) => {
  const user = JSON.parse(localStorage.getItem("user")) || {};
  const spot = (user.pages || []).find((p) => p.filename === filename);
  const frac = Number.parseFloat(spot?.page);
  // старые закладки — epubcfi от прежней читалки, числом не притворяются
  return Number.isFinite(frac) && frac >= 0 && frac < 1 ? frac : 0;
};

const rememberSpot = (filename, frac) => {
  const user = JSON.parse(localStorage.getItem("user"));
  if (!user) return;
  user.pages = (user.pages || []).filter((p) => p.filename !== filename);
  user.pages.push({ filename, page: String(frac) });
  localStorage.setItem("user", JSON.stringify(user));
};

export const Reader = () => {
  const { filename } = useParams();
  const navigate = useNavigate();
  const location = useLocation();
  const token = JSON.parse(localStorage.getItem("token"));

  const view = useRef(null);
  /// доля книги, на которую надо встать после пересчёта страниц
  const spot = useRef(savedSpot(filename));
  /// первая страница каждой главы — для оглавления и подписи в шапке
  const starts = useRef([]);
  const touch = useRef(0);

  const [book, setBook] = useState(null);
  const [error, setError] = useState(null);
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(1);
  const [toc, setToc] = useState(false);
  /// страница, с которой ушли по ссылке, — чтобы вернуться из сноски
  const [back, setBack] = useState(null);
  const [font, setFont] = useState(
    () => Number(localStorage.getItem("readerFont")) || 20,
  );
  const [theme, setTheme] = useState(
    () => localStorage.getItem("readerTheme") || "sepia",
  );

  useEffect(() => {
    let alive = true;
    axios
      .get(`/api/read/${filename}`, {
        headers: { Authorization: `Bearer ${token}` },
      })
      .then((r) => alive && setBook(r.data))
      .catch(
        (e) =>
          alive &&
          setError(e.response?.data?.message || "Не удалось открыть книгу"),
      );
    return () => {
      alive = false;
    };
    // token лежит в localStorage и меняется только вместе с сессией
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filename]);

  // Вся книга одной строкой: колонки должны быть сквозными, иначе главу
  // не перелистнёшь в следующую, а оглавление станет набором отдельных полос.
  const html = useMemo(
    () =>
      (book?.chapters || []).map((c) => `<section>${c.html}</section>`).join(""),
    [book],
  );

  /// Номер страницы, на которую попал элемент. Округлять нельзя, только
  /// вниз: на широком экране страница — это разворот из двух колонок, и то,
  /// что лежит в правой, по округлению уехало бы на страницу вперёд.
  /// Пара пикселей запаса — на дробные размеры колонок.
  const pageOf = (el) => {
    const v = view.current;
    const left =
      el.getBoundingClientRect().left - v.getBoundingClientRect().left;
    return Math.floor((left + v.scrollLeft + 2) / v.clientWidth);
  };

  /// Пересчёт страниц. Нужен после загрузки книги и после любого изменения
  /// размера — шрифта или самого окна.
  const measure = useCallback(() => {
    const v = view.current;
    if (!v || !v.clientWidth) return;
    const step = v.clientWidth;

    const box = v.getBoundingClientRect();
    starts.current = [...v.querySelectorAll("section")].map((s) =>
      Math.floor(
        (s.getBoundingClientRect().left - box.left + v.scrollLeft + 2) / step,
      ),
    );

    const count = Math.max(1, Math.round(v.scrollWidth / step));
    const at = Math.min(count - 1, Math.round(spot.current * count));
    setTotal(count);
    setPage(at);
    // без auto прыжок к сохранённому месту едет плавной прокруткой через всю книгу
    v.style.scrollBehavior = "auto";
    v.scrollLeft = at * step;
    v.style.scrollBehavior = "";
  }, []);

  useLayoutEffect(measure, [html, font, measure]);

  useEffect(() => {
    const v = view.current;
    if (!v) return;
    const ro = new ResizeObserver(measure);
    ro.observe(v);
    return () => ro.disconnect();
  }, [measure, html]);

  // Картинка занимает место только после того, как загрузится, а размер окна
  // при этом не меняется — сам по себе ResizeObserver этого не заметит,
  // и число страниц осталось бы посчитанным по книге без картинок.
  useEffect(() => {
    const v = view.current;
    if (!v) return;
    const waiting = [...v.querySelectorAll("img")].filter((i) => !i.complete);
    if (!waiting.length) return;
    let left = waiting.length;
    const done = () => --left === 0 && measure();
    waiting.forEach((i) => {
      i.addEventListener("load", done);
      i.addEventListener("error", done);
    });
    return () =>
      waiting.forEach((i) => {
        i.removeEventListener("load", done);
        i.removeEventListener("error", done);
      });
  }, [html, measure]);

  const go = useCallback(
    (to) => {
      const v = view.current;
      if (!v) return;
      const at = Math.max(0, Math.min(total - 1, to));
      v.scrollLeft = at * v.clientWidth;
      setPage(at);
      spot.current = at / total;
    },
    [total],
  );

  const close = () =>
    location.key === "default" ? navigate("/", { replace: true }) : navigate(-1);

  /// Ссылки внутри книги: оглавление в аннотации, сноски, перекрёстные ссылки.
  /// Обычный переход по якорю прокрутил бы колонки мимо страниц, поэтому
  /// считаем страницу цели сами. Куда вернуться — помним: сноску читают
  /// и возвращаются в текст.
  const follow = (e) => {
    const link = e.target.closest('a[href^="#"]');
    if (!link) return;
    e.preventDefault();
    const target = view.current?.querySelector(
      `[id="${CSS.escape(link.getAttribute("href").slice(1))}"]`,
    );
    if (!target) return;
    setBack(page);
    go(pageOf(target));
  };

  useEffect(() => {
    const onKey = (e) => {
      if (e.key === "Escape") return toc ? setToc(false) : close();
      if (["ArrowRight", "PageDown", " "].includes(e.key)) {
        e.preventDefault();
        go(page + 1);
      } else if (["ArrowLeft", "PageUp"].includes(e.key)) go(page - 1);
      else if (e.key === "Home") go(0);
      else if (e.key === "End") go(total - 1);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // close меняется каждый рендер, но делает всегда одно и то же
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [go, page, total, toc]);

  // На сервер закладка уходит с задержкой: иначе на каждое перелистывание
  // будет запрос, а листают подряд. У себя запоминаем сразу — иначе выход
  // из книги в первую же секунду терял бы страницу.
  useEffect(() => {
    if (!book || total <= 1) return;
    const frac = page / total;
    rememberSpot(filename, frac);
    const id = setTimeout(() => {
      axios
        .post(
          "/api/pageWasFlipped",
          { filename, page: String(frac) },
          { headers: { Authorization: `Bearer ${token}` } },
        )
        .catch(() => {});
    }, 800);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page, total, book, filename]);

  const pick = (size) => {
    setFont(size);
    localStorage.setItem("readerFont", String(size));
  };
  const paint = (name) => {
    setTheme(name);
    localStorage.setItem("readerTheme", name);
  };

  if (error)
    return (
      <div className={`reader ${theme}`}>
        <div className="readerFail">
          <p>{error}</p>
          <button onClick={close}>К библиотеке</button>
        </div>
      </div>
    );
  if (!book) return <Loading />;

  let chapter = 0;
  starts.current.forEach((at, i) => {
    if (at <= page) chapter = i;
  });
  const percent = total > 1 ? Math.round((page / (total - 1)) * 100) : 100;

  return (
    <div className={`reader ${theme}`}>
      <header className="readerBar">
        <button
          className="flat"
          onClick={close}
          title="К библиотеке"
        >
          ←
        </button>
        <div className="readerWhat">
          <b>{book.title}</b>
          <span>{book.chapters[chapter]?.title || book.author}</span>
        </div>
        <div className="readerTools">
          <button
            className="flat"
            title="Меньше шрифт"
            disabled={font <= FONTS[0]}
            onClick={() => pick(FONTS[FONTS.indexOf(font) - 1] ?? FONTS[0])}
          >
            A−
          </button>
          <button
            className="flat"
            title="Больше шрифт"
            disabled={font >= FONTS[FONTS.length - 1]}
            onClick={() =>
              pick(FONTS[FONTS.indexOf(font) + 1] ?? FONTS[FONTS.length - 1])
            }
          >
            A+
          </button>
          {Object.entries(THEMES).map(([name, label]) => (
            <button
              key={name}
              className={`swatch ${name} ${theme === name ? "active" : ""}`}
              title={label}
              onClick={() => paint(name)}
            />
          ))}
          <button
            className={`flat ${toc ? "active" : ""}`}
            title="Оглавление"
            onClick={() => setToc(!toc)}
          >
            ☰
          </button>
        </div>
      </header>

      <div className="readerBody">
        <button
          className="flip left"
          title="Назад"
          disabled={page === 0}
          onClick={() => go(page - 1)}
        >
          ‹
        </button>
        <div
          className="readerViewport"
          ref={view}
          onClick={follow}
          onTouchStart={(e) => (touch.current = e.changedTouches[0].clientX)}
          onTouchEnd={(e) => {
            const moved = e.changedTouches[0].clientX - touch.current;
            if (Math.abs(moved) > 40) go(page + (moved < 0 ? 1 : -1));
          }}
        >
          {/* Разметку собирает сервер: теги только из его таблицы, весь текст
              экранирован, скрипты и атрибуты выброшены. Подробности — в
              api/src/reader.rs. */}
          <div
            className="readerPages"
            // язык книги включает переносы: без него hyphens: auto не знает,
            // по какому словарю ломать слово, и текст в узкой колонке рвётся
            // огромными пробелами
            lang={book.lang || undefined}
            style={{ "--fs": `${font}px` }}
            dangerouslySetInnerHTML={{ __html: html }}
          />
        </div>
        <button
          className="flip right"
          title="Дальше"
          disabled={page >= total - 1}
          onClick={() => go(page + 1)}
        >
          ›
        </button>

        {toc && (
          <nav className="readerToc">
            <ol>
              {book.chapters.map((c, i) => (
                <li key={i}>
                  <button
                    className={i === chapter ? "active" : ""}
                    onClick={() => {
                      go(starts.current[i] ?? 0);
                      setToc(false);
                    }}
                  >
                    {c.title || `Часть ${i + 1}`}
                  </button>
                </li>
              ))}
            </ol>
          </nav>
        )}

        {back !== null && back !== page && (
          <button
            className="readerBack"
            onClick={() => {
              go(back);
              setBack(null);
            }}
          >
            ← назад к странице {back + 1}
          </button>
        )}
      </div>

      <footer className="readerFoot">
        <input
          type="range"
          min={0}
          max={Math.max(0, total - 1)}
          value={page}
          aria-label="Положение в книге"
          onChange={(e) => go(Number(e.target.value))}
        />
        <span>
          {page + 1} / {total} · {percent}%
        </span>
      </footer>
    </div>
  );
};
