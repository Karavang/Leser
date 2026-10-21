import axios from "axios";
import { useSyncExternalStore } from "react";

const KEY = "offlineMode";
// ponytail: fixed threshold; derive it from past response times if it misfires
const SLOW = 4000;
const listeners = new Set();
let slow = false;
let dismissed = false;

export const canOffline = "serviceWorker" in navigator;

export const isOffline = () => localStorage.getItem(KEY) === "1";

const emit = () => listeners.forEach((listener) => listener());

export const subscribe = (listener) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

const state = () => (isOffline() ? "offline" : slow ? "slow" : "");

export const useOffline = () => useSyncExternalStore(subscribe, state);

export const tellWorker = () => {
  if (canOffline) {
    navigator.serviceWorker.ready.then((reg) =>
      reg.active.postMessage({ offline: isOffline() }),
    );
  }
};

export const setOffline = (on) => {
  if (on) localStorage.setItem(KEY, "1");
  else localStorage.removeItem(KEY);
  slow = false;
  tellWorker();
  emit();
};

export const dismissSlow = () => {
  dismissed = true;
  slow = false;
  emit();
};

const offer = () => {
  if (canOffline && !isOffline() && !dismissed && !slow) {
    slow = true;
    emit();
  }
};

axios.interceptors.request.use((config) => {
  if (!config.patient) config.slowTimer = setTimeout(offer, SLOW);
  return config;
});
axios.interceptors.response.use(
  (response) => {
    clearTimeout(response.config.slowTimer);
    return response;
  },
  (error) => {
    clearTimeout(error.config?.slowTimer);
    if (!error.response && !axios.isCancel(error)) offer();
    return Promise.reject(error);
  },
);
window.addEventListener("offline", offer);

export const downloadedIds = async () => {
  if (!window.caches) return new Set();
  const keys = await (await caches.open("api")).keys();
  return new Set(
    keys
      .map(
        (request) =>
          /^\/api\/read\/([^/]+)$/.exec(new URL(request.url).pathname)?.[1],
      )
      .filter(Boolean),
  );
};

export const download = async (id) => {
  const token = JSON.parse(localStorage.getItem("token"));
  const auth = { headers: { Authorization: `Bearer ${token}` }, patient: true };
  const { data } = await axios.get(`/api/read/${id}`, auth);
  const pics = new Set(
    (data.chapters || []).flatMap((c) =>
      [...c.html.matchAll(/src="img\/(\d+)"/g)].map((m) => m[1]),
    ),
  );
  if (!pics.size) return;
  const { data: img } = await axios.get(`/api/imgToken/${id}`, auth);
  await Promise.all(
    [...pics].map((n) =>
      fetch(`/api/img/${id}/${n}?t=${img.token}`).then((r) => {
        if (!r.ok) throw new Error(r.statusText);
      }),
    ),
  );
};
