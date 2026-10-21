const SHELL = "shell";
const API = "api";
const MODE = "/__offline";

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(SHELL).then(async (cache) => {
      const html = await (await fetch("/", { cache: "no-cache" })).text();
      const assets = [...html.matchAll(/(?:src|href)="(\/assets\/[^"]+)"/g)].map((m) => m[1]);
      await cache.addAll(["/favicon.png", ...assets]);
      await cache.put("/", new Response(html, { headers: { "Content-Type": "text/html" } }));
    }),
  );
  self.skipWaiting();
});

self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

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
    if (response.ok || response.type === "opaque") {
      const copy = response.clone();
      caches.open(name).then((cache) => cache.put(key, copy));
    }
    return response;
  });

const networkFirst = async (request, key, name) => {
  try {
    return await fetchAndKeep(request, key, name);
  } catch (error) {
    const cached = await fromCache(key, name);
    if (cached) return cached;
    throw error;
  }
};

// ponytail: hashed assets pile up across deploys; prune by index.html contents if storage matters
const cacheFirst = async (request, key, name) =>
  (await fromCache(key, name)) || fetchAndKeep(request, key, name);

const respond = async (request, key, name) =>
  (await offlineMode()) ? cacheFirst(request, key, name) : networkFirst(request, key, name);

self.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET") return;
  const url = new URL(request.url);

  if (url.origin !== location.origin) {
    if (/fonts\.(googleapis|gstatic)\.com$/.test(url.hostname)) {
      event.respondWith(cacheFirst(request, request, SHELL));
    }
    return;
  }
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
