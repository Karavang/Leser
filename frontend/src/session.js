// Токен stateless, сервер сессий не хранит — выход это просто забыть его тут.
export function clearSession() {
  localStorage.removeItem("token");
  localStorage.removeItem("user");
  localStorage.removeItem("unsentPages");
  localStorage.removeItem("offlineMode");
  if (window.caches) caches.delete("api");
}
