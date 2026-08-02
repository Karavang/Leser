import { useEffect, useRef } from "react";

const pad = (n) => String(n).padStart(2, "0");

/// Момент времени с бэка (rfc3339) — в дд.мм.гггг в зоне читателя.
const stamp = (iso) => {
  const d = new Date(iso);
  return isNaN(d) ? null : `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`;
};

/// Год издания из метаданных книги — строка произвольной формы: бывает "1869",
/// бывает полный ISO. Полную дату переворачиваем, остальное показываем как есть.
/// Через Date такое парсить нельзя: new Date("1869") даст 01.01.1869, а в файле
/// этого дня не написано.
const published = (v) => {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(v ?? "");
  return m ? `${m[3]}.${m[2]}.${m[1]}` : v;
};

// Нативный <dialog>: Esc, подложка, фокус внутри окна и inert для остальной
// страницы — всё это браузер делает сам. Своего кода на это нет.
export const BookDetails = ({ book, onClose, onDelete }) => {
  const ref = useRef(null);

  useEffect(() => {
    ref.current.showModal();
  }, []);

  const addedAt = book.addedAt ? stamp(book.addedAt) : null;

  return (
    <dialog
      className="bookDetails"
      ref={ref}
      onClose={onClose}
      // клик по подложке — это клик по самому dialog, содержимое лежит внутри div
      onClick={(e) => e.target === ref.current && ref.current.close()}
    >
      <div className="detailsBody">
        <h2>{book.title}</h2>
        <h3>{book.author}</h3>

        <dl>
          {book.series && (
            <>
              <dt>Серия</dt>
              <dd>{book.series}</dd>
            </>
          )}
          {book.date && (
            <>
              <dt>Издана</dt>
              <dd>{published(book.date)}</dd>
            </>
          )}
          {book.lang && (
            <>
              <dt>Язык</dt>
              <dd>{book.lang}</dd>
            </>
          )}
          {/* Источник — откуда книга взялась: читатель, который её принёс,
              или адрес, откуда её забрали. У внешнего это ссылка. */}
          {book.source && (
            <>
              <dt>Источник</dt>
              <dd>
                {book.sourceUrl ? (
                  <a
                    href={book.sourceUrl}
                    target="_blank"
                    rel="noreferrer noopener"
                  >
                    {book.source}
                  </a>
                ) : (
                  book.source
                )}
              </dd>
            </>
          )}
          {addedAt && (
            <>
              <dt>Добавлена</dt>
              <dd>{addedAt}</dd>
            </>
          )}
        </dl>

        {book.desc && <p className="detailsDesc">{book.desc}</p>}

        <div className="detailsActions">
          <button onClick={() => ref.current.close()}>Закрыть</button>
          {book.canDelete && (
            <button
              className="deleteBook"
              onClick={() => onDelete(book)}
            >
              Удалить книгу
            </button>
          )}
        </div>
      </div>
    </dialog>
  );
};
