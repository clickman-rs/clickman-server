# Clickman dashboard for Svelte

One analytics workspace for the standalone Clickman application and Svelte hosts.
Includes projects, activity, events, retention, funnels, segments, signed webhooks,
source/access keys, processing settings, destinations and privacy erasure. Server authorization
controls the available operations.

## Install and render

Install the dashboard together with its peers: Svelte 5, Cachebay 0.11.3 or later
within 0.11, and GraphQL 16. The package ships compiled CSS; the host does not
need Tailwind or any Clickman private workspace package.

The current distribution is a packed archive; npm publication is deferred.
Build/pack it using the commands below, copy it into the host repository, then
install it by relative path:

```sh
pnpm add ./vendor/clickman-dashboard-0.2.2.tgz
```

```svelte
<script lang="ts">
  import { ClickmanWorkspace, type ClickmanRoute } from '@clickman/dashboard';
  import '@clickman/dashboard/styles.css';
  import type { CachebayInstance } from 'cachebay';

  let { client }: { client: CachebayInstance } = $props();
  let route = $state<ClickmanRoute>({ projectId: null, section: 'overview' });
</script>

<ClickmanWorkspace
  {client}
  {route}
  onnavigate={(next) => {
    route = next;
  }}
/>
```

This example keeps navigation in component state. A routed host derives `route`
from its URL and implements `onnavigate` with its router. `projectId` is a
canonical UUID or `null`; a null selection opens the first accessible project.
Sections are `overview`, `retention`, `funnels`, `segments` and `settings`, exported as
`clickmanSections`. Creating a project navigates to its overview after refreshing
the project list. Reader access never renders management forms.

## Client ownership

The host creates an authenticated `CachebayInstance`, configures its HTTP
transport, and disposes it when that session ends. The workspace creates its own
query/mutation observers and cancels its refresh timers on unmount. Passing a
different client remounts the observers; the workspace never disposes a supplied
client. Abort the host's in-flight requests before disposing its client.

Render this workspace in the browser. In a SvelteKit SPA, set `ssr = false` in
the enclosing layout. The workspace does not import `$app`, manage login, store
credentials, or impose a URL structure. A separate optional client entry creates
the same Cachebay transport used by the standalone application.

The standalone host sends Bearer-authenticated requests to `/graphql`; an
embedded host can use its session endpoint, such as Ferment's
`/admin/api/analytics/graphql`. Return Cachebay operation results with GraphQL
errors preserved, including errors accompanying partial data. Do not put access
tokens into URLs. The existing Clickman GraphQL schema is required; no REST or
SDK ingestion adapter is needed for the workspace.

## Headless client and host policy

`@clickman/dashboard/client` has no Svelte dependency in its runtime imports.
It exports generated typed GraphQL documents and the same project-scoped API
used by the Cloud browser agent:

```ts
import { createClickmanClient, createClickmanApi } from '@clickman/dashboard/client';

const lifetime = new AbortController();
const client = createClickmanClient({
  endpoint: '/api/organizations/42/graphql',
  credentials: 'same-origin',
  lifetime: lifetime.signal,
});
const analytics = createClickmanApi(client, projectId, lifetime.signal);
const project = await analytics.project();
// Fetch only the report needed by a tool, or [] for project metadata.
const activity = await analytics.project({ reportKeys: ['actives'] });
// On user, organization, grant or project change:
lifetime.abort();
client.dispose();
```

The transport preserves GraphQL errors, uses a 30-second request deadline and
supports either a host session or a supplied Bearer token. The headless API
checks the returned project identity; all writes still require server access.

Hosts can pass `allowAccessKeys={false}` and `maxRetentionDays={30}` to the
workspace, with matching server `ProjectPolicy`. Optional `createWebhook` and
`createDestination` callbacks let a SaaS host encrypt submitted credentials and
save their project-scoped references atomically. Standalone mode instead uses
operator environment references. The browser agent is part of the SaaS host,
not a dependency of the shared workspace.

## Styles

Styles use prefixed `cm:` utilities and scoped `.clickman` defaults. The package
does not include Tailwind Preflight. Import `styles.css` once at the host layout.
Customize inherited variables on an enclosing element:

```css
.analytics {
  --clickman-color-brand: #3454d1;
  --clickman-color-brand-light: #edf0ff;
  --clickman-color-ink: #172324;
  --clickman-color-muted: #667778;
  --clickman-color-line: #dde5e3;
  --clickman-color-canvas: #f5f8f6;
  --clickman-font-sans: system-ui, sans-serif;
}
```

`ClickmanButton` is also exported for a host's login/header inside a `.clickman`
wrapper. Internal screen components are private. Typed GraphQL documents are
exported from `@clickman/dashboard/client`.

## Development and release

From the Clickman repository root:

```sh
pnpm generate
pnpm check
pnpm format:check
pnpm --filter @clickman/dashboard test
pnpm --filter @clickman/web test:unit
pnpm test:package
pnpm test:browser
pnpm test:browser --embedded
pnpm --filter @clickman/dashboard pack --pack-destination ./tmp
```

`test:package` installs the tarball in an isolated consumer, checks its types,
builds and renders it without Tailwind, and checks that host heading styles are
preserved. Browser suites use real Rust servers and disposable PostgreSQL
clusters. The current release workflow stops at a checked archive and repository
commit. An npm publication is a separate future step; local packing does not
publish the package.

The package layout and exports follow [Svelte packaging](https://svelte.dev/docs/kit/packaging).
See the repository's [embedding guide](https://github.com/lockvoid/clickman-server/blob/main/docs/EMBEDDING.md)
for standalone and host-owned integration options.
