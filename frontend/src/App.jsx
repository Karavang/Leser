import { Navigate, Outlet, Route, Routes, useLocation } from "react-router-dom";
import Auth from "./components/auth/Auth";
import { Reader } from "./components/Reader";
import UserPage from "./components/UserPage";
import "./App.scss";
import { Home } from "./Home";
import { t } from "./i18n";
import { useOffline } from "./offline";

const NET_TEXT = { offline: "offlineMode", checking: "netChecking", back: "netBack" };

const NetStatus = () => {
  const state = useOffline();
  const { pathname } = useLocation();
  if (!state || (state === "offline" && pathname.startsWith("/book/"))) return null;
  return (
    <div className={`offlineBar ${state}`} role="status" aria-live="polite">
      <i />
      {t(NET_TEXT[state])}
    </div>
  );
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
      <NetStatus />
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
