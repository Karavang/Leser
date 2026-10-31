import axios from "axios";
import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { BookDetails } from "./BookDetails";
import { t } from "../i18n";
import { seriesGroups } from "../series";

const HOVER = matchMedia("(hover: hover)").matches;

const savedBySeries = () => {
  try {
    return localStorage.getItem("bySeries") === "1";
  } catch {
    return false;
  }
};

export const BooksList = ({ books, onDeleted, onChange }) => {
  const navigate = useNavigate();

  const [isEnter, setEnter] = useState(null);
  const [details, setDetails] = useState(null);
  const [error, setError] = useState(null);
  const [bySeries, setBySeries] = useState(savedBySeries);
  const groups = useMemo(
    () => (bySeries ? seriesGroups(books) : [["", books]]),
    [books, bySeries],
  );

  const toggleSeries = () => {
    const next = !bySeries;
    setBySeries(next);
    try {
      localStorage.setItem("bySeries", next ? "1" : "0");
    } catch {
      return;
    }
  };
  const token = JSON.parse(localStorage.getItem("token"));

  // Право на удаление считает бэк (canDelete): своя книга либо админ.
  // Кнопка живёт в подробностях, а не на карточке: случайно снести книгу,
  // целясь в неё же, чтобы почитать, — слишком легко.
  const remove = async (book) => {
    if (!confirm(t("confirmDeleteBook", book.title))) return;
    try {
      await axios.delete(`/api/deleteOne/${book.id}`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      setError(null);
      setDetails(null);
      onDeleted(book.id);
    } catch (e) {
      setError(e.response?.data?.message ?? t("deleteBookFailed"));
    }
  };

  // Полка — не права: отложить себе можно любую книгу из библиотеки,
  // и на саму книгу это никак не влияет. Поэтому и подтверждения нет:
  // передумать — это тот же клик обратно.
  const shelve = async (book) => {
    const mine = !book.mine;
    try {
      await axios({
        method: mine ? "post" : "delete",
        url: `/api/myBooks/${book.id}`,
        headers: { Authorization: `Bearer ${token}` },
      });
      setError(null);
      onChange(book.id, { mine });
      // подробности открыты по этой же книге — отметку в них тоже поправить
      setDetails((d) => (d && d.id === book.id ? { ...d, mine } : d));
    } catch (e) {
      setError(e.response?.data?.message ?? t("shelfFailed"));
    }
  };

  const publish = async (book) => {
    const open = !book.public;
    try {
      await axios.post(
        `/api/admin/books/${book.id}/public`,
        { public: open },
        { headers: { Authorization: `Bearer ${token}` } },
      );
      setError(null);
      onChange(book.id, { public: open });
      setDetails((d) => (d && d.id === book.id ? { ...d, public: open } : d));
    } catch (e) {
      setError(e.response?.data?.message ?? t("publicFailed"));
    }
  };

  return (
    <div className="bookList">
      {error && <p className="error">{error}</p>}
      {books.length > 0 && (
        <button
          className={`seriesToggle ${bySeries ? "on" : ""}`}
          aria-pressed={bySeries}
          onClick={toggleSeries}
        >
          {t("bySeries")}
        </button>
      )}
      {books.length > 0 ? (
        groups.map(([name, group]) => (
          <section key={name.toLowerCase()}>
            {bySeries && (
              <h2 className="seriesTitle">{name || t("noSeries")}</h2>
            )}
            <ul>
              {group.map((book) => (
                <li
                  key={book.id}
                  onMouseEnter={() => HOVER && setEnter(book.id)}
                  onMouseLeave={() => HOVER && setEnter(null)}
                  onClick={() => navigate(`/book/${book.id}`)}
                >
                  {/* Карточка — название и автор, дальше подробности. Даты тут
                  не место: в метаданных она произвольной формы и въезжала
                  на карточку сырой строкой вида 2015-06-15T00:00:00+00:00. */}
                  {isEnter === book.id ? (
                    <p>{book.desc}</p>
                  ) : (
                    <>
                      <h2>{book.title}</h2>
                      <h3>{book.author}</h3>
                    </>
                  )}
                  <div className="bookFooter">
                    {(isEnter === book.id || !HOVER) && (
                      <button
                        className="detailsButton"
                        title={t("more")}
                        aria-label={t("more")}
                        onClick={(e) => {
                          // иначе клик дойдёт до li и откроет читалку
                          e.stopPropagation();
                          setDetails(book);
                        }}
                      >
                        {HOVER ? t("more") : "⋯"}
                      </button>
                    )}
                    {/* Видно всегда, а не по наведению: по этой отметке и понятно,
                    какие книги у читателя свои. */}
                    <button
                      className={`shelfButton ${book.mine ? "on" : ""}`}
                      title={book.mine ? t("shelfRemove") : t("shelfAdd")}
                      aria-pressed={book.mine}
                      onClick={(e) => {
                        e.stopPropagation();
                        shelve(book);
                      }}
                    >
                      {book.mine ? "★" : "☆"}
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          </section>
        ))
      ) : (
        <p>{t("noBooks")}</p>
      )}
      {details && (
        <BookDetails
          book={details}
          onClose={() => setDetails(null)}
          onDelete={remove}
          onShelf={shelve}
          onPublic={publish}
        />
      )}
    </div>
  );
};
