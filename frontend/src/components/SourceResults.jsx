import axios from "axios";
import { useEffect, useRef, useState } from "react";
import { t } from "../i18n";
import { hoursMinutes } from "../audio";

const POLL = 4000;

// Второй по приоритету результат: своя библиотека выше, найденное снаружи —
// под ней и только пока в поиске что-то введено.
export const SourceResults = ({ found, onImported, audio = false, books = [] }) => {
  const [busy, setBusy] = useState(null);
  const [waiting, setWaiting] = useState({});
  const [error, setError] = useState(null);
  const token = JSON.parse(localStorage.getItem("token"));
  const alive = useRef(true);

  useEffect(
    () => () => {
      alive.current = false;
    },
    [],
  );

  const keyOf = (book) => (audio ? book.id : book.downloadUrl);
  const have = new Set(books.map((b) => b.sourceUrl).filter(Boolean));

  const fetchAudio = async (book) => {
    const auth = { headers: { Authorization: `Bearer ${token}` } };
    setWaiting((w) => ({ ...w, [book.id]: true }));
    try {
      while (alive.current) {
        const { status } = await axios.post("/api/importAudio", { id: book.id }, auth);
        if (status !== 202) break;
        await new Promise((resolve) => setTimeout(resolve, POLL));
      }
      onImported();
    } catch (e) {
      setError(e.response?.data?.message ?? t("importFailed"));
    } finally {
      setWaiting((w) => ({ ...w, [book.id]: false }));
    }
  };

  // Скачивает сервер, а не браузер: адрес проверяется по списку источников,
  // метаданные берутся из самого файла, а не из того, что показал источник.
  const add = async (book) => {
    setError(null);
    if (audio) return fetchAudio(book);
    setBusy(book.downloadUrl);
    try {
      await axios.post(
        "/api/import",
        { url: book.downloadUrl, pageUrl: book.pageUrl },
        { headers: { Authorization: `Bearer ${token}` } },
      );
      onImported();
    } catch (e) {
      setError(e.response?.data?.message ?? t("importFailed"));
    } finally {
      setBusy(null);
    }
  };

  if (!found.length) return null;

  return (
    <div className="sourceResults">
      <h3>{t("otherSources")}</h3>
      {error && <p className="error">{error}</p>}
      <ul>
        {found.map((book) => {
          const key = keyOf(book);
          const pending = busy === key || waiting[key];
          const here = audio && have.has(book.pageUrl);
          return (
            <li key={key}>
              <div>
                <h4>{book.title}</h4>
                {book.author && <p>{book.author}</p>}
                {audio && book.seconds > 0 && (
                  <p>{t("hoursMinutes", ...hoursMinutes(book.seconds))}</p>
                )}
                <a
                  href={book.pageUrl}
                  target="_blank"
                  rel="noreferrer noopener"
                >
                  {[book.source, book.lang].filter(Boolean).join(" · ")}
                </a>
                {waiting[key] && <p className="sourceWait">{t("importAudioWait")}</p>}
              </div>
              <button
                disabled={pending || here}
                onClick={() => add(book)}
              >
                {here ? t("inLibrary") : pending ? t("importing") : t("importBook")}
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
};
