import { mkdtemp, mkdir, readFile, writeFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { createRequire } from 'node:module';
import { once } from 'node:events';

const exec = promisify(execFile);
const root = fileURLToPath(new URL('../', import.meta.url));
const manifest = JSON.parse(await readFile(join(root, 'packages/dashboard/package.json'), 'utf8'));
const world = await mkdtemp(join(tmpdir(), 'clickman-package-consumer-'));
let server;
let browser;
try {
  await mkdir(join(world, 'packed'));
  await exec(
    'pnpm',
    ['--filter', manifest.name, 'pack', '--pack-destination', join(world, 'packed')],
    { cwd: root, timeout: 180_000 },
  );
  const archives = (await readdir(join(world, 'packed'))).filter((name) => name.endsWith('.tgz'));
  if (archives.length !== 1) throw new Error('Expected exactly one packed dashboard.');
  const archive = join(world, 'packed', archives[0]);
  const packedFiles = (await exec('tar', ['-tzf', archive], { timeout: 10_000 })).stdout;
  if (/\.test\.|node_modules|\.svelte-kit/.test(packedFiles))
    throw new Error('Package contains test or development files.');
  await mkdir(join(world, 'src'));
  await writeFile(
    join(world, 'package.json'),
    JSON.stringify(
      {
        name: 'clickman-package-consumer',
        private: true,
        type: 'module',
        packageManager: 'pnpm@10.29.2',
        dependencies: {
          [manifest.name]: `file:${archive}`,
          cachebay: '0.11.3',
          graphql: '^16.11.0',
          svelte: '^5.57.2',
        },
        devDependencies: {
          '@sveltejs/vite-plugin-svelte': '^5.1.1',
          vite: '^6.4.4',
          typescript: '^5.9.2',
          'svelte-check': '^4.7.6',
        },
      },
      null,
      2,
    ),
  );
  await writeFile(join(world, 'pnpm-workspace.yaml'), 'onlyBuiltDependencies:\n  - esbuild\n');
  await writeFile(
    join(world, 'index.html'),
    '<html lang="en"><head><meta charset="UTF-8"></head><body><div id="app"></div><script type="module" src="/src/main.ts"></script></body></html>',
  );
  await writeFile(
    join(world, 'vite.config.ts'),
    "import { defineConfig } from 'vite'; import { svelte } from '@sveltejs/vite-plugin-svelte'; export default defineConfig({ plugins: [svelte()] });\n",
  );
  await writeFile(
    join(world, 'svelte.config.js'),
    "import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'; export default { preprocess: vitePreprocess() };\n",
  );
  await writeFile(
    join(world, 'tsconfig.json'),
    JSON.stringify(
      {
        compilerOptions: {
          target: 'ES2022',
          module: 'ESNext',
          moduleResolution: 'Bundler',
          strict: true,
          skipLibCheck: true,
          allowJs: true,
          lib: ['ES2022', 'DOM'],
        },
        include: ['src/**/*.ts', 'src/**/*.svelte'],
      },
      null,
      2,
    ),
  );
  await writeFile(
    join(world, 'src/main.ts'),
    "import { mount } from 'svelte'; import App from './App.svelte'; mount(App, { target: document.getElementById('app')! });\n",
  );
  await writeFile(
    join(world, 'src/App.svelte'),
    `<script lang="ts">
import { ClickmanWorkspace, type ClickmanRoute } from '${manifest.name}';
import '${manifest.name}/styles.css';
import { createCachebay } from 'cachebay';
import { onDestroy } from 'svelte';
let route = $state<ClickmanRoute>({ projectId: null, section: 'overview' });
const client = createCachebay({ cachePolicy: 'cache-and-network', suspensionTimeout: 0, transport: { http: async () => ({ data: { __typename: 'Query', viewer: { __typename: 'Viewer', canCreateProjects: false }, projects: { __typename: 'ProjectConnection', edges: [], pageInfo: { __typename: 'PageInfo', startCursor: null, endCursor: null, hasPreviousPage: false, hasNextPage: false } } }, error: null }) } });
onDestroy(() => client.dispose());
</script>
<h1 id="host-heading">Host heading</h1><button id="host-button">Host action</button>
<ClickmanWorkspace {client} {route} onnavigate={(next) => { route = next; }} />
`,
  );
  const options = { cwd: world, timeout: 180_000, maxBuffer: 4 * 1024 * 1024 };
  await exec('pnpm', ['install', '--prod=false'], options);
  await exec('pnpm', ['exec', 'svelte-check', '--tsconfig', './tsconfig.json'], options);
  await exec('pnpm', ['exec', 'vite', 'build'], options);
  const port = Number(
    (
      await exec(
        'python3',
        [
          '-c',
          'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])',
        ],
        { timeout: 10_000 },
      )
    ).stdout,
  );
  server = spawn(
    process.execPath,
    [
      resolve(world, 'node_modules/vite/bin/vite.js'),
      'preview',
      '--host',
      '127.0.0.1',
      '--port',
      String(port),
      '--strictPort',
    ],
    { cwd: world, stdio: 'ignore' },
  );
  let spawnError;
  server.on('error', (error) => {
    spawnError = error;
  });
  const url = `http://127.0.0.1:${port}`;
  const deadline = Date.now() + 15_000;
  while (true) {
    if (spawnError) throw spawnError;
    if (server.exitCode != null) throw new Error('Consumer preview server exited.');
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(1000) });
      if (response.ok) break;
    } catch (cause) {
      if (Date.now() >= deadline) throw new Error('Consumer preview did not start.', { cause });
    }
    if (Date.now() >= deadline) throw new Error('Consumer preview did not become ready.');
    await new Promise((done) => setTimeout(done, 100));
  }
  const require = createRequire(join(root, 'apps/web/package.json'));
  const { chromium } = require('@playwright/test');
  browser = await chromium.launch();
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(url);
  await page.getByText('No accessible projects.', { exact: true }).waitFor({ timeout: 10_000 });
  const margin = await page
    .locator('#host-heading')
    .evaluate((element) => parseFloat(getComputedStyle(element).marginTop));
  if (!(margin > 0)) throw new Error('Dashboard CSS reset the host heading margin.');
  if (errors.length) throw new Error(errors.join('\n'));
  console.log(
    'Packed dashboard installed, typechecked, built and rendered without host Tailwind; host CSS preserved.',
  );
} finally {
  if (browser) await browser.close();
  if (server && server.exitCode == null && !server.killed) {
    const exited = once(server, 'exit');
    server.kill('SIGTERM');
    let timer;
    await Promise.race([
      exited,
      new Promise((done) => {
        timer = setTimeout(() => {
          server.kill('SIGKILL');
          done();
        }, 3000);
      }),
    ]);
    clearTimeout(timer);
  }
  await rm(world, { recursive: true, force: true });
}
