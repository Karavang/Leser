import axios from "axios";
import { useEffect, useRef, useState } from "react";
import { t } from "../i18n";
import { AUDIO_TYPES, audioGroups, groupName, hoursMinutes, isAudio } from "../audio";

// Тот же список, что BOOK_EXTS в api/src/books.rs — здесь он только чтобы
// в диалоге выбора файла не показывались форматы, которые всё равно отлетят.
// Синонимы (htm, markdown, azw) бэк сводит к основному имени сам, но в
// диалоге они нужны: иначе файл просто не выбрать. rda — формат данных R,
// читалки под него нет, бэк превращает его в fb2 при загрузке.
const BOOK_TYPES = [
  "epub",
  "fb2",
  "mobi",
  "azw3",
  "azw",
  "pdf",
  "txt",
  "md",
  "markdown",
  "html",
  "htm",
  "rda",
];
const ACCEPT = [...BOOK_TYPES, ...AUDIO_TYPES].map((t) => `.${t}`).join(",");
/// В подписи только основные имена: `htm` рядом с `html` читателю ничего
/// не говорит, а строка из-за них переносится на три.
const SHOWN = ["epub", "fb2", "mobi", "azw3", "pdf", "txt", "md", "html"];
const MAX_MB = 90;
const MAX_AUDIO_GB = 4;
const extOf = (file) => file.name.split(".").pop().toLowerCase();

const duration = (file) =>
  new Promise((resolve) => {
    const probe = document.createElement("audio");
    const url = URL.createObjectURL(file);
    let timer;
    const done = (seconds) => {
      clearTimeout(timer);
      probe.removeAttribute("src");
      URL.revokeObjectURL(url);
      resolve(Number.isFinite(seconds) && seconds > 0 ? seconds : 0);
    };
    timer = setTimeout(() => done(0), 20000);
    probe.preload = "metadata";
    probe.onloadedmetadata = () => done(probe.duration);
    probe.onerror = () => done(0);
    probe.src = url;
  });

const again = async (send, tries = 3) => {
  for (let n = 1; ; n++) {
    try {
      return await send();
    } catch (error) {
      if (error.response || n >= tries) throw error;
      await new Promise((resolve) => setTimeout(resolve, 2000 * n));
    }
  }
};

/// Что показать про файл в списке. Ошибку присылает бэк — она конкретнее
/// любой нашей: битый файл, DRM, не тот формат внутри.
const STATE = {
  wait: () => t("queued"),
  // На 100% файл только доехал: дальше сервер разбирает книгу, а у скана
  // на сотню мегабайт это минуты. Проценты, застывшие на сотне, читаются
  // как «всё повисло».
  send: (item) => (item.percent >= 100 ? t("parsing") : `${item.percent}%`),
  done: () => t("uploaded"),
  same: () => t("alreadyHere"),
  removed: () => t("removed"),
  skipped: () => t("skipped"),
  fail: (item) => item.message,
};

