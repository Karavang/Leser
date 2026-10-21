// Токен stateless, сервер сессий не хранит — выход это просто забыть его тут.
export function clearSession() {
  localStorage.removeItem("token");
  localStorage.removeItem("user");
  forgetDevice();
}

export function expireSession() {
  localStorage.removeItem("token");
}

export function forgetDevice() {
  localStorage.removeItem("unsentPages");
  localStorage.removeItem("offlineMode");
  if (window.caches) caches.delete("api");
}
