self.BUILD = "dev";
const SHELL = "shell";
const API = "api";
const MODE = "/__offline";
const PATIENCE = 3000;

const linked = (text, pattern) => [...text.matchAll(pattern)].map((m) => m[1]);

const shellAssets = async (html, read) => {
  const assets = linked(html, /(?:src|href)="(\/assets\/[^"]+)"/g);
  const styles = await Promise.all(assets.filter((a) => a.endsWith(".css")).map(read));
  return [...new Set([...assets, ...styles.flatMap((css) => linked(css, /url\((\/assets\/[^)]+)\)/g))])];
};

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(SHELL).then(async (cache) => {
      const html = await (await fetch("/", { cache: "no-cache" })).text();
      const assets = await shellAssets(html, (url) => fetch(url).then((r) => r.text()));
      const now = assets.filter((a) => !/\.woff$|greek|vietnamese|cyrillic-ext/.test(a));
      await cache.addAll(["/favicon.png", ...now]);
      await cache.put("/", new Response(html, { headers: { "Content-Type": "text/html" } }));
    }),
  );
  self.skipWaiting();
});

const prune = async () => {
  const cache = await caches.open(SHELL);
  const page = await cache.match("/");
  if (!page) return;
  const read = (url) => cache.match(url).then((r) => (r ? r.text() : ""));
  const keep = new Set(await shellAssets(await page.text(), read));
  const keys = await cache.keys();
  await Promise.all(
    keys
      .filter((request) => {
        const path = new URL(request.url).pathname;
        return path.startsWith("/assets/") && !keep.has(path);
      })
      .map((request) => cache.delete(request)),
  );
};

self.addEventListener("activate", (event) =>
  event.waitUntil(Promise.all([self.clients.claim(), prune()])),
);

self.addEventListener("message", (event) => {
  const offline = event.data && event.data.offline;
  event.waitUntil(
    caches
      .open(SHELL)
      .then((cache) => (offline ? cache.put(MODE, new Response("1")) : cache.delete(MODE))),
  );
});

const offlineMode = () => caches.open(SHELL).then((cache) => cache.match(MODE)).then(Boolean);

const fromCache = (key, name) =>
  caches.open(name).then((cache) => cache.match(key, { ignoreVary: true }));

const fetchAndKeep = (request, key, name) =>
  fetch(request).then((response) => {
    if (response.ok) {
      const copy = response.clone();
      caches.open(name).then((cache) => cache.put(key, copy));
    }
    return response;
  });

const networkFirst = async (request, key, name) => {
  const network = fetchAndKeep(request, key, name);
  const cached = await fromCache(key, name);
  if (!cached) return network;
  const late = new Promise((resolve) => setTimeout(resolve, PATIENCE, cached));
  return Promise.race([network.catch(() => cached), late]);
};

const cacheFirst = async (request, key, name) =>
  (await fromCache(key, name)) || fetchAndKeep(request, key, name);

const respond = async (request, key, name) =>
  (await offlineMode()) ? cacheFirst(request, key, name) : networkFirst(request, key, name);

self.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== location.origin) return;

  if (request.mode === "navigate") {
    event.respondWith(respond(request, "/", SHELL));
  } else if (url.pathname.startsWith("/assets/")) {
    event.respondWith(cacheFirst(request, request, SHELL));
  } else if (url.pathname.startsWith("/api/img/")) {
    event.respondWith(cacheFirst(request, url.pathname, API));
  } else if (url.pathname.startsWith("/api/") && url.pathname !== "/api/export") {
    event.respondWith(respond(request, request, API));
  }
});
