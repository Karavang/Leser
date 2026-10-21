import { Navigate, Outlet, Route, Routes } from "react-router-dom";
import Auth from "./components/auth/Auth";
import { Reader } from "./components/Reader";
import UserPage from "./components/UserPage";
import "./App.scss";
import { Home } from "./Home";
import { t } from "./i18n";
import { dismissSlow, setOffline, useOffline } from "./offline";

const OfflineBar = () => {
  const state = useOffline();
  if (state === "offline") {
    return (
      <div className="offlineBar">
        {t("offlineMode")}
        <button onClick={() => setOffline(false)}>{t("goOnline")}</button>
      </div>
    );
  }
  if (state === "slow") {
    return (
      <div className="offlineBar">
        {t("slowNetwork")}
        <button onClick={() => setOffline(true)}>{t("goOffline")}</button>
        <button
          aria-label={t("close")}
          onClick={dismissSlow}
        >
          ✕
        </button>
      </div>
    );
  }
  return null;
};

/// Заслон перед всем, что требует токена. Без него выход из аккаунта оставлял
/// на экране страницу, которой уже нечего показать: данные из localStorage
/// стёрты, а компонент всё ещё смонтирован.
const RequireAuth = () => {
  const token = localStorage.getItem("token");
  return token ? <Outlet /> : <Navigate to="/login" replace />;
};

function App() {
  return (
    <>
      <OfflineBar />
      <Routes>
        <Route
          path="/login"
          element={<Auth />}
        />
        <Route element={<RequireAuth />}>
          <Route
            path="/"
            element={<Home />}
          />
          <Route
            path="/me"
            element={<UserPage />}
          />
          <Route
            path="/book/:id"
            element={<Reader />}
          />
        </Route>
        {/* чужой адрес — не ошибка, просто отправляем в библиотеку */}
        <Route
          path="*"
          element={
            <Navigate
              to="/"
              replace
            />
          }
        />
      </Routes>
    </>
  );
}

export default App;
