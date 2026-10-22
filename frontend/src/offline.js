import axios from "axios";
import { useSyncExternalStore } from "react";

const KEY = "offlineMode";
// ponytail: fixed timings; derive them from past response times if they misfire
const SLOW = 2500;
const GRACE = 15000;
const PROBE_EVERY = 3000;
const RETRY_EVERY = 10000;
const PROBE_TIMEOUT = 5000;
const listeners = new Set();
let net = "";
let lostAt = 0;
let timer;
let probing = false;

export const canOffline = "serviceWorker" in navigator;

export const isOffline = () => localStorage.getItem(KEY) === "1";

const emit = () => listeners.forEach((listener) => listener());

export const subscribe = (listener) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

const state = () => (isOffline() ? "offline" : net);

export const useOffline = () => useSyncExternalStore(subscribe, state);

export const tellWorker = () => {
  if (canOffline) {
    navigator.serviceWorker.ready.then((reg) =>
      reg.active.postMessage({ offline: isOffline() }),
    );
  }
};

const setMode = (on) => {
  if (on) localStorage.setItem(KEY, "1");
  else localStorage.removeItem(KEY);
  tellWorker();
};

const show = (next) => {
  net = next;
  emit();
};

const probe = async () => {
  const abort = new AbortController();
  const stop = setTimeout(() => abort.abort(), PROBE_TIMEOUT);
  try {
    return (await fetch("/api/health", { cache: "no-store", signal: abort.signal })).ok;
  } catch {
    return false;
  } finally {
    clearTimeout(stop);
  }
};

export const checkNetwork = async (now = false) => {
  if (!canOffline || probing) return;
  probing = true;
  clearTimeout(timer);
  const up = await probe();
  probing = false;
  if (up) {
    lostAt = 0;
    if (isOffline()) {
      setMode(false);
      show("back");
      setTimeout(() => net === "back" && show(""), 3000);
    } else if (net === "checking") show("");
    return;
  }
  if (isOffline()) {
    timer = setTimeout(checkNetwork, RETRY_EVERY);
    return;
  }
  lostAt ||= Date.now();
  if (now || Date.now() - lostAt >= GRACE) {
    lostAt = 0;
    setMode(true);
    show("");
    timer = setTimeout(checkNetwork, RETRY_EVERY);
    return;
  }
  show("checking");
  timer = setTimeout(checkNetwork, PROBE_EVERY);
};

const suspect = () => {
  if (!isOffline() && net !== "checking") checkNetwork();
};

axios.interceptors.request.use((config) => {
  if (!config.patient) config.slowTimer = setTimeout(suspect, SLOW);
  return config;
});
axios.interceptors.response.use(
  (response) => {
    clearTimeout(response.config.slowTimer);
    return response;
  },
  (error) => {
    clearTimeout(error.config?.slowTimer);
    if (!error.response && !axios.isCancel(error)) suspect();
    return Promise.reject(error);
  },
);
window.addEventListener("offline", suspect);
window.addEventListener("online", () => checkNetwork());
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible" && isOffline()) checkNetwork();
});

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

const bookOf = (request) =>
  /^\/api\/(?:read|img|imgToken|progress)\/([^/]+)/.exec(new URL(request.url).pathname)?.[1];

// ponytail: sizes come from reading every cached body; keep a size index if the list grows slow
export const downloads = async () => {
  if (!window.caches) return [];
  const cache = await caches.open("api");
  const have = await downloadedIds();
  const library = await cache
    .match("/api/getAll")
    .then((r) => (r ? r.json() : []))
    .catch(() => []);
  const titles = new Map(library.map((book) => [String(book.id), book]));
  const sizes = new Map();
  for (const request of await cache.keys()) {
    const id = bookOf(request);
    if (!have.has(id)) continue;
    const body = await (await cache.match(request)).blob();
    sizes.set(id, (sizes.get(id) || 0) + body.size);
  }
  return [...sizes].map(([id, size]) => ({
    id,
    size,
    title: titles.get(id)?.title || id,
    author: titles.get(id)?.author || "",
  }));
};

export const forgetDownload = async (id) => {
  const cache = await caches.open("api");
  const keys = await cache.keys();
  await Promise.all(keys.filter((r) => bookOf(r) === id).map((r) => cache.delete(r)));
};
