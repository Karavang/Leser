import axios from "axios";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { clearSession } from "../session";
import LogoutButton from "./LogoutButton";
import { lang, setLang, t } from "../i18n";
import { applyTheme, theme as saved, THEMES } from "../theme";

const UserPage = () => {
  const navigate = useNavigate();
  const user = JSON.parse(localStorage.getItem("user"));
  const [books, setBooks] = useState([]);
  const [theme, setTheme] = useState(saved);
  const [error, setError] = useState(null);

  // Кого удалять, сервер берёт из токена — тело запроса не нужно.
  // Закладки уезжают вместе с аккаунтом, загруженные книги остаются в библиотеке.
  const deleteAccount = async () => {
    if (!confirm(t("confirmDeleteAccount"))) return;
    try {
      await axios.delete("/api/account", {
        headers: { Authorization: `Bearer ${user.token}` },
      });
      clearSession();
      navigate("/login", { replace: true });
    } catch (e) {
      setError(e.response?.data?.message ?? t("deleteAccountFailed"));
    }
  };

  useEffect(() => {
    const fetchBooks = async () => {
      try {
        const response = await axios.get(
          "/api/booksInRead",
          {
            headers: { Authorization: `Bearer ${user.token}` },
          },
        );
        setBooks(response.data);
      } catch (error) {
        console.error("Error fetching books:", error);
      }
    };

    fetchBooks();
  }, [user.token]);

  return (
    <div className="userPage">
      <div className="userDetails">
        <h1>{user.username}</h1>
        <h2>{user.email}</h2>
        <h3>{t("booksInRead")}</h3>
        <ul style={{ maxHeight: "400px", overflowY: "auto" }}>
          {books.map((book) => (
            <li
              key={book.id}
              onClick={() => navigate(`/book/${book.filename}`)}
            >
              <h4>{book.title}</h4>
              <p>{book.author}</p>
            </li>
          ))}
        </ul>
        {/* details вместо кнопки с useState: открытие/закрытие, клавиатура
            и доступность уже есть в браузере, писать нечего */}
        <details className="settings">
          <summary>
            <svg
              xmlns="http://www.w3.org/2000/svg"
              height="20px"
              width="20px"
              viewBox="0 -960 960 960"
            >
              <path d="m388-80-20-126q-19-7-40-19t-37-25l-118 54-93-164 108-79q-2-9-2.5-20.5T185-480q0-9 .5-20.5T188-521L80-600l93-164 118 54q16-13 37-25t40-18l20-127h184l20 126q19 7 40.5 18.5T669-710l118-54 93 164-108 77q2 10 2.5 21.5t.5 21.5q0 10-.5 21t-2.5 21l108 78-93 164-118-54q-16 13-36.5 25.5T592-206L572-80H388Zm92-270q54 0 92-38t38-92q0-54-38-92t-92-38q-54 0-92 38t-38 92q0 54 38 92t92 38Z" />
            </svg>
            {t("settings")}
          </summary>

          <div className="settingsRow">
            <span>
              {t("theme")}
              <small>{t("themeHint")}</small>
            </span>
            <div className="themePick">
              {THEMES.map((name) => (
                <button
                  key={name}
                  className={`swatch ${name} ${theme === name ? "active" : ""}`}
                  title={t(`theme${name[0].toUpperCase()}${name.slice(1)}`)}
                  aria-pressed={theme === name}
                  onClick={() => {
                    applyTheme(name);
                    setTheme(name);
                  }}
                />
              ))}
            </div>
          </div>

          <div className="settingsRow">
            <span>
              {t("interfaceLanguage")}
              <small>{t("languageHint")}</small>
            </span>
            {/* select, а не свои кнопки: список языков — это список,
                и на телефоне его рисует сама система */}
            <select
              className="langPick"
              value={lang}
              onChange={(e) => setLang(e.target.value)}
            >
              <option value="ru">Русский</option>
              <option value="en">English</option>
            </select>
          </div>

          <div className="settingsRow">
            <span>
              {t("logout")}
              <small>{t("logoutHint")}</small>
            </span>
            <LogoutButton />
          </div>

          <div className="settingsRow">
            <span>
              {t("deleteAccount")}
              <small>{t("deleteAccountHint")}</small>
            </span>
            <button
              className="deleteAccount"
              onClick={deleteAccount}
            >
              {t("delete")}
            </button>
          </div>

          {error && <p className="error">{error}</p>}
        </details>
      </div>
      <div
        className="closeButton"
        onClick={() => navigate("/")}
        title={t("toLibrary")}
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          height="40px"
          viewBox="0 -960 960 960"
          width="40px"
        >
          <path d="M226.67-186.67h140v-246.66h226.66v246.66h140v-380L480-756.67l-253.33 190v380ZM160-120v-480l320-240 320 240v480H526.67v-246.67h-93.34V-120H160Zm320-352Z" />
        </svg>
      </div>
    </div>
  );
};

export default UserPage;
