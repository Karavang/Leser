import axios from "axios";
import { useEffect, useRef, useState } from "react";
import { t } from "../i18n";

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
const ACCEPT = BOOK_TYPES.map((t) => `.${t}`).join(",");
/// В подписи только основные имена: `htm` рядом с `html` читателю ничего
/// не говорит, а строка из-за них переносится на три.
const SHOWN = ["epub", "fb2", "mobi", "azw3", "pdf", "txt", "md", "html"];
const MAX_MB = 64;

/// Что показать про файл в списке. Ошибку присылает бэк — она конкретнее
/// любой нашей: битый файл, DRM, не тот формат внутри.
const STATE = {
  wait: () => t("queued"),
  send: (item) => `${item.percent}%`,
  done: () => t("uploaded"),
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

  const close = () => {
    setIsAddModal(false);
    document.body.style.overflowY = "scroll";
  };

  const patch = (id, fields) =>
    setItems((prev) => prev.map((i) => (i.id === id ? { ...i, ...fields } : i)));

  const send = async (item) => {
    const ext = item.file.name.split(".").pop().toLowerCase();
    if (!BOOK_TYPES.includes(ext)) {
      return patch(item.id, { state: "fail", message: t("badFormat") });
    }
    if (item.file.size > MAX_MB * 1024 * 1024) {
      return patch(item.id, { state: "fail", message: t("tooBig", MAX_MB) });
    }

    patch(item.id, { state: "send" });
    const form = new FormData();
    form.append("file", item.file);
    try {
      await axios.post("/api/putNewOne", form, {
        // Content-Type ставит браузер: только он знает границу частей.
        headers: { Authorization: `Bearer ${user.token}` },
        onUploadProgress: (e) =>
          e.total &&
          patch(item.id, { percent: Math.round((e.loaded / e.total) * 100) }),
      });
      patch(item.id, { state: "done" });
      // книга уже в базе — список обновляем сразу, а не к перезагрузке
      onAdded?.();
    } catch (error) {
      patch(item.id, {
        state: "fail",
        // при сетевой ошибке response нет вовсе — без ?. тут падало
        message: error.response?.data?.message ?? t("uploadFailed"),
      });
    }
  };

  /// Книги уходят по одной, а не разом: десять файлов сразу — это десять
  /// разборов epub на сервере в один момент.
  const add = (files) => {
    const fresh = [...files].map((file) => ({
      id: nextId.current++,
      file,
      state: "wait",
      percent: 0,
    }));
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
          </small>
        </label>

        {items.length > 0 && (
          <ul className="uploads">
            {items.map((item) => (
              <li key={item.id} className={item.state}>
                <span className="uploadName">{item.file.name}</span>
                <span className="uploadState">{STATE[item.state](item)}</span>
                {item.state === "send" && (
                  <div className="uploadBar">
                    <div style={{ width: `${item.percent}%` }} />
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
