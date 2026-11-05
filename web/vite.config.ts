import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";

const client = fileURLToPath(new URL("../gen/management-client/src/", import.meta.url));

/** The generated client's package maps its runtime configuration to the browser
 * one through its `browser` field, which applies to its built files. Compiled
 * from source, the same mapping happens here. */
function browserRuntimeConfig(): Plugin {
  return {
    name: "afterimage-client-browser-runtime",
    enforce: "pre",
    resolveId(source, importer) {
      if (source === "./runtimeConfig" && importer?.startsWith(client)) {
        return `${client}runtimeConfig.browser.ts`;
      }
      return null;
    },
  };
}

// The generated client is compiled from its TypeScript source, so it needs no
// build of its own. In development the server answers /api and /manage.
export default defineConfig({
  plugins: [browserRuntimeConfig(), react()],
  resolve: {
    alias: {
      "@afterimage/management-client": `${client}index.ts`,
    },
  },
  build: {
    // The editor's chunk carries CodeMirror, the document schema's completion and
    // lint, and the highlighter its help text uses; it loads only when a screen
    // opens.
    chunkSizeWarningLimit: 1024,
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:8080",
      "/manage": "http://127.0.0.1:8080",
    },
  },
});
