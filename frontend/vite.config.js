import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import legacy from "@vitejs/plugin-legacy";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), legacy({ additionalLegacyPolyfills: ["whatwg-fetch"] })],
  // Фронт зовёт /api/* — тот же origin, поэтому CORS не участвует.
  // В dev проксирует vite, в контейнере nginx, на проде — nginx на leser.karavang.com.
  server: {
    proxy: {
      "/api": {
        target: process.env.VITE_API_TARGET || "http://localhost:4444",
        rewrite: (p) => p.replace(/^\/api/, ""),
      },
    },
  },
});
