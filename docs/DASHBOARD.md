# Dashboard implementation and review

## Agreed change — 13 October 2026

The user approved one reusable Svelte dashboard package, a SvelteKit static
standalone application, Tailwind 4 styles isolated from the host, and importing
that package into Ferment administration. GraphQL execution stays in Cachebay.

### Current client → shared dashboard

| Current entry                                                            | Intended owner                                    |
| ------------------------------------------------------------------------ | ------------------------------------------------- |
| `apps/web/src/App.svelte`, token storage and application header          | Standalone SvelteKit auth/layout                  |
| `Dashboard.svelte`, activity, retention and event tables                 | Dashboard package                                 |
| `Funnels.svelte`                                                         | Dashboard package funnels screen                  |
| `Settings.svelte`, source/access keys, processing, destinations, erasure | Dashboard package settings screens                |
| Web GraphQL operations and generated documents                           | Dashboard package GraphQL                         |
| Private UI and Tailwind workspace dependencies                           | Internal dashboard components and packaged styles |
| Ferment's separate project list/create page                              | Thin host route rendering the shared package      |

The behavior reference is the current browser application and its GraphQL
documents. Existing project permissions, report JSON shapes, key workflows,
privacy erasure, SDK ingestion, database tables, migrations and API/worker
responsibilities remain their current contracts. No new server workflow is
required by this frontend change. Standalone authenticates with Bearer tokens;
Ferment uses its existing session-authorized analytics GraphQL endpoint.

The host owns URL navigation, credentials, Cachebay clients and their disposal.
The package owns its components, watchers and mounted-screen refresh timers.
Static output must work at the root and at a configured build-time base path.
The packaged dashboard must install without private workspace or sibling-source
dependencies and without requiring the consumer to configure Tailwind.

## Acceptance

The primary agent completed a separate necessity, ownership, correctness,
security, readability and error-handling review. All 15 previous GraphQL operation
names remain. The extra `DashboardAccess` query lets permissions arrive while a
project list is delayed; Analytics also selects existing slug/admin metadata to
identify the current route. No backend schema, data model, permissions or jobs
changed. The embedding example accepts an optional test bind address while
preserving its normal default.

Review findings fixed with browser evidence:

- Protected deep links now load absolute assets and preserve login destinations.
- UUIDs are canonicalized before comparing route and response identity.
- Cached/late responses cannot populate another project or session. Reader direct
  settings URLs render an explicit permission outcome without management forms.
- Mutation drafts remain locked through the write and follow-up query. Partial
  data/errors and missing data are rejected; failed refreshes stay visible.
- Root project auto-selection replaces history, so Back reaches the previous
  actual screen. This scenario failed before the navigation fix.
- Compiled CSS has prefixed utilities/scoped defaults without Preflight. Host
  colors/fonts are supplied through variables. Desktop/mobile screenshots were
  inspected and the project label/control defaults corrected.

Local acceptance evidence:

| Check                                           | Result                                                                                                                   |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Rust schema and package document generation     | Passed; no removed operations                                                                                            |
| Svelte/TypeScript checks                        | Both library and standalone: zero errors/warnings                                                                        |
| Formatting and Rust example rustfmt             | Passed                                                                                                                   |
| Unit tests                                      | 2 result/error cases and 4 real client-transport cases passed                                                            |
| Packed external consumer                        | Installed, typechecked, built and rendered without Tailwind; host heading CSS preserved, including `NODE_ENV=production` |
| Real standalone Rust API/PostgreSQL             | 9/9 browser scenarios passed                                                                                             |
| Real `/analytics/` embedding example/PostgreSQL | 9/9 browser scenarios passed                                                                                             |

Final logs are `/tmp/clickman-dashboard-{check,format-check,package-consumer,browser,prefix-browser}-final.log`;
unit logs end in `unit-2.log` and `library-unit-2.log`. Earlier failing regression
logs are retained separately. CI runs generation, formatting, types, units,
isolated package consumption and both browser mounts. The workflow has not been
run on GitHub for these changes.

Ferment's adapter and browser evidence are recorded in its existing
`docs/reviews/2026-10-11/foundation-review.md`: all 27 production browser cases
passed through Axum on disposable PostgreSQL. The host's frozen install and clean
manifest-only install with the package archive passed as well.

The user deferred npm publication and requested a local Clickman commit, with
Ferment left uncommitted. `@clickman/dashboard` is distributed as a portable
packed archive for the host; it has no private library source dependency. Registry
publication/verification and GitHub deployment are outside this acceptance.

Ferment's backend audit passed 8 relevant PostgreSQL/HTTP checks, including a
real persisted Maintenance job processed by its worker after SDK ingestion.
Its staff-session dashboard and SDK/worker/report pipeline are verified.
Standalone Bearer login is not configured in that host: `/analytics/graphql`
uses the default deny authenticator and rejects even valid project access keys.
Issuing/revoking keys is separate from enabling their authentication workflow.
No live Mixpanel delivery is claimed by these checks.
