import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Static bundle only: no SSR, no meta-framework. /api and /ws go to the Rust server in dev.
export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      "/api": "http://127.0.0.1:3000",
      "/ws": { target: "ws://127.0.0.1:3000", ws: true },
    },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
