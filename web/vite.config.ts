import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// The generated client is compiled from its TypeScript source, so it needs no
// build of its own. In development the server answers /api and /manage.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@afterimage/management-client": fileURLToPath(
        new URL("../gen/management-client/src/index.ts", import.meta.url),
      ),
    },
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:8080",
      "/manage": "http://127.0.0.1:8080",
    },
  },
});