export const ModalAdd = ({ setIsAddModal, onAdded }) => {
  const user = JSON.parse(localStorage.getItem("user"));
  const dialog = useRef(null);
  const nextId = useRef(0);
  const [items, setItems] = useState([]);
  const [over, setOver] = useState(false);

  // <dialog> сам даёт затемнение, Escape и удержание фокуса внутри —
  // всё это раньше было руками и наполовину.
  useEffect(() => dialog.current?.showModal(), []);

  // Файл, брошенный мимо зоны, браузер по умолчанию открывает вместо
  // страницы — то есть уводит с сайта вместе с несохранённым.
  useEffect(() => {
    const stop = (e) => e.preventDefault();
    window.addEventListener("dragover", stop);
    window.addEventListener("drop", stop);
    return () => {
      window.removeEventListener("dragover", stop);
      window.removeEventListener("drop", stop);
    };
  }, []);

  const close = () => setIsAddModal(false);

  const patch = (id, fields) =>
    setItems((prev) => prev.map((i) => (i.id === id ? { ...i, ...fields } : i)));

  const auth = { headers: { Authorization: `Bearer ${user.token}` } };

  const failed = (item, error) => {
    const dup = error.response?.status === 409 && error.response.data;
    if (dup) {
      patch(item.id, { state: dup.duplicate, book: dup.book });
      if (dup.duplicate === "same") onAdded?.();
      return;
    }
    const said = error.response?.data?.message;
    const code = error.response?.status;
    patch(item.id, {
      state: "fail",
      tracks: null,
      message: said ?? (code ? `${t("uploadFailed")} (${code})` : t("uploadFailed")),
    });
  };

  const sendAudio = async (item, force) => {
    if (item.files.some((f) => f.size > MAX_AUDIO_GB * 1024 ** 3)) {
      return patch(item.id, { state: "fail", message: t("tooBigGb", MAX_AUDIO_GB) });
    }
    patch(item.id, { state: "send", percent: 0, force, editError: null });
    const slow = { ...auth, patient: true };
    try {
      let tracks = item.tracks;
      if (!tracks) {
        const total = item.files.reduce((n, f) => n + f.size, 0) || 1;
        let sent = 0;
        tracks = [];
        for (const file of item.files) {
          const seconds = await duration(file);
          const { data } = await axios.post("/api/audio/tracks", { name: file.name, size: file.size, seconds }, slow);
          for (let start = 0; start < file.size; start += data.chunk) {
            const part = file.slice(start, start + data.chunk);
            await again(() =>
              axios.post(`/api/audio/tracks/${data.id}?start=${start}`, part, {
                ...slow,
                headers: { ...auth.headers, "Content-Type": "application/octet-stream" },
                onUploadProgress: (e) =>
                  patch(item.id, { percent: Math.min(99, Math.round(((sent + e.loaded) / total) * 100)) }),
              }),
            );
            sent += part.size;
          }
          tracks.push(data.id);
        }
        patch(item.id, { tracks, percent: 100 });
      }
      const { data } = await axios.post(
        `/api/audio${force ? "?force=true" : ""}`,
        { tracks, title: item.name },
        slow,
      );
      patch(item.id, { state: "check", book: data.book });
      onAdded?.();
    } catch (error) {
      failed(item, error);
    }
  };

  const send = async (item, force = false) => {
    if (item.audio) return sendAudio(item, force);
    const ext = extOf(item.file);
    if (!BOOK_TYPES.includes(ext)) {
      return patch(item.id, { state: "fail", message: t("badFormat") });
    }
    if (item.file.size > MAX_MB * 1024 * 1024) {
      return patch(item.id, { state: "fail", message: t("tooBig", MAX_MB) });
    }

    patch(item.id, { state: "send", percent: 0, force, editError: null });
    const form = new FormData();
    form.append("file", item.file);
    try {
      const { data } = await axios.post(`/api/putNewOne${force ? "?force=true" : ""}`, form, {
        ...auth,
        onUploadProgress: (e) =>
          e.total &&
          patch(item.id, { percent: Math.round((e.loaded / e.total) * 100) }),
      });
      patch(item.id, { state: "check", book: data.book });
      // книга уже в базе — список обновляем сразу, а не к перезагрузке
      onAdded?.();
    } catch (error) {
      failed(item, error);
    }
  };

  const edit = async (item, form) => {
    patch(item.id, { busy: true, editError: null });
    try {
      const { data } = await axios.post(
        `/api/editBook/${item.book.id}`,
        {
          title: form.get("title"),
          author: form.get("author"),
          series: form.get("series"),
        },
        auth,
      );
      patch(item.id, { state: "done", book: data, busy: false });
      onAdded?.();
    } catch (error) {
      patch(item.id, {
        busy: false,
        editError: error.response?.data?.message ?? t("saveFailed"),
      });
    }
  };

  const remove = async (item) => {
    patch(item.id, { busy: true, editError: null });
    try {
      await axios.delete(`/api/deleteOne/${item.book.id}`, auth);
      patch(item.id, { state: "removed", busy: false });
      onAdded?.();
    } catch (error) {
      patch(item.id, {
        busy: false,
        editError: error.response?.data?.message ?? t("deleteFailed"),
      });
    }
  };

  /// Книги уходят по одной, а не разом: десять файлов сразу — это десять
  /// разборов epub на сервере в один момент.
  const add = (files) => {
    const all = [...files];
    const sounds = all.filter((f) => AUDIO_TYPES.includes(extOf(f)));
    const fresh = [
      ...all
        .filter((f) => !sounds.includes(f))
        .map((file) => ({ id: nextId.current++, file, state: "wait", percent: 0 })),
      ...audioGroups(sounds).map((group) => ({
        id: nextId.current++,
        audio: true,
        files: group,
        name: groupName(group),
        file: { name: groupName(group) },
        state: "wait",
        percent: 0,
      })),
    ];
    setItems((prev) => [...prev, ...fresh]);
    fresh.reduce((queue, item) => queue.then(() => send(item)), Promise.resolve());
  };

  return (
    <dialog
      ref={dialog}
      className="modalAdd"
      onClose={close}
      // клик мимо карточки попадает в сам dialog — это и есть фон
      onClick={(e) => e.target === dialog.current && dialog.current.close()}
    >
      <div className="modalCard">
        <header>
          <h2>{t("addBooks")}</h2>
          <button
            type="button"
            className="modalClose"
            aria-label={t("close")}
            onClick={() => dialog.current.close()}
          >
            ×
          </button>
        </header>

        <label
          className={over ? "dropZone over" : "dropZone"}
          onDragOver={(e) => {
            e.preventDefault();
            setOver(true);
          }}
          onDragLeave={() => setOver(false)}
          onDrop={(e) => {
            e.preventDefault();
            setOver(false);
            add(e.dataTransfer.files);
          }}
        >
          <input
            type="file"
            multiple
            accept={ACCEPT}
            onChange={(e) => {
              add(e.target.files);
              // иначе тот же файл вторым разом не выбрать: значение не сменилось
              e.target.value = "";
            }}
          />
          <svg
            viewBox="0 0 24 24"
            width="40"
            height="40"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M12 16V4m0 0L7.5 8.5M12 4l4.5 4.5" />
            <path d="M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />
          </svg>
          <b>{t("dropHere")}</b>
          <span>{t("orPick")}</span>
          <small>
            {SHOWN.join(" · ")}
            <br />
            {t("eachUpTo", MAX_MB)}
            <br />
            {t("audioHint", MAX_AUDIO_GB)}
          </small>
        </label>

        {items.length > 0 && (
          <ul className="uploads">
            {items.map((item) => (
              <li key={item.id} className={item.state}>
                <span className="uploadName">
                  {item.book && item.state !== "similar" ? item.book.title : item.file.name}
                  {item.audio && item.files.length > 1 && <small> · {t("filesCount", item.files.length)}</small>}
                </span>
                {STATE[item.state] && (
                  <span className="uploadState">{STATE[item.state](item)}</span>
                )}
                {item.state === "fail" && (
                  <button type="button" className="uploadAct" onClick={() => send(item, item.force)}>
                    {t("retry")}
                  </button>
                )}
                {item.state === "send" && (
                  <div className="uploadBar">
                    <div style={{ width: `${item.percent}%` }} />
                  </div>
                )}
                {item.state === "check" && (
                  <div className="uploadCheck">
                    <BookLine book={item.book} />
                    <p>{t("isRight")}</p>
                    <fieldset className="uploadActs" disabled={item.busy}>
                      <button type="button" onClick={() => patch(item.id, { state: "done" })}>
                        {t("looksRight")}
                      </button>
                      <button type="button" onClick={() => patch(item.id, { state: "edit", editError: null })}>
                        {t("fixMeta")}
                      </button>
                      <button type="button" className="danger" onClick={() => remove(item)}>
                        {t("removeUpload")}
                      </button>
                    </fieldset>
                    {item.editError && <p className="uploadError">{item.editError}</p>}
                  </div>
                )}
                {item.state === "edit" && (
                  <form
                    className="uploadCheck"
                    onSubmit={(e) => {
                      e.preventDefault();
                      edit(item, new FormData(e.currentTarget));
                    }}
                  >
                    <label>
                      {t("bookTitle")}
                      <input name="title" required defaultValue={item.book.title} />
                    </label>
                    <label>
                      {t("bookAuthor")}
                      <input name="author" defaultValue={item.book.author} />
                    </label>
                    <label>
                      {t("series")}
                      <input name="series" defaultValue={item.book.series ?? ""} />
                    </label>
                    <fieldset className="uploadActs" disabled={item.busy}>
                      <button type="submit">{t("saveMeta")}</button>
                      <button type="button" onClick={() => patch(item.id, { state: "check", editError: null })}>
                        {t("cancel")}
                      </button>
                    </fieldset>
                    {item.editError && <p className="uploadError">{item.editError}</p>}
                  </form>
                )}
                {item.state === "similar" && (
                  <div className="uploadCheck">
                    <p>{t("similarHere")}</p>
                    <BookLine book={item.book} />
                    <fieldset className="uploadActs" disabled={item.busy}>
                      <button type="button" onClick={() => send(item, true)}>
                        {t("addAnyway")}
                      </button>
                      <button type="button" onClick={() => patch(item.id, { state: "skipped" })}>
                        {t("skip")}
                      </button>
                    </fieldset>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
    </dialog>
  );
};

const BookLine = ({ book }) => (
  <div className="uploadBook">
    <b>{book.title}</b>
    <span>{book.author || t("noAuthor")}</span>
    {book.series && <span>{book.series}</span>}
    <small>
      {[
        isAudio(book) ? t("hoursMinutes", ...hoursMinutes(book.seconds)) : book.ext,
        book.lang,
        book.date?.slice(0, 4),
      ]
        .filter(Boolean)
        .join(" · ")}
    </small>
  </div>
);
