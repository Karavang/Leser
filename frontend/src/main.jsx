import axios from "axios";
import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App.jsx";
import { lang } from "./i18n.js";
import { applyTheme, theme } from "./theme.js";
import { isOffline, subscribe, tellWorker } from "./offline.js";
import { sendUnsent } from "./sync.js";

// Язык страницы — не только для порядка: от него зависят переносы, проверка
// орфографии в полях и то, каким голосом читает скринридер.
document.documentElement.lang = lang;
applyTheme(theme);

// На каком языке отвечать, сервер узнаёт отсюда. Accept-Language не годится:
// его ставит браузер, а читатель мог выбрать в настройках другой язык.
axios.defaults.headers.common["X-Lang"] = lang;

if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => navigator.serviceWorker.register("/sw.js"));
}
tellWorker();
sendUnsent();
window.addEventListener("online", sendUnsent);
subscribe(() => !isOffline() && sendUnsent());

// BrowserRouter, а не Hash: nginx и vite уже отдают index.html на любой путь
// (try_files), так что ссылка вида /book/… открывается напрямую.
ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>,
);
