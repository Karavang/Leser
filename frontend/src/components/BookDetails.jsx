import axios from "axios";
import { useEffect, useRef, useState } from "react";
import { t } from "../i18n";
import { canOffline, download, downloadedIds } from "../offline";
import { hoursMinutes, isAudio } from "../audio";

const pad = (n) => String(n).padStart(2, "0");

/// Момент времени с бэка (rfc3339) — в дд.мм.гггг в зоне читателя.
const stamp = (iso) => {
  const d = new Date(iso);
  return isNaN(d)
    ? null
    : `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`;
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
export const BookDetails = ({ book, onClose, onDelete, onShelf, onPublic }) => {
  const ref = useRef(null);
  const [saved, setSaved] = useState(null);

  useEffect(() => {
    downloadedIds().then((have) =>
      setSaved(have.has(String(book.id)) ? "done" : null),
    );
  }, [book.id]);

  const [stored, setStored] = useState(null);

  const store = async () => {
    setStored("busy");
    const auth = { headers: { Authorization: `Bearer ${JSON.parse(localStorage.getItem("token"))}` } };
    try {
      while ((await axios.post(`/api/store/${book.id}`, null, auth)).status === 202) {
        await new Promise((resolve) => setTimeout(resolve, 4000));
      }
      setStored("done");
    } catch (e) {
      setStored(e.response?.data?.message ?? t("downloadFailed"));
    }
  };

  const save = () => {
    setSaved("busy");
    download(book.id, isAudio(book))
      .then(() => setSaved("done"))
      .catch(() => setSaved("failed"));
  };

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
          {isAudio(book) && book.seconds > 0 && (
            <>
              <dt>{t("duration")}</dt>
              <dd>{t("hoursMinutes", ...hoursMinutes(book.seconds))}</dd>
            </>
          )}
          {book.series && (
            <>
              <dt>{t("series")}</dt>
              <dd>
                {book.series}
                {book.seriesIndex != null && ` #${book.seriesIndex}`}
              </dd>
            </>
          )}
          {book.genres?.length > 0 && (
            <>
              <dt>{t("genres")}</dt>
              <dd>{book.genres.join(", ")}</dd>
            </>
          )}
          {book.date && (
            <>
              <dt>{t("published")}</dt>
              <dd>{published(book.date)}</dd>
            </>
          )}
          {book.lang && (
            <>
              <dt>{t("language")}</dt>
              <dd>{book.lang}</dd>
            </>
          )}
          {/* Источник — откуда книга взялась: читатель, который её принёс,
              или адрес, откуда её забрали. У внешнего это ссылка. */}
          {book.source && (
            <>
              <dt>{t("source")}</dt>
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
          {book.publicBasis && (
            <>
              <dt>{book.publicBasis.includes("†") ? t("publicDomain") : t("license")}</dt>
              <dd>
                {book.publicBasis.split(", ").map((who) =>
                  /^Q\d+ /.test(who) ? (
                    <a
                      key={who}
                      href={`https://www.wikidata.org/wiki/${who.split(" ")[0]}`}
                      target="_blank"
                      rel="noreferrer noopener"
                    >
                      {who}{" "}
                    </a>
                  ) : (
                    <span key={who}>{who} </span>
                  ),
                )}
              </dd>
            </>
          )}
          {addedAt && (
            <>
              <dt>{t("addedAt")}</dt>
              <dd>{addedAt}</dd>
            </>
          )}
        </dl>

        {book.desc && <p className="detailsDesc">{book.desc}</p>}
        {book.linked && stored !== "done" && <p className="detailsNote">{t("linkedNote")}</p>}
        {stored === "busy" && <p className="detailsNote">{t("importAudioWait")}</p>}
        {stored && !["busy", "done"].includes(stored) && <p className="error">{stored}</p>}

        <div className="detailsActions">
          <button onClick={() => ref.current.close()}>{t("close")}</button>
          {book.linked && book.public && stored !== "done" && (
            <button onClick={store} disabled={stored === "busy"}>
              {stored === "busy" ? t("downloading") : t("storeHere")}
            </button>
          )}
          {canOffline && !(book.linked && stored !== "done") && (
            <button
              onClick={save}
              disabled={saved === "busy" || saved === "done"}
            >
              {saved === "busy"
                ? t("downloading")
                : saved === "done"
                  ? t("downloaded")
                  : saved === "failed"
                    ? t("downloadFailed")
                    : t("download")}
            </button>
          )}
          {/* Убрать с полки — не то же самое, что удалить: книга остаётся
              в библиотеке, поэтому и кнопка обычная, без предупреждения. */}
          <button onClick={() => onShelf(book)}>
            {book.mine ? t("shelfRemoveShort") : t("shelfAdd")}
          </button>
          {book.canPublish && (
            <button onClick={() => onPublic(book)}>
              {book.public ? t("makePrivate") : t("makePublic")}
            </button>
          )}
          {book.canDelete && (
            <button className="deleteBook" onClick={() => onDelete(book)}>
              {t("deleteBook")}
            </button>
          )}
        </div>
      </div>
    </dialog>
  );
};
