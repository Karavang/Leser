import axios from "axios";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { LoadingBook } from "../ui/Loading.jsx";
import { t } from "../i18n";
import { isOffline, useOffline } from "../offline";
import { rememberSpot, savedSpot, sendPage, unsentPage } from "../sync";
import { theme } from "../theme";
import { clock, markAt, marks, spotOf, starts } from "../audio";

const RATES = [0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3];
const SLEEP = [0, 15, 30, 45, 60, -1];
const BACK = 15;
const AHEAD = 30;
// ponytail: every spot sent to the server counts as a page in the reading stats; one per minute of listening is about a page of narration
const SAVE_EVERY = 60000;
const ONE_WORD = /^[\p{L}\p{M}'’-]+$/u;

const savedRate = () => {
  try {
    const r = Number(localStorage.getItem("listenRate"));
    return RATES.includes(r) ? r : 1;
  } catch {
    return 1;
  }
};

const svg = (d, size = 24) => (
  <svg
    aria-hidden="true"
    width={size}
    height={size}
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
const PLAY = (
  <svg aria-hidden="true" width="34" height="34" viewBox="0 0 24 24">
    <path d="M8 5.5v13l10.5-6.5z" fill="currentColor" />
  </svg>
);
const PAUSE = (
  <svg aria-hidden="true" width="34" height="34" viewBox="0 0 24 24">
    <path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" fill="currentColor" />
  </svg>
);
const PREV = svg("M6 5v14M19 5l-9 7 9 7z");
const NEXT = svg("M18 5v14M5 5l9 7-9 7z");
const BACKWARD = svg("M3 12a9 9 0 1 0 3-6.7M3 4v5h5");
const FORWARD = svg("M21 12a9 9 0 1 1-3-6.7M21 4v5h-5");
const MOON = svg("M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z", 18);
const MARK = svg("M6 3h12v18l-6-4-6 4z", 18);

export const Listener = () => {
  const { id } = useParams();
  const navigate = useNavigate();
  const location = useLocation();
  const token = JSON.parse(localStorage.getItem("token"));
  const auth = { headers: { Authorization: `Bearer ${token}` } };
  const openAt = Number.parseFloat(new URLSearchParams(location.search).get("at"));
  const fromQuote = Number.isFinite(openAt) && openAt >= 0 && openAt < 1;

  const audio = useRef(null);
  const seekTo = useRef(null);
  const autoplay = useRef(false);
  const started = useRef(false);
  const sentAt = useRef(0);
  const retried = useRef(false);
  const sleepMark = useRef(null);
  const latest = useRef(null);

  const [book, setBook] = useState(null);
  const [error, setError] = useState(null);
  const [media, setMedia] = useState(null);
  const [track, setTrack] = useState(0);
  const [time, setTime] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [rate, setRate] = useState(savedRate);
  const [sleep, setSleep] = useState(0);
  const [sleepAt, setSleepAt] = useState(null);
  const [toc, setToc] = useState(false);
  const [note, setNote] = useState(null);
  const [said, setSaid] = useState(null);
  const offline = useOffline() === "offline";

  useEffect(() => {
    let alive = true;
    const where =
      fromQuote || isOffline()
        ? Promise.resolve(null)
        : axios.get(`/api/progress/${id}`, auth).catch(() => null);
    Promise.all([
      axios.get(`/api/listen/${id}`, auth),
      where,
      axios.get(`/api/imgToken/${id}`, auth),
    ])
      .then(([listen, progress, signed]) => {
        if (!alive) return;
        const tracks = listen.data.tracks || [];
        if (!tracks.length) return setError(t("openFailed"));
        let frac = fromQuote ? openAt : savedSpot(id);
        const server = Number.parseFloat(unsentPage(id) ?? progress?.data?.page);
        if (!fromQuote && Number.isFinite(server) && server >= 0 && server < 1) frac = server;
        const [i, within] = spotOf(tracks, frac);
        seekTo.current = within;
        setTrack(i);
        setTime(within);
        setMedia(signed.data.token);
        setBook(listen.data);
      })
      .catch((e) => alive && setError(e.response?.data?.message || t("openFailed")));
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const tracks = useMemo(() => book?.tracks || [], [book]);
  const cum = useMemo(() => starts(tracks), [tracks]);
  const list = useMemo(() => marks(tracks), [tracks]);
  const total = cum[cum.length - 1] || 1;
  const last = tracks.length - 1;
  const here = (cum[track] || 0) + time;
  const frac = Math.min(1, here / total);
  const cur = markAt(list, track, time);
  const mark = list[cur];
  const markEnd =
    list[cur + 1]?.track === track ? list[cur + 1].start : tracks[track]?.seconds || 0;
  latest.current = frac;

  const save = useCallback(
    (now) => {
      if (!book || (fromQuote && !started.current)) return;
      rememberSpot(id, latest.current);
      if (now || Date.now() - sentAt.current >= SAVE_EVERY) {
        sentAt.current = Date.now();
        sendPage(id, String(latest.current));
      }
    },
    [book, fromQuote, id],
  );

  const bucket = Math.floor(here / 5);
  useEffect(() => {
    if (playing) save(false);
  }, [bucket, playing, save]);

  useEffect(() => {
    const away = () => document.visibilityState === "hidden" && save(true);
    document.addEventListener("visibilitychange", away);
    return () => {
      document.removeEventListener("visibilitychange", away);
      save(true);
    };
  }, [save]);

  useEffect(() => {
    if (audio.current) {
      audio.current.playbackRate = rate;
      audio.current.defaultPlaybackRate = rate;
    }
    try {
      localStorage.setItem("listenRate", String(rate));
    } catch {
      return;
    }
  }, [rate, media, track]);

  const seekBook = useCallback(
    (seconds) => {
      const a = audio.current;
      if (!a || !tracks.length) return;
      const [i, within] = spotOf(tracks, seconds / total);
      started.current = true;
      if (i === track) {
        a.currentTime = within;
        setTime(within);
      } else {
        seekTo.current = within;
        autoplay.current = !a.paused;
        setTrack(i);
        setTime(within);
      }
    },
    [tracks, total, track],
  );

  const toggle = useCallback(() => {
    const a = audio.current;
    if (!a) return;
    if (a.paused) a.play().catch(() => setError(t("playFailed")));
    else a.pause();
  }, []);

  const skip = useCallback(
    (delta) => seekBook(Math.min(total - 0.5, Math.max(0, here + delta))),
    [seekBook, here, total],
  );

  const toMark = useCallback(
    (n) => {
      const m = list[Math.max(0, Math.min(list.length - 1, n))];
      if (m) seekBook(cum[m.track] + m.start);
    },
    [list, cum, seekBook],
  );
  const prevMark = useCallback(
    () => toMark(time - (mark?.start ?? 0) > 3 ? cur : cur - 1),
    [toMark, time, mark, cur],
  );
  const nextMark = useCallback(() => cur < list.length - 1 && toMark(cur + 1), [toMark, cur, list]);

  const close = () =>
    location.key === "default" ? navigate("/", { replace: true }) : navigate(-1);

  useEffect(() => {
    const onKey = (e) => {
      if (e.target.closest?.("input")) return;
      if (e.key === "Escape") {
        if (toc) return setToc(false);
        if (note !== null) return setNote(null);
        return close();
      }
      if (e.key === " ") {
        e.preventDefault();
        toggle();
      } else if (e.key === "ArrowLeft") skip(-BACK);
      else if (e.key === "ArrowRight") skip(AHEAD);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [toggle, skip, toc, note]);

  useEffect(() => {
    const session = navigator.mediaSession;
    if (!session || !book) return;
    if (window.MediaMetadata) {
      session.metadata = new MediaMetadata({
        title: mark?.title || book.title,
        artist: book.author,
        album: book.title,
      });
    }
    const handlers = {
      play: toggle,
      pause: toggle,
      seekbackward: () => skip(-BACK),
      seekforward: () => skip(AHEAD),
      previoustrack: prevMark,
      nexttrack: nextMark,
      seekto: (d) => audio.current && (audio.current.currentTime = d.seekTime),
    };
    for (const [action, handler] of Object.entries(handlers)) {
      try {
        session.setActionHandler(action, handler);
      } catch {
        continue;
      }
    }
    return () => {
      for (const action of Object.keys(handlers)) {
        try {
          session.setActionHandler(action, null);
        } catch {
          continue;
        }
      }
    };
  }, [book, mark, toggle, skip, prevMark, nextMark]);

  const onLoaded = () => {
    const a = audio.current;
    a.playbackRate = rate;
    if (seekTo.current != null) {
      a.currentTime = seekTo.current;
      seekTo.current = null;
    }
    if (autoplay.current) {
      autoplay.current = false;
      a.play().catch(() => {});
    }
  };

  const onTime = () => {
    const a = audio.current;
    setTime(a.currentTime);
    if (navigator.mediaSession?.setPositionState && Number.isFinite(a.duration)) {
      try {
        navigator.mediaSession.setPositionState({
          duration: a.duration,
          playbackRate: a.playbackRate,
          position: Math.min(a.currentTime, a.duration),
        });
      } catch {
        return;
      }
    }
    const chapterOver = sleep === -1 && markAt(list, track, a.currentTime) !== sleepMark.current;
    if ((sleepAt && Date.now() >= sleepAt) || chapterOver) {
      a.pause();
      setSleep(0);
      setSleepAt(null);
    }
  };

  const onEnded = () => {
    if (track < last) {
      autoplay.current = sleep !== -1;
      if (sleep === -1) setSleep(0);
      seekTo.current = 0;
      setTrack(track + 1);
      setTime(0);
      return;
    }
    setPlaying(false);
    rememberSpot(id, 1);
    sendPage(id, "1");
  };

  const onError = () => {
    if (!audio.current?.error || !media) return;
    if (tracks[track]?.url) return setError(t("sourceDown"));
    if (retried.current || isOffline()) return setError(t("playFailed"));
    retried.current = true;
    seekTo.current = time;
    autoplay.current = playing;
    axios
      .get(`/api/imgToken/${id}`, auth)
      .then((r) => setMedia(r.data.token))
      .catch(() => setError(t("playFailed")));
  };

  const nextSleep = () => {
    const next = SLEEP[(SLEEP.indexOf(sleep) + 1) % SLEEP.length];
    setSleep(next);
    setSleepAt(next > 0 ? Date.now() + next * 60000 : null);
    sleepMark.current = cur;
  };

  const keep = (url, body, ok) => {
    axios
      .post(url, { book: id, ...body }, auth)
      .then(() => {
        setSaid(ok);
        setNote(null);
      })
      .catch((e) => setSaid(e.response?.data?.message || t("saveFailed")))
      .finally(() => setTimeout(() => setSaid(null), 2500));
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
  if (!book || !media) return <LoadingBook theme={theme} />;

  const left = Math.max(0, total - here);
  const where = `${mark?.title || book.title} · ${clock(time)}`;
  const typed = (note || "").trim();
  const sleepLabel =
    sleep === -1
      ? t("sleepChapter")
      : sleepAt
        ? t("minutes", Math.max(1, Math.ceil((sleepAt - Date.now()) / 60000)))
        : t("sleep");

  return (
    <div className={`reader listener ${theme}`}>
      <header className="readerBar">
        <button className="flat" onClick={close} title={t("toLibrary")} aria-label={t("toLibrary")}>
          ←
        </button>
        <div className="readerWhat">
          <b>{book.title}</b>
          <span>{book.author}</span>
        </div>
        <button
          className={`flat ${toc ? "active" : ""}`}
          title={t("contents")}
          aria-label={t("contents")}
          aria-expanded={toc}
          onClick={() => setToc(!toc)}
        >
          ☰
        </button>
      </header>

      <div className="readerBody">
        <main className="listenBody" lang={book.lang || undefined}>
          <div className="listenCover" aria-hidden="true">
            <b>{book.title}</b>
            {book.author && <span>{book.author}</span>}
          </div>
          <p className="listenPart">
            {list.length > 1 && `${cur + 1} / ${list.length}`}
          </p>
          <h1>{mark?.title || book.title}</h1>
          <input
            type="range"
            className="listenScrub"
            min={0}
            max={Math.max(1, Math.round(markEnd - (mark?.start ?? 0)))}
            step={1}
            value={Math.round(time - (mark?.start ?? 0))}
            aria-label={t("position")}
            onChange={(e) => seekBook(cum[track] + (mark?.start ?? 0) + Number(e.target.value))}
          />
          <div className="listenTimes">
            <span>{clock(time - (mark?.start ?? 0))}</span>
            <span>−{clock(markEnd - time)}</span>
          </div>
          <div className="listenControls">
            <button title={t("prevChapter")} aria-label={t("prevChapter")} onClick={prevMark}>
              {PREV}
            </button>
            <button
              className="skip"
              title={t("skipBack", BACK)}
              aria-label={t("skipBack", BACK)}
              onClick={() => skip(-BACK)}
            >
              {BACKWARD}
              <small>{BACK}</small>
            </button>
            <button
              className="play"
              title={playing ? t("pause") : t("play")}
              aria-label={playing ? t("pause") : t("play")}
              onClick={toggle}
            >
              {playing ? PAUSE : PLAY}
            </button>
            <button
              className="skip"
              title={t("skipAhead", AHEAD)}
              aria-label={t("skipAhead", AHEAD)}
              onClick={() => skip(AHEAD)}
            >
              {FORWARD}
              <small>{AHEAD}</small>
            </button>
            <button
              title={t("nextChapter")}
              aria-label={t("nextChapter")}
              disabled={cur >= list.length - 1}
              onClick={nextMark}
            >
              {NEXT}
            </button>
          </div>
          <div className="listenExtras">
            <button
              title={t("speed")}
              aria-label={`${t("speed")} ${rate}×`}
              onClick={() => setRate(RATES[(RATES.indexOf(rate) + 1) % RATES.length])}
            >
              {rate}×
            </button>
            <button
              className={sleep ? "on" : ""}
              title={t("sleep")}
              aria-pressed={Boolean(sleep)}
              onClick={nextSleep}
            >
              {MOON}
              {sleepLabel}
            </button>
            <button title={t("markPlace")} onClick={() => setNote(note === null ? "" : null)}>
              {MARK}
              {t("markPlace")}
            </button>
          </div>
        </main>

        {toc && <div className="readerTocShade" onClick={() => setToc(false)} />}
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
              {list.map((m, n) => (
                <li key={n}>
                  <button
                    className={n === cur ? "active" : ""}
                    onClick={() => {
                      toMark(n);
                      setToc(false);
                    }}
                  >
                    <span>{m.title}</span>
                    <small>{clock(cum[m.track] + m.start)}</small>
                  </button>
                </li>
              ))}
            </ol>
          </nav>
        )}

        {note !== null && (
          <form
            className="readerSel listenNote"
            onSubmit={(e) => {
              e.preventDefault();
              keep("/api/quotes", { text: typed || where, position: String(frac) }, t("quoteSaved"));
            }}
          >
            <input
              autoFocus
              value={note}
              placeholder={where}
              aria-label={t("markPlace")}
              onChange={(e) => setNote(e.target.value)}
            />
            <button type="submit">{t("saveQuote")}</button>
            {ONE_WORD.test(typed) && (
              <button
                type="button"
                onClick={() =>
                  keep("/api/words", { word: typed, context: `${book.title} · ${where}` }, t("wordSaved"))
                }
              >
                {t("saveWord")}
              </button>
            )}
            <button type="button" className="flat" title={t("close")} onClick={() => setNote(null)}>
              ×
            </button>
          </form>
        )}
        {said && <div className="readerSaid">{said}</div>}
      </div>

      <footer className="readerFoot">
        <input
          type="range"
          min={0}
          max={Math.round(total)}
          step={1}
          value={Math.round(here)}
          aria-label={t("position")}
          onChange={(e) => seekBook(Number(e.target.value))}
        />
        <span>
          {offline && <i className="netDot" title={t("offlineMode")} />}
          {offline && `${t("offlineShort")} · `}
          {t("timeLeft", clock(left / rate))} · {Math.round(frac * 100)}%
        </span>
      </footer>

      <audio
        ref={audio}
        src={tracks[track]?.url ?? `/api/audio/${id}/${track}?t=${media}`}
        referrerPolicy="no-referrer"
        preload="auto"
        onLoadedMetadata={onLoaded}
        onTimeUpdate={onTime}
        onPlay={() => {
          started.current = true;
          retried.current = false;
          setPlaying(true);
        }}
        onPause={() => {
          setPlaying(false);
          save(true);
        }}
        onEnded={onEnded}
        onError={onError}
      />
    </div>
  );
};
