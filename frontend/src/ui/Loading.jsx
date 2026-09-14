import { t } from "../i18n";

// Скелеты вместо спиннера: страница стоит на месте, а место содержимого
// занимают серые полосы с бликом. Ничего не прыгает и не перекрывает экран,
// слово «загрузка» остаётся только для скринридера. Полосы — <b> (заголовок)
// и <i> (строка), ширина задаёт «текст» разной длины.

const CARDS = ["80%", "60%", "90%", "70%", "85%", "50%"];
const LEFT = ["60%", "100%", "95%", "88%", "100%", "70%", "92%", "100%", "60%"];
const RIGHT = ["100%", "85%", "100%", "93%", "78%", "100%", "90%", "66%", "100%"];

const lines = (widths) => widths.map((w, i) => <i key={i} style={{ width: w }} />);

/// Библиотека: та же сетка карточек, что у .bookList, — книги проявятся
/// на своих местах. Шапка с поиском остаётся живой, её рисует Home.
export const Loading = () => (
  <div className="bookList skeleton" role="status" aria-label={t("loading")}>
    <ul>
      {CARDS.map((w) => (
        <li key={w}>
          <b style={{ width: w }} />
          <i style={{ width: "55%" }} />
        </li>
      ))}
    </ul>
  </div>
);

/// Читалка: шапка, разворот из двух колонок строк и полоса внизу — там,
/// где потом встанут название, текст и прогресс по книге. Тема — та же,
/// что у читалки, иначе на тёмной книге мигнёт светлый экран.
export const LoadingBook = ({ theme }) => (
  <div
    className={`reader skeleton ${theme}`}
    role="status"
    aria-label={t("loadingBook")}
  >
    <header className="readerBar">
      <b style={{ width: "30%" }} />
      <b style={{ width: "18%" }} />
    </header>
    <div className="readerBody skeletonPages">
      <div>{lines(LEFT)}</div>
      <div>{lines(RIGHT)}</div>
    </div>
    <div className="readerFoot">
      <span className="busyLine" />
    </div>
  </div>
);

/// Бегущая полоса под верхним краем: запрос идёт, а страница уже показана —
/// поиск, смена вкладки, возврат на вкладку. Скелет тут был бы лишним:
/// книги уже на экране, и прятать их ради новых незачем.
export const BusyLine = () => (
  <div className="busyLine top" role="progressbar" aria-label={t("loading")} />
);
