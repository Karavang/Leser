import axios from "axios";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { BookDetails } from "./BookDetails";

export const BooksList = ({ books, onDeleted, onShelf }) => {
  const navigate = useNavigate();

  const [isEnter, setEnter] = useState(null);
  const [details, setDetails] = useState(null);
  const [error, setError] = useState(null);
  const token = JSON.parse(localStorage.getItem("token"));

  // Право на удаление считает бэк (canDelete): своя книга либо админ.
  // Кнопка живёт в подробностях, а не на карточке: случайно снести книгу,
  // целясь в неё же, чтобы почитать, — слишком легко.
  const remove = async (book) => {
    if (!confirm(`Удалить «${book.title}»? Отменить нельзя.`)) return;
    try {
      await axios.delete(`/api/deleteOne/${book.filename}`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      setError(null);
      setDetails(null);
      onDeleted(book.filename);
    } catch (e) {
      setError(e.response?.data?.message ?? "Не удалось удалить книгу");
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
        url: `/api/myBooks/${book.filename}`,
        headers: { Authorization: `Bearer ${token}` },
      });
      setError(null);
      onShelf(book.filename, mine);
      // подробности открыты по этой же книге — отметку в них тоже поправить
      setDetails((d) => (d && d.id === book.id ? { ...d, mine } : d));
    } catch (e) {
      setError(e.response?.data?.message ?? "Не удалось изменить «мои книги»");
    }
  };

  return (
    <div className="bookList">
      {error && <p className="error">{error}</p>}
      {books.length > 0 ? (
        <ul>
          {books.map((book) => (
            <li
              key={book.id}
              onMouseEnter={() => setEnter(book.id)}
              onMouseLeave={() => setEnter(null)}
              onClick={() => navigate(`/book/${book.filename}`)}
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
                {isEnter === book.id && (
                  <button
                    className="detailsButton"
                    onClick={(e) => {
                      // иначе клик дойдёт до li и откроет читалку
                      e.stopPropagation();
                      setDetails(book);
                    }}
                  >
                    Подробнее
                  </button>
                )}
                {/* Видно всегда, а не по наведению: по этой отметке и понятно,
                    какие книги у читателя свои. */}
                <button
                  className={`shelfButton ${book.mine ? "on" : ""}`}
                  title={book.mine ? "Убрать из моих книг" : "В мои книги"}
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
      ) : (
        <p>No books available.</p>
      )}
      {details && (
        <BookDetails
          book={details}
          onClose={() => setDetails(null)}
          onDelete={remove}
          onShelf={shelve}
        />
      )}
    </div>
  );
};
