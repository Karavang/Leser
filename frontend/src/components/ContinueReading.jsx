import { useNavigate } from "react-router-dom";
import { t } from "../i18n";

export const ContinueReading = ({ books }) => {
  const navigate = useNavigate();
  if (!books.length) return null;
  return (
    <section className="continue" aria-label={t("continueReading")}>
      <h2>{t("continueReading")}</h2>
      <div>
        {books.map((book) => {
          const percent = Math.round((book.progress ?? 0) * 100);
          return (
            <button key={book.id} onClick={() => navigate(`/book/${book.id}`)}>
              <b>{book.title}</b>
              <span>{book.author}</span>
              <span className="continueBar">
                <span>
                  <i style={{ width: `${percent}%` }} />
                </span>
                {percent}%
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
};
