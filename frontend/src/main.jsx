import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App.jsx";

// BrowserRouter, а не Hash: nginx и vite уже отдают index.html на любой путь
// (try_files), так что ссылка вида /book/… открывается напрямую.
ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>,
);
