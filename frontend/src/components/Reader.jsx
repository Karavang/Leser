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
import { LoadingBook } from "../ui/Loading.jsx";
import { t } from "../i18n";
import { applyTheme, theme as saved, THEMES } from "../theme";

/// Читалка одна на все форматы: сервер отдаёт книгу главами с разметкой
/// (`/api/read`), и epub с fb2 отсюда неотличимы.
///
/// Страницы делает сам браузер: колоночная раскладка режет текст по высоте
/// экрана, а «перелистнуть» — это прокрутить ровно на ширину окна. Своей
/// разбивки на страницы нет и не нужно: браузер уже умеет верстать колонки.

const FONTS = [16, 18, 20, 22, 25, 28, 32];

/// Где читатель остановился — доля книги, а не номер страницы: страницы
/// разные при разном шрифте и размере окна, доля одна и та же.
const savedSpot = (filename) => {
  const user = JSON.parse(localStorage.getItem("user")) || {};
  const spot = (user.pages || []).find((p) => p.filename === filename);
  const frac = Number.parseFloat(spot?.page);
  // старые закладки — epubcfi от прежней читалки, числом не притворяются
  return Number.isFinite(frac) && frac >= 0 && frac < 1 ? frac : 0;
};

/// Одно слово — то, что кладут в словарь; всё остальное идёт в цитаты.
/// Дефис и апостроф внутри слова считаются частью слова: «во-первых»,
/// «don't», «l'homme» — это по-прежнему одно слово.
const ONE_WORD = /^[\p{L}\p{M}'’-]+$/u;

/// Сколько букв фразы сохраняется вокруг слова. Карточка без примера
/// бесполезна, а весь абзац в неё не влезет.
const CONTEXT = 120;

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
  const auth = { headers: { Authorization: `Bearer ${token}` } };

  const view = useRef(null);
  /// Открыть книгу на конкретном месте: так из кабинета открывается цитата.
  /// Своя закладка при этом не трогается — сюда пришли за другим.
  const openAt = Number.parseFloat(
    new URLSearchParams(location.search).get("at"),
  );
  const fromQuote = Number.isFinite(openAt) && openAt >= 0 && openAt < 1;
  /// доля книги, на которую надо встать после пересчёта страниц
  const spot = useRef(fromQuote ? openAt : savedSpot(filename));
  /// первая страница каждой главы — для оглавления и подписи в шапке
  const starts = useRef([]);
  const touch = useRef(0);
  /// листали ли в этот заход — см. сохранение закладки ниже
  const moved = useRef(false);
  /// Жест тачпада: листали ли уже на этом жесте. Ref, а не состояние эффекта:
  /// обработчик переподписывается на каждой странице, и хвост инерции
  /// от предыдущей пролистал бы дальше.
  const wheel = useRef({ done: false });

  const [book, setBook] = useState(null);
  const [error, setError] = useState(null);
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(1);
  const [toc, setToc] = useState(false);
  /// страница, с которой ушли по ссылке, — чтобы вернуться из сноски
  const [back, setBack] = useState(null);
  /// выделенное сейчас: {text, context} — из него делают цитату или карточку
  const [sel, setSel] = useState(null);
  /// короткий ответ на «сохранил» — гаснет сам
  const [said, setSaid] = useState(null);
  const [font, setFont] = useState(
    () => Number(localStorage.getItem("readerFont")) || 20,
  );
  const [theme, setTheme] = useState(saved);

  useEffect(() => {
    let alive = true;
    // Где читатель остановился, знает сервер, а не эта вкладка: сессия здесь
    // могла открыться вчера, а читали с телефона. Своя закладка остаётся
    // запасным вариантом — на случай, если сервер не ответит.
    const where = fromQuote
      ? Promise.resolve(null)
      : axios.get(`/api/progress/${filename}`, auth).catch(() => null);

    Promise.all([axios.get(`/api/read/${filename}`, auth), where])
      .then(([read, progress]) => {
        if (!alive) return;
        const at = Number.parseFloat(progress?.data?.page);
        if (Number.isFinite(at) && at >= 0 && at < 1) spot.current = at;
        setBook(read.data);
      })
      .catch(
        (e) =>
          alive &&
          setError(e.response?.data?.message || t("openFailed")),
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
    v.scrollLeft = at * step;
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
      moved.current = true;
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

  // Перелистывание двумя пальцами по тачпаду. Нативно прокрутить читалку
  // нельзя — у неё overflow: hidden, — поэтому горизонтальное колесо сами
  // превращаем в страницу.
  useEffect(() => {
    const v = view.current;
    if (!v) return;
    const onWheel = (e) => {
      // По вертикали не листаем: иначе обычная прокрутка мышью или чуть
      // косой жест уводили бы страницу без спросу.
      if (Math.abs(e.deltaX) <= Math.abs(e.deltaY)) return;
      // Без этого macOS понимает горизонтальный жест как «назад по истории»
      // и уносит из книги совсем.
      e.preventDefault();

      // За один жест тачпад шлёт десятки событий, а после того как пальцы
      // убрали, ещё секунду идёт хвост инерции. Листать надо один раз
      // на жест — значит, надо отличать новый жест от хвоста старого.
      //
      // Хвост дрожит, поэтому сравнивать соседние события бесполезно: любой
      // скачок вверх выглядит как новый жест. По времени тоже не развести —
      // пауза между жестами короче, чем подтормаживание на перерисовке.
      // Надёжно одно: касание гасит инерцию, и новый жест всегда начинается
      // с разгона — с сдвигов в пару пикселей, до которых хвост доходит уже
      // затухая и обратно не поднимается.
      const dx = Math.abs(e.deltaX);
      const g = wheel.current;

      if (dx <= 2) g.done = false;

      // Жест начинается с почти нулевых сдвигов — ждём первого заметного
      if (g.done || dx < 6) return;
      g.done = true;
      go(page + (e.deltaX > 0 ? 1 : -1));
    };

    // passive: false — иначе preventDefault не работает вовсе
    v.addEventListener("wheel", onWheel, { passive: false });
    return () => v.removeEventListener("wheel", onWheel);
  }, [go, page]);

  // На сервер закладка уходит с задержкой: иначе на каждое перелистывание
  // будет запрос, а листают подряд. У себя запоминаем сразу — иначе выход
  // из книги в первую же секунду терял бы страницу.
  useEffect(() => {
    if (!book || total <= 1) return;
    // Пришли из цитаты и ещё не листали — это заглянуть, а не читать.
    // Перебивать закладку, на которой человек остановился, за такое нельзя.
    if (fromQuote && !moved.current) return;
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

  /// Что выделили. Выделение уже сделал браузер — нам остаётся забрать текст
  /// и фразу вокруг него: сохранять слово без примера бессмысленно.
  const grab = () => {
    const selection = window.getSelection();
    const text = String(selection).replace(/\s+/g, " ").trim();
    if (!text) return setSel(null);

    const around = (selection.anchorNode?.parentElement?.textContent || "")
      .replace(/\s+/g, " ")
      .trim();
    const at = around.indexOf(text);
    setSel({
      text,
      context:
        at < 0
          ? around.slice(0, CONTEXT * 2)
          : around.slice(
              Math.max(0, at - CONTEXT),
              at + text.length + CONTEXT,
            ),
    });
    setSaid(null);
  };

  /// Цитата и слово отличаются только адресом и телом запроса — отправка одна.
  const keep = (url, body, ok) => {
    axios
      .post(url, { filename, ...body }, auth)
      .then(() => {
        setSaid(ok);
        setSel(null);
        window.getSelection().removeAllRanges();
      })
      .catch((e) => setSaid(e.response?.data?.message || t("saveFailed")))
      // ponytail: таймер не чистим — он гасит подпись, а размонтированной
      // читалке лишний setState ничего не ломает
      .finally(() => setTimeout(() => setSaid(null), 2500));
  };

  const pick = (size) => {
    setFont(size);
    localStorage.setItem("readerFont", String(size));
  };
  // Тема одна на всё приложение: кружок в читалке перекрашивает и библиотеку.
  const paint = (name) => {
    setTheme(name);
    applyTheme(name);
  };

  if (error)
    return (
      <div className={`reader ${theme}`}>
        <div className="readerFail">
          <p>{error}</p>
          <button onClick={close}>{t("toLibrary")}</button>
        </div>
      </div>
    );
  if (!book) return <LoadingBook theme={theme} />;

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
          title={t("toLibrary")}
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
            title={t("fontSmaller")}
            disabled={font <= FONTS[0]}
            onClick={() => pick(FONTS[FONTS.indexOf(font) - 1] ?? FONTS[0])}
          >
            A−
          </button>
          <button
            className="flat"
            title={t("fontBigger")}
            disabled={font >= FONTS[FONTS.length - 1]}
            onClick={() =>
              pick(FONTS[FONTS.indexOf(font) + 1] ?? FONTS[FONTS.length - 1])
            }
          >
            A+
          </button>
          {THEMES.map((name) => (
            <button
              key={name}
              className={`swatch ${name} ${theme === name ? "active" : ""}`}
              title={t(`theme${name[0].toUpperCase()}${name.slice(1)}`)}
              aria-pressed={theme === name}
              onClick={() => paint(name)}
            />
          ))}
          <button
            className={`flat ${toc ? "active" : ""}`}
            title={t("contents")}
            onClick={() => setToc(!toc)}
          >
            ☰
          </button>
        </div>
      </header>

      <div className="readerBody">
        <button
          className="flip left"
          title={t("prevPage")}
          disabled={page === 0}
          onClick={() => go(page - 1)}
        >
          ‹
        </button>
        <div
          className="readerViewport"
          ref={view}
          onClick={follow}
          onMouseUp={grab}
          onTouchStart={(e) => (touch.current = e.changedTouches[0].clientX)}
          onTouchEnd={(e) => {
            const moved = e.changedTouches[0].clientX - touch.current;
            if (Math.abs(moved) > 40) go(page + (moved < 0 ? 1 : -1));
            else grab();
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
          title={t("nextPage")}
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
                    {c.title || t("part", i + 1)}
                  </button>
                </li>
              ))}
            </ol>
          </nav>
        )}

        {/* Панель внизу, а не всплывашка у выделения: она никогда не закрывает
            то, что выделили, и не нуждается в расчёте координат. */}
        {sel && (
          <div className="readerSel">
            <span>{sel.text}</span>
            <button
              onClick={() =>
                keep(
                  "/api/quotes",
                  { text: sel.text, position: String(page / total) },
                  t("quoteSaved"),
                )
              }
            >
              {t("saveQuote")}
            </button>
            {ONE_WORD.test(sel.text) && (
              <button
                onClick={() =>
                  keep(
                    "/api/words",
                    { word: sel.text, context: sel.context },
                    t("wordSaved"),
                  )
                }
              >
                {t("saveWord")}
              </button>
            )}
            <button
              className="flat"
              title={t("close")}
              onClick={() => setSel(null)}
            >
              ×
            </button>
          </div>
        )}
        {said && <div className="readerSaid">{said}</div>}

        {back !== null && back !== page && (
          <button
            className="readerBack"
            onClick={() => {
              go(back);
              setBack(null);
            }}
          >
            {t("backToPage", back + 1)}
          </button>
        )}
      </div>

      <footer className="readerFoot">
        <input
          type="range"
          min={0}
          max={Math.max(0, total - 1)}
          value={page}
          aria-label={t("position")}
          onChange={(e) => go(Number(e.target.value))}
        />
        <span>
          {page + 1} / {total} · {percent}%
        </span>
      </footer>
    </div>
  );
};
