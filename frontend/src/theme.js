/// Тема одна на всё приложение — та же, что в читалке. Класс висит на <html>,
/// цвета берутся из переменных в App.scss, поэтому переключение это одна
/// строка и никакой перерисовки.

export const THEMES = ["day", "sepia", "night"];

/// Ключ старый, `readerTheme`: тема родилась в читалке, и переименование
/// сбросило бы выбор у тех, кто уже читает.
export const theme = THEMES.includes(localStorage.getItem("readerTheme"))
  ? localStorage.getItem("readerTheme")
  : "sepia";

export const applyTheme = (name) => {
  document.documentElement.classList.remove(...THEMES);
  document.documentElement.classList.add(name);
  localStorage.setItem("readerTheme", name);
};
