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
import { isOffline, useOffline } from "../offline";
import { sendPage, unsentPage } from "../sync";
import { applyTheme, theme as saved, THEMES } from "../theme";

/// Читалка одна на все форматы: сервер отдаёт книгу главами с разметкой
/// (`/api/read`), и epub с fb2 отсюда неотличимы.
///
/// Страницы делает сам браузер: колоночная раскладка режет текст по высоте
/// экрана, а «перелистнуть» — это прокрутить ровно на ширину окна. Своей
/// разбивки на страницы нет и не нужно: браузер уже умеет верстать колонки.

const SIZE = [14, 36];

const TOUCH = matchMedia("(pointer: coarse), (max-width: 720px)").matches;

const clamp = (lo, x, hi) => Math.min(hi, Math.max(lo, x));

// ponytail: read once per page load; changing Dynamic Type or the browser font size mid-session applies on the next open
const IOS =
  navigator.maxTouchPoints > 0 && /iPhone|iPad|Macintosh/.test(navigator.userAgent);

const deviceScale = () => {
  const probe = document.createElement("p");
  probe.style.font = "-apple-system-body";
  if (!IOS || !probe.style.font) {
    const root = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
    return clamp(0.85, (root || 16) / 16, 1.6);
  }
  document.body.append(probe);
  const px = Number.parseFloat(getComputedStyle(probe).fontSize) || 17;
  probe.remove();
  return clamp(0.85, px / 17, 1.6);
};

const SCALE = deviceScale();
const SHORT = Math.min(screen.width, screen.height);
const TABLET = TOUCH && SHORT >= 600;
const UI = clamp(0.9, SCALE * (TABLET ? 1.15 : 1), 1.5);
const baseSize = () =>
  Math.round(clamp(SIZE[0], (TOUCH ? clamp(17, 14 + SHORT / 120, 22) : 20) * SCALE, SIZE[1]));

const FAMILIES = {
  serif: '"Iowan Old Style", "Palatino Linotype", Georgia, "Times New Roman", serif',
  book: 'Charter, "Bitstream Charter", "Sitka Text", Cambria, Georgia, serif',
  sans: '-apple-system, system-ui, "Segoe UI", Roboto, "Helvetica Neue", sans-serif',
};
const LEADING = [1.3, 1.55, 1.8, 2.1];
const MARGIN = [8, 16, 32, 56];
const VMARGIN = [8, 24, 48];

const freshLook = () => ({
  family: "serif",
  size: baseSize(),
  leading: 1.55,
  margin: TOUCH ? 16 : 32,
  vmargin: 24,
  align: "justify",
  hyphens: "on",
  indent: "on",
  cols: "auto",
});

// ponytail: trusts whatever is in readerLook; a junk value only yields a bad CSS value, reset fixes it
const savedLook = () => {
  let look = {};
  try {
    look = JSON.parse(localStorage.getItem("readerLook")) || {};
  } catch {
    look = {};
  }
  const old = Number(localStorage.getItem("readerFont"));
  return { ...freshLook(), ...(old ? { size: old } : {}), ...look };
};

