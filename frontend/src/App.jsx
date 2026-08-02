import { Navigate, Outlet, Route, Routes } from "react-router-dom";
import Auth from "./components/auth/Auth";
import { Reader } from "./components/Reader";
import UserPage from "./components/UserPage";
import "./App.scss";
import { Home } from "./Home";

/// Заслон перед всем, что требует токена. Без него выход из аккаунта оставлял
/// на экране страницу, которой уже нечего показать: данные из localStorage
/// стёрты, а компонент всё ещё смонтирован.
const RequireAuth = () => {
  const token = localStorage.getItem("token");
  return token ? <Outlet /> : <Navigate to="/login" replace />;
};

function App() {
  return (
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
          path="/book/:filename"
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
  );
}

export default App;
