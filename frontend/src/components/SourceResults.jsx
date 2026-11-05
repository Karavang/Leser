import axios from "axios";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { t } from "../i18n";
import { hoursMinutes } from "../audio";

// Второй по приоритету результат: своя библиотека выше, найденное снаружи —
// под ней и только пока в поиске что-то введено.
export const SourceResults = ({ found, onImported, audio = false, books = [] }) => {
  const [busy, setBusy] = useState(null);
  const [error, setError] = useState(null);
  const navigate = useNavigate();
  const token = JSON.parse(localStorage.getItem("token"));

  const keyOf = (book) => (audio ? book.id : book.downloadUrl);
  const have = new Set(books.map((b) => b.sourceUrl).filter(Boolean));

  const add = async (book) => {
    setError(null);
    setBusy(keyOf(book));
    const auth = { headers: { Authorization: `Bearer ${token}` } };
    try {
      if (audio) {
        const { data } = await axios.post("/api/importAudio", { id: book.id }, auth);
        onImported();
        navigate(`/listen/${data.location}`);
        return;
      }
      await axios.post("/api/import", { url: book.downloadUrl, pageUrl: book.pageUrl }, auth);
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
          const pending = busy === key;
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
              </div>
              <button
                disabled={pending || here}
                onClick={() => add(book)}
              >
                {here ? t("inLibrary") : pending ? t("importing") : audio ? t("listenLinked") : t("importBook")}
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
};