/// Где читатель остановился — доля книги, а не номер страницы: страницы
/// разные при разном шрифте и размере окна, доля одна и та же.
const savedSpot = (id) => {
  const user = JSON.parse(localStorage.getItem("user")) || {};
  const spot = (user.pages || []).find((p) => p.id === id);
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

const sizes = (chapters) => chapters.map((c) => c.html.length || 1);

const locate = (chapters, frac) => {
  const len = sizes(chapters);
  let left = frac * len.reduce((a, b) => a + b, 0);
  for (let i = 0; i < len.length - 1; i++) {
    if (left < len[i]) return [i, left / len[i]];
    left -= len[i];
  }
  return [len.length - 1, Math.min(1, left / len[len.length - 1])];
};

const pageOf = (v, el) => {
  const left = el.getBoundingClientRect().left - v.getBoundingClientRect().left;
  return Math.floor((left + v.scrollLeft + 2) / v.clientWidth);
};

const rememberSpot = (id, frac) => {
  const user = JSON.parse(localStorage.getItem("user"));
  if (!user) return;
  user.pages = (user.pages || []).filter((p) => p.id !== id);
  user.pages.push({ id, page: String(frac) });
  localStorage.setItem("user", JSON.stringify(user));
};

export const Reader = () => {
  const { id } = useParams();
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
  const spot = useRef(fromQuote ? openAt : savedSpot(id));
  const inner = useRef(0);
  const aim = useRef(null);
  const rate = useRef({ basis: 0, perChar: 0 });
  const chRef = useRef(0);
  const touch = useRef(0);
  /// листали ли в этот заход — см. сохранение закладки ниже
  const moved = useRef(false);
  /// Жест тачпада: листали ли уже на этом жесте. Ref, а не состояние эффекта:
  /// обработчик переподписывается на каждой странице, и хвост инерции
  /// от предыдущей пролистал бы дальше.
  const wheel = useRef({ done: false });

  const [book, setBook] = useState(null);
  const [error, setError] = useState(null);
  const [ch, setCh] = useState(0);
  const [page, setPage] = useState(0);
  const [laid, setLaid] = useState({ ch: -1, n: 0 });
  const count = laid.ch === ch ? laid.n : 0;
  chRef.current = ch;
  const [toc, setToc] = useState(false);
  /// страница, с которой ушли по ссылке, — чтобы вернуться из сноски
  const [back, setBack] = useState(null);
  /// выделенное сейчас: {text, context} — из него делают цитату или карточку
  const [sel, setSel] = useState(null);
  /// короткий ответ на «сохранил» — гаснет сам
  const [said, setSaid] = useState(null);
  const [look, setLook] = useState(savedLook);
  const [lookOpen, setLookOpen] = useState(false);
  const offline = useOffline() === "offline";
  const [theme, setTheme] = useState(saved);
  const [bars, setBars] = useState(false);
  /// иллюстрации: у книги-альбома они и есть книга, в дороге — лишние
  /// мегабайты. Выбор помнится на устройстве, как шрифт и тема.
  const [pics, setPics] = useState(
    () => localStorage.getItem("readerPics") !== "off",
  );
  /// картинки иллюстрированной книги лежат отдельно и едут по одной:
  /// заголовков у <img> нет, поэтому доступ даёт короткий токен в адресе
  const [imgToken, setImgToken] = useState(null);

  useEffect(() => {
    let alive = true;
    // Где читатель остановился, знает сервер, а не эта вкладка: сессия здесь
    // могла открыться вчера, а читали с телефона. Своя закладка остаётся
    // запасным вариантом — на случай, если сервер не ответит.
    const where = fromQuote || isOffline()
      ? Promise.resolve(null)
      : axios.get(`/api/progress/${id}`, auth).catch(() => null);

    // no-cache в запросе — «сверь с сервером»: у кого книга лежала в кэше
    // с прежним годовым immutable, тот иначе новый разбор не увидит никогда.
    // Сервер отвечает 304 по ETag, тело едет только при новой версии.
    const fresh = {
      headers: { ...auth.headers, "Cache-Control": "no-cache" },
    };
    Promise.all([axios.get(`/api/read/${id}`, fresh), where])
      .then(([read, progress]) => {
        if (!alive) return;
        const at = Number.parseFloat(unsentPage(id) ?? progress?.data?.page);
        if (Number.isFinite(at) && at >= 0 && at < 1) spot.current = at;
        const [i, within] = locate(read.data.chapters || [], spot.current);
        aim.current = { inner: within };
        setCh(i);
        setBook(read.data);
        if (read.data.chapters?.some((c) => c.html.includes('src="img/'))) {
          axios
            .get(`/api/imgToken/${id}`, auth)
            .then((r) => alive && setImgToken(r.data.token))
            .catch(() => {});
        }
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
  }, [id]);

  // ponytail: only the current chapter is in the DOM, old e-ink WebKit chokes on a whole book in one multicol box; a single-chapter book is still laid out whole
  const html = useMemo(() => {
    const body = book?.chapters?.[ch]?.html ?? "";
    return `<section>${body.replace(/src="img\/(\d+)"/g, (whole, n) =>
      pics && imgToken ? `src="/api/img/${id}/${n}?t=${imgToken}"` : "hidden",
    )}</section>`;
  }, [book, ch, pics, imgToken, id]);

  const len = useMemo(() => sizes(book?.chapters || []), [book]);
  const cum = useMemo(
    () => len.reduce((acc, n) => [...acc, acc[acc.length - 1] + n], [0]),
    [len],
  );
  const last = len.length - 1;
  const bookLen = cum[cum.length - 1] || 1;

  const measure = useCallback(() => {
    const v = view.current;
    if (!v || !v.clientWidth) return;
    const step = v.clientWidth;
    v.style.setProperty("--end", "0px");
    // ponytail: ceil, not round — a half-filled last spread on ultrawide gives ~1.49 and round dropped it; 0.02 absorbs subpixel overflow
    const n = Math.max(1, Math.ceil(v.scrollWidth / step - 0.02));
    v.style.setProperty("--end", `${n * step}px`);
    if (rate.current.step !== step) rate.current = { basis: 0, perChar: 0, step };
    const want = aim.current;
    aim.current = null;
    let at;
    if (want?.id) {
      const el = v.querySelector(`[id="${CSS.escape(want.id)}"]`);
      at = el ? pageOf(v, el) : 0;
    } else if (want?.end) at = n - 1;
    else at = Math.round((want ? want.inner : inner.current) * n);
    at = Math.max(0, Math.min(n - 1, at));
    setLaid({ ch: chRef.current, n });
    setPage(at);
    inner.current = at / n;
    v.scrollLeft = at * step;
  }, []);

  useLayoutEffect(() => {
    rate.current = { basis: 0, perChar: 0 };
  }, [look]);
  useLayoutEffect(measure, [html, ch, look, measure]);
  useEffect(() => {
    localStorage.setItem("readerLook", JSON.stringify(look));
  }, [look]);

  useEffect(() => {
    const v = view.current;
    if (!v) return;
    if (!window.ResizeObserver) {
      window.addEventListener("resize", measure);
      return () => window.removeEventListener("resize", measure);
    }
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
      if (to < 0 && ch > 0) {
        aim.current = { end: true };
        return setCh(ch - 1);
      }
      if (to >= count && ch < last) {
        aim.current = { inner: 0 };
        return setCh(ch + 1);
      }
      const at = Math.max(0, Math.min(count - 1, to));
      v.scrollLeft = at * v.clientWidth;
      setPage(at);
      inner.current = at / count;
    },
    [ch, count, last],
  );

  const jump = (i, want) => {
    moved.current = true;
    aim.current = want;
    if (i === ch) measure();
    else setCh(i);
  };

  // ponytail: page totals are estimated from the longest chapter laid out so far; they settle as the reader moves through the book
  if (count && len[ch] >= rate.current.basis) {
    rate.current = { ...rate.current, basis: len[ch], perChar: count / len[ch] };
  }
  const perChar = rate.current.perChar;
  const startOf = (i) => Math.round(cum[i] * perChar);
  const here = startOf(ch) + page;
  const total = Math.max(startOf(ch) + count, Math.round(bookLen * perChar), 1);
  const frac = count ? (cum[ch] + (page / count) * len[ch]) / bookLen : 0;

  const close = () =>
    location.key === "default" ? navigate("/", { replace: true }) : navigate(-1);

  /// Ссылки внутри книги: оглавление в аннотации, сноски, перекрёстные ссылки.
  /// Обычный переход по якорю прокрутил бы колонки мимо страниц, поэтому
  /// считаем страницу цели сами. Куда вернуться — помним: сноску читают
  /// и возвращаются в текст.
  const follow = (e) => {
    if (toc) return setToc(false);
    const link = e.target.closest('a[href^="#"]');
    if (!link) {
      if (String(window.getSelection()).trim()) return;
      if (!TOUCH || bars) return setBars(!bars);
      // ponytail: Kindle zones — top 15% menu, left 30% back, rest forward; hardcoded ratios, make them settings if readers ask
      const box = view.current.getBoundingClientRect();
      const x = (e.clientX - box.left) / box.width;
      const y = (e.clientY - box.top) / box.height;
      if (y < 0.15) setBars(true);
      else go(page + (x < 0.3 ? -1 : 1));
      return;
    }
    e.preventDefault();
    const target = link.getAttribute("href").slice(1);
    const i = view.current.querySelector(`[id="${CSS.escape(target)}"]`)
      ? ch
      : book.chapters.findIndex((c) => c.html.includes(`id="${target}"`));
    if (i < 0) return;
    setBack({ ch, inner: page / count, at: here });
    jump(i, { id: target });
  };

  useEffect(() => {
    const onKey = (e) => {
      if (e.key === "Escape") {
        if (toc) return setToc(false);
        return lookOpen ? setLookOpen(false) : close();
      }
      if (e.target.closest?.("input")) return;
      if (["ArrowRight", "PageDown", " "].includes(e.key)) {
        e.preventDefault();
        go(page + 1);
      } else if (["ArrowLeft", "PageUp"].includes(e.key)) go(page - 1);
      else if (e.key === "Home") jump(0, { inner: 0 });
      else if (e.key === "End") jump(last, { end: true });
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // close меняется каждый рендер, но делает всегда одно и то же
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [go, page, last, toc, lookOpen]);

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
    if (!book || !count) return;
    // Пришли из цитаты и ещё не листали — это заглянуть, а не читать.
    // Перебивать закладку, на которой человек остановился, за такое нельзя.
    if (fromQuote && !moved.current) return;
    rememberSpot(id, frac);
    const timer = setTimeout(() => sendPage(id, String(frac)), 800);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [frac, book, id]);

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
      .post(url, { book: id, ...body }, auth)
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

  const set = (key, value) => setLook((l) => ({ ...l, [key]: value }));
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

  const percent = Math.round(frac * 100);

  const hasPics = book.chapters?.some((c) => c.html.includes('src="img/'));

  const seg = (key, options, label) => (
    <div className="seg">
      {options.map((o) => (
        <button
          key={o}
          className={look[key] === o ? "active" : ""}
          aria-pressed={look[key] === o}
          title={String(o)}
          onClick={() => set(key, o)}
        >
          {label(o)}
        </button>
      ))}
    </div>
  );
  const bars3 = <><b /><b /><b /></>;

  const panel = (
    <div className="readerLook" role="dialog" aria-label={t("look")}>
      <header>
        <b>{t("look")}</b>
        <button
          className="tocClose"
          title={t("close")}
          aria-label={t("close")}
          onClick={() => setLookOpen(false)}
        >
          ×
        </button>
      </header>
      <div className="lookRow">
        <span>{t("theme")}</span>
        <div className="seg">
          {THEMES.map((name) => (
            <button
              key={name}
              className={`swatch ${name} ${theme === name ? "active" : ""}`}
              title={t(`theme${name[0].toUpperCase()}${name.slice(1)}`)}
              aria-pressed={theme === name}
              onClick={() => paint(name)}
            />
          ))}
        </div>
      </div>
      <div className="lookRow">
        <span>
          {t("lookSize")} · {look.size}px
        </span>
        <div className="lookSize">
          <button
            aria-label={`${t("lookSize")} −`}
            disabled={look.size <= SIZE[0]}
            onClick={() => set("size", Math.max(SIZE[0], look.size - 1))}
          >
            <small>A</small>
          </button>
          <input
            type="range"
            aria-label={t("lookSize")}
            min={SIZE[0]}
            max={SIZE[1]}
            value={look.size}
            onChange={(e) => set("size", Number(e.target.value))}
          />
          <button
            aria-label={`${t("lookSize")} +`}
            disabled={look.size >= SIZE[1]}
            onClick={() => set("size", Math.min(SIZE[1], look.size + 1))}
          >
            <big>A</big>
          </button>
        </div>
      </div>
      <div className="lookRow">
        <span>{t("lookFont")}</span>
        {seg("family", Object.keys(FAMILIES), (f) => (
          <span style={{ fontFamily: FAMILIES[f] }}>
            {t(`font${f[0].toUpperCase()}${f.slice(1)}`)}
          </span>
        ))}
      </div>
      <div className="lookRow">
        <span>{t("lookLeading")}</span>
        {seg("leading", LEADING, (v) => (
          <i className="glyph" style={{ "--gap": `${(v - 1) * 9}px` }}>
            {bars3}
          </i>
        ))}
      </div>
      <div className="lookRow">
        <span>{t("lookMargin")}</span>
        {seg("margin", MARGIN, (v) => (
          <i className="glyph page" style={{ "--side": `${v / 8}px` }}>
            {bars3}
          </i>
        ))}
      </div>
      <div className="lookRow">
        <span>{t("lookVmargin")}</span>
        {seg("vmargin", VMARGIN, (v) => (
          <i className="glyph page" style={{ "--top": `${v / 8}px` }}>
            {bars3}
          </i>
        ))}
      </div>
      <div className="lookRow">
        <span>{t("lookAlign")}</span>
        {seg("align", ["justify", "left"], (a) => (
          <i className={`glyph ${a}`} aria-label={t(`align${a[0].toUpperCase()}${a.slice(1)}`)}>
            {bars3}
          </i>
        ))}
      </div>
      <div className="lookRow">
        <span>{t("lookHyph")}</span>
        {seg("hyphens", ["on", "off"], t)}
      </div>
      <div className="lookRow">
        <span>{t("lookIndent")}</span>
        {seg("indent", ["on", "off"], t)}
      </div>
      <div className="lookRow cols">
        <span>{t("lookCols")}</span>
        {seg("cols", ["auto", "1", "2"], (c) => (c === "auto" ? t("auto") : c))}
      </div>
      {hasPics && (
        <div className="lookRow">
          <span>{t("lookPics")}</span>
          <div className="seg">
            {[true, false].map((on) => (
              <button
                key={String(on)}
                className={pics === on ? "active" : ""}
                aria-pressed={pics === on}
                onClick={() => {
                  localStorage.setItem("readerPics", on ? "on" : "off");
                  setPics(on);
                }}
              >
                {t(on ? "on" : "off")}
              </button>
            ))}
          </div>
        </div>
      )}
      <button className="lookReset" onClick={() => setLook(freshLook())}>
        {t("lookReset")}
      </button>
    </div>
  );

  return (
    <div
      className={`reader ${theme} ${bars ? "" : "bare"}`}
      style={{ "--m": look.margin, "--v": look.vmargin, "--ui": UI }}
    >
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
          <span>{book.chapters[ch]?.title || book.author}</span>
        </div>
        <button
          className={`flat aa ${lookOpen ? "active" : ""}`}
          title={t("look")}
          aria-expanded={lookOpen}
          onClick={() => {
            setToc(false);
            setLookOpen(!lookOpen);
            if (TOUCH) setBars(false);
          }}
        >
          Aa
        </button>
        <button
          className={`flat ${toc ? "active" : ""}`}
          title={t("contents")}
          onClick={() => {
            setLookOpen(false);
            setToc(!toc);
          }}
        >
          ☰
        </button>
      </header>

      <div className="readerBody">
        <button
          className="flip left"
          title={t("prevPage")}
          disabled={ch === 0 && page === 0}
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
            if (toc) return;
            const moved = e.changedTouches[0].clientX - touch.current;
            if (Math.abs(moved) > 40) return go(page + (moved < 0 ? 1 : -1));
            grab();
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
            data-align={look.align}
            data-hyph={look.hyphens}
            data-indent={look.indent}
            data-cols={look.cols}
            style={{
              "--fs": `${look.size}px`,
              "--lh": look.leading,
              "--ff": FAMILIES[look.family] ?? FAMILIES.serif,
            }}
            dangerouslySetInnerHTML={{ __html: html }}
          />
        </div>
        <button
          className="flip right"
          title={t("nextPage")}
          disabled={ch === last && page >= count - 1}
          onClick={() => go(page + 1)}
        >
          ›
        </button>

        {toc && (
          <div className="readerTocShade" onClick={() => setToc(false)} />
        )}
        {lookOpen && (
          <div
            className="readerTocShade clear"
            onClick={() => setLookOpen(false)}
          />
        )}
        {lookOpen && panel}
        {toc && (
          <nav className="readerToc" aria-label={t("contents")}>
            <header>
              <b>{t("contents")}</b>
              <button
                className="tocClose"
                title={t("close")}
                aria-label={t("close")}
                onClick={() => setToc(false)}
              >
                ×
              </button>
            </header>
            <ol>
              {book.chapters.map((c, i) => (
                <li key={i}>
                  <button
                    className={i === ch ? "active" : ""}
                    onClick={() => {
                      jump(i, { inner: 0 });
                      setToc(false);
                      if (TOUCH) setBars(false);
                    }}
                  >
                    <span>{c.title || t("part", i + 1)}</span>
                    <small>{startOf(i) + 1}</small>
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
                  { text: sel.text, position: String(frac) },
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

        {back !== null && back.at !== here && (
          <button
            className="readerBack"
            onClick={() => {
              jump(back.ch, { inner: back.inner });
              setBack(null);
            }}
          >
            {t("backToPage", back.at + 1)}
          </button>
        )}
      </div>

      <footer className="readerFoot">
        <input
          type="range"
          min={0}
          max={Math.max(0, total - 1)}
          value={here}
          aria-label={t("position")}
          onChange={(e) => {
            const n = Number(e.target.value);
            if (n >= startOf(ch) && n < startOf(ch) + count) return go(n - startOf(ch));
            const [i, within] = locate(book.chapters, n / total);
            jump(i, { inner: within });
          }}
        />
        <span>
          {offline && <i className="netDot" title={t("offlineMode")} />}
          {offline && `${t("offlineShort")} · `}
          {here + 1} / {total} · {percent}%
        </span>
      </footer>
    </div>
  );
};
