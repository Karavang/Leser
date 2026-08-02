import axios from "axios";
import { useState } from "react";

// Второй по приоритету результат: своя библиотека выше, найденное снаружи —
// под ней и только пока в поиске что-то введено.
export const SourceResults = ({ found, onImported }) => {
  const [busy, setBusy] = useState(null);
  const [error, setError] = useState(null);
  const token = JSON.parse(localStorage.getItem("token"));

  // Скачивает сервер, а не браузер: адрес проверяется по списку источников,
  // метаданные берутся из самого файла, а не из того, что показал источник.
  const add = async (book) => {
    setBusy(book.downloadUrl);
    setError(null);
    try {
      await axios.post(
        "/api/import",
        { url: book.downloadUrl, pageUrl: book.pageUrl },
        { headers: { Authorization: `Bearer ${token}` } },
      );
      onImported();
    } catch (e) {
      setError(e.response?.data?.message ?? "Не удалось добавить книгу");
    } finally {
      setBusy(null);
    }
  };

  if (!found.length) return null;

  return (
    <div className="sourceResults">
      <h3>Другие источники</h3>
      {error && <p className="error">{error}</p>}
      <ul>
        {found.map((book) => (
          <li key={book.downloadUrl}>
            <div>
              <h4>{book.title}</h4>
              {book.author && <p>{book.author}</p>}
              <a
                href={book.pageUrl}
                target="_blank"
                rel="noreferrer noopener"
              >
                {book.source}
              </a>
            </div>
            <button
              disabled={busy === book.downloadUrl}
              onClick={() => add(book)}
            >
              {busy === book.downloadUrl ? "Забираем…" : "В библиотеку"}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
};
