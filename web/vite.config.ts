import { readFileSync, readdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

/** Emit sw.js from sw.template.js with a precache list of every built file plus the public shell files. */
function serviceWorker(): Plugin {
  return {
    name: "op-service-worker",
    apply: "build",
    generateBundle(_options, bundle) {
      const publicFiles = readdirSync("public").filter((f) => f !== "sw.js");
      const files = ["/", "/index.html", ...Object.keys(bundle).map((f) => `/${f}`), ...publicFiles.map((f) => `/${f}`)];
      const unique = [...new Set(files)].sort();
      // Hashed bundle names change with their content; public files keep their names, so hash their bytes too.
      const hash = createHash("sha256").update(unique.join("\n"));
      for (const f of publicFiles) hash.update(readFileSync(`public/${f}`));
      const version = hash.digest("hex").slice(0, 12);
      const source = readFileSync("sw.template.js", "utf8")
        .replace("__VERSION__", version)
        .replace("__PRECACHE__", JSON.stringify(unique));
      this.emitFile({ type: "asset", fileName: "sw.js", source });
    },
  };
}

// Static bundle only: no SSR, no meta-framework. /api and /ws go to the Rust server in dev.
export default defineConfig({
  plugins: [svelte(), serviceWorker()],
  server: {
    proxy: {
      "/api": { target: "http://127.0.0.1:3000", ws: true },
    },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
