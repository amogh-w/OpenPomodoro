#!/usr/bin/env node
// Keeps the string catalog honest (U-06): every piece of text a person reads comes from web/src/locales/en.ts, and
// nothing in the catalog is left unused. Reports problems and exits 1 if there are any.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(new URL("..", import.meta.url)), "web", "src");
const catalogPath = join(root, "locales", "en.ts");

function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return files(path);
    return /\.(svelte|ts)$/.test(name) && !/\.test\.ts$/.test(name) && path !== catalogPath ? [path] : [];
  });
}

const sources = files(root).map((path) => ({ path, text: readFileSync(path, "utf8") }));
const keys = [...readFileSync(catalogPath, "utf8").matchAll(/^\s*"([^"]+)":/gm)].map((m) => m[1]);
const problems = [];

// 1. Catalog keys that nothing uses. A key counts as used if its exact name appears, if a template string such as
//    `settings.sounds.${kind}` could build it, or (for plural pairs) if `tn("base"` appears.
const everything = sources.map((s) => s.text).join("\n");
const prefixes = [...everything.matchAll(/`([a-zA-Z_.]+)\$\{/g)].map((m) => m[1]);
for (const key of keys) {
  const base = key.replace(/\.(one|other)$/, "");
  const used =
    everything.includes(`"${key}"`) ||
    everything.includes(`'${key}'`) ||
    (base !== key && everything.includes(`"${base}"`)) ||
    prefixes.some((p) => key.startsWith(p));
  if (!used) problems.push(`unused catalog key: ${key}`);
}

// 2. Text typed straight into a template instead of coming from the catalog.
const LETTERS = /[A-Za-z]{2,}/;
for (const { path, text } of sources.filter((s) => s.path.endsWith(".svelte"))) {
  let markup = text.replace(/<script[\s\S]*?<\/script>/g, "").replace(/<style[\s\S]*?<\/style>/g, "").replace(/<!--[\s\S]*?-->/g, "");
  // Svelte expressions in braces can contain < and >, so take them out before looking for text between tags.
  for (let before = ""; before !== markup; ) {
    before = markup;
    markup = markup.replace(/\{[^{}]*\}/g, "");
  }
  for (const m of markup.matchAll(/>([^<>{}]+)</g)) {
    const literal = m[1].trim();
    if (LETTERS.test(literal)) problems.push(`${relative(root, path)}: hard-coded text "${literal}"`);
  }
  for (const m of markup.matchAll(/\s(aria-label|placeholder|title|alt)="([^"{}]+)"/g)) {
    if (LETTERS.test(m[2])) problems.push(`${relative(root, path)}: hard-coded ${m[1]} "${m[2]}"`);
  }
}

if (problems.length > 0) {
  console.error(problems.map((p) => `  ${p}`).join("\n"));
  console.error(`\n${problems.length} problem(s) with the string catalog`);
  process.exit(1);
}
console.log(`string catalog ok (${keys.length} keys, ${sources.length} files checked)`);
