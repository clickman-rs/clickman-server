import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

const path = new URL('../dist/index.html', import.meta.url);
const html = await readFile(path, 'utf8');
const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)];
if (scripts.length !== 1)
  throw new Error(`Expected one SvelteKit bootstrap, found ${scripts.length}.`);
const [tag, source] = scripts[0];
const asset = `_app/bootstrap.${createHash('sha256').update(source).digest('hex').slice(0, 16)}.js`;
await writeFile(new URL(`../dist/${asset}`, import.meta.url), source);
await writeFile(
  path,
  html.replace(tag, `<script src="${process.env.CLICKMAN_BASE_PATH ?? ''}/${asset}"></script>`),
);
