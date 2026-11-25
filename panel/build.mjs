// Builds panel/dist: index.html (CSS inlined) + app.js, both for Chrome 51 (stock Android 7 WebView).
import { build } from 'esbuild';
import { mkdirSync, readFileSync, writeFileSync, statSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const src = join(here, 'src');
const dist = join(here, 'dist');
const TARGET = 'chrome51';

mkdirSync(dist, { recursive: true });

await build({
  entryPoints: [join(src, 'main.js')],
  outfile: join(dist, 'app.js'),
  bundle: true,
  format: 'iife',
  target: TARGET,
  minify: true,
  charset: 'utf8',
  legalComments: 'none',
  logLevel: 'warning',
});

const css = await build({
  entryPoints: [join(src, 'style.css')],
  write: false,
  bundle: true,
  target: TARGET,
  minify: true,
  charset: 'utf8',
  logLevel: 'warning',
});
const cssText = css.outputFiles[0].text.trim();

const html = readFileSync(join(src, 'index.html'), 'utf8').replace('/*__CSS__*/', () => cssText);
writeFileSync(join(dist, 'index.html'), html);

let total = 0;
for (const f of readdirSync(dist)) {
  const size = statSync(join(dist, f)).size;
  total += size;
  console.log(`dist/${f}  ${(size / 1024).toFixed(1)} KB`);
}
console.log(`total ${(total / 1024).toFixed(1)} KB (target ${TARGET})`);
