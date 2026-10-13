# Dashboard implementation and review

## Refactor and MCP — 14 October 2026

The user approved a review/refactor of all eight Clickman repositories, an MCP
entry point, bug fixes, UI improvements and local commits. Ferment remains a
session-authorized host and is left uncommitted. The primary agent reviewed the
changes separately, with independent frontend/SDK review from the SaaS agent.

The MCP executable lives in `apps/mcp`, with reusable typed tools in `crates/mcp`.
The official Rust MCP SDK owns negotiation, transport framing and tool schemas.
Eight read/preview tools and two explicitly enabled save tools call the existing
GraphQL host. Token or member-cookie/Origin authentication, current project
grants, response/input bounds and credential redaction are covered by real
transport, stdio and PostgreSQL host tests. See [MCP setup](MCP.md).

Core metadata CRUD now uses SeaORM entities. Cloud has twelve control-plane
entities with typed stored enums and relations; team/grant queries are batched.
Its six native PostgreSQL statements have specific queue, locking, atomic retry
or engine-JSON reasons, documented in the [Cloud backend guide](../../clickman-saas/docs/BACKEND.md).
The only Cloud migration adds three lookup indexes; populated up/down/up is
tested. Native ClickHouse analytics and compressed PostgreSQL operations retain
their specialized SQL. No generic repository layer or new account/analytics
schema replaces the existing domain boundaries.

The dashboard 0.2.2 separates funnel/segment editors, shared event-filter forms
and webhook management. It adds funnel editing, preserves literal event names,
alternative events, missing-event filters, paused status and fresh-draft defaults.
Tables have column spacing and an accessible scroll region on small screens.
The scoped CSS still excludes Preflight. The headless client can select report
keys in PostgreSQL before transferring report JSON; omitted keys retain the
existing all-report query. Cloud session refreshes publish only their newest
result. Browser tools are split from provider composition, refresh visible
collections after writes and acknowledge saved definitions even if refresh fails.
The landing separates fictional data from its Svelte host and renders guide
headings/contents, including the new MCP guide.

Material review findings fixed with regressions:

- ClickHouse report refresh re-acquired a one-connection metadata pool. Shared
  transaction-bound loaders remove that deadlock. Funnel reports and segment
  projections now order timestamp ties consistently; unrelated properties no
  longer change membership. Limiter retries cannot overflow or refill twice
  after out-of-order timestamps, and the client-bucket map is bounded.
- Extreme protocol clock corrections return a rejection instead of panicking.
  JSON strings produced by flattening obey the same Unicode character limit;
  canonical snapshots match in all native and browser clients and legacy.
- Cloud compound metadata mutations roll back completely on GraphQL failure;
  mixed native ClickHouse/metadata mutations are rejected before execution.
  New-project response permissions belong only to the returned node. Stripe
  Checkout retries keep identical idempotent parameters, and complete SSE frames
  are bounded as well as unfinished frames.
- Browser collection validates configuration, keeps source identity fixed,
  preserves old queue IDs during migration, reports expiry/capacity drops and
  rejects invalid Unicode before storage. Revoked consent, close and a later
  consent grant cannot send a previously retained queue snapshot. Native clients
  validate setup before storage, expire data during long retries and retain
  redirected batches. Swift safely rejects cyclic containers and preserves UInt64.
- Legacy clients mirror those fixes. The legacy Rust client acknowledges the
  final batch item instead of resending it forever; Rails validates trusted
  events through its model, excludes future funnel completions and resets its
  partition cache after rollback/retention.

| Final local check                         | Result                                                                                                                                       |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Core Rust, owned PostgreSQL + ClickHouse  | 75 passed, zero failed/ignored; strict Clippy and rustfmt passed                                                                             |
| MCP within those core tests               | 7 transport cases, 3 actual stdio cases and 1 real engine/authorization case                                                                 |
| Cloud Rust, owned PostgreSQL + ClickHouse | 12 passed, zero failed/ignored; strict Clippy and rustfmt passed                                                                             |
| Shared library / standalone units         | 5 + 4 passed; packed external consumer installed, typechecked, built and rendered without host Tailwind                                      |
| Standalone / embedded browsers            | 13 + 13 passed against actual Rust/PostgreSQL                                                                                                |
| Cloud browser tools / full browser flow   | 11 unit + 6 browser cases passed                                                                                                             |
| Landing                                   | 3 browser cases, Astro/build/format checks passed; desktop/mobile screenshots reviewed                                                       |
| Browser SDK 0.1.1                         | 70 unit + 2 Chromium cases; types/build/format passed                                                                                        |
| Native Rust / Kotlin / Swift              | 104 Rust; 113 JVM + 3 Android; 93 macOS + 94 iOS; schema checks passed                                                                       |
| Legacy                                    | 82 PostgreSQL + 79 SQLite Rails; 155 Rust; 93 macOS + 94 iOS; 113 JVM + 3 Android; 10 + 10 real native E2E cases; contract and Clippy passed |
| Cloud recovery and container              | API/worker restart, dump/restore, sessions, source keys, credential references and resumed reports passed; non-root Docker flow passed       |
| Core Docker                               | API/worker/dashboard/MCP image built; shipped MCP executable starts with `--help`                                                            |
| Ferment compatibility                     | 3 HTTP + 4 analytics transaction cases and 27 production browser cases; zero skipped/flaky/failed browser outcomes                           |

Frontend checks report zero errors/warnings. The named, focusable scroll region
follows [MDN's keyboard scrolling guidance](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/overflow#accessibility);
one explained Svelte warning exception preserves keyboard access. The final
repack changed only that explanation and produces identical client/server
compiled component code to the browser-tested version. Frozen installs and
archive integrity checks cover Cloud, landing and Ferment. Consumer vendor
READMEs record exact source commits and SHA-256 values.

Evidence is retained in `/tmp/clickman-core-mcp-platform-final-3.log`,
`/tmp/clickman-saas-post-created-role-final.log`,
`/tmp/clickman-refactor-ui-{check,browser,browser-embedded,package}-final-*.log`,
`/tmp/clickman-refactor-cloud-{unit,browser,recovery,container}-final-*.log`,
`/tmp/clickman-refactor-landing-*-final-*.log`, `/tmp/clickman-js-*-final.log`,
`/tmp/clickman-legacy-{gate,ios}-final.log` and the legacy `tmp/gates/` logs.
Ferment's exact logs and unchanged staff/transaction boundary are recorded in
its existing foundation review. An early custom runner omitted its required
8 MiB stack setting; the standard runner and corrected browser invocation passed.

This is local acceptance and repository distribution. No npm publication, push,
remote CI run or production deployment is claimed. Stripe, SMTP, inference and
delivery tests use owned local responses; live provider-account acceptance is
separate. Query/buffering reductions are structural changes, not a measured
throughput improvement.

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

## Platform implementation — approved scope

The next approved stage keeps Clickman an independent, embeddable open-source
engine. SaaS consumes its public Rust APIs and the shared Svelte workspace.
Organizations, authentication, subscriptions and the browser agent belong to
`clickman-saas`; collection belongs to `clickman-js`; marketing belongs to
`clickman-landing`. PostgreSQL remains the transactional control store, with
PostgreSQL and ClickHouse analytics adapters. Ferment keeps staff-session access.

The agent loop and tools execute in the browser. Fireworks runs inference; the
Rust host owns credentials, authenticated proxying, usage limits and ordinary API
authorization. Browser tools share the typed Clickman GraphQL client with the UI.

Implementation and acceptance checklist:

- [x] Analytics contract and PostgreSQL adapter, preserving host transactions.
- [x] ClickHouse adapter with durable handoff, replay, retention and erasure.
- [x] Common backend conformance tests and comparative load measurements.
- [x] SaaS registration, email verification/reset, sessions and organizations.
- [x] Projects, invitation acceptance, Owner/Admin/Viewer roles and isolation.
- [x] SaaS SvelteKit shell, shared dashboard and first-event onboarding.
- [x] Browser TypeScript collection SDK, durable queue and identity linking.
- [x] Segments, entry transitions, signed webhooks, retry/replay and Zapier hooks.
- [x] Free/Paid plan catalog, organization usage and Stripe subscription lifecycle.
- [x] Browser agent with tools, streaming, previews, cancellation and usage budget.
- [x] Prerendered landing, self-host/cloud paths, pricing and documentation.
- [x] CI, owned database/browser tests, deployment artifacts and recovery checks.
- [x] Final separate review and local end-to-end product acceptance.

Prices, event limits and Stripe Price IDs are configuration, not branches in the
billing implementation. Registry publication and pushes remain deferred by the
user. This checklist covers the implemented platform and local acceptance below;
it does not claim a live provider-account release or production deployment.

### Early core adapter verification

The primary agent reviewed the actual facade/metadata/adapter boundary separately
from implementation. PostgreSQL uses the supplied metadata transaction. ClickHouse
uses the durable PostgreSQL inbox and native ReplacingMergeTree rows; the logical
key includes the original receipt time so a new acceptance outside the dedup
window remains a new event. Rotation retries preserve raw inbox work, native
queries deduplicate replay, and source revocation is checked during acceptance.
Remote analytics explicitly rejects caller-transaction execution rather than
claiming a transaction across databases. Ferment's PostgreSQL path retains its
existing atomicity.

An opaque project namespace provides per-host slug uniqueness. A request-local
ProjectCreator capability lets an authenticated host create and attach projects
without granting global read/admin rights. Standalone creation keeps the empty
namespace and instance-root policy. The SQL migration is additive; rollback
refuses to discard populated namespaces.

37 Rust tests passed with zero failures/ignored outcomes using owned PostgreSQL
and ClickHouse 26.9. This includes all prior PostgreSQL scenarios, full report
parity, failed-export recovery, replay, late data, retention, erasure, revoked
source keys and host-creation transaction rollback. Workspace/all-target Clippy
with warnings denied passed. Logs: `/tmp/clickman-platform-core-full-1.log` and
`/tmp/clickman-platform-core-clippy-1.log`. The first native run stopped because a
new dependency required updating Cargo.lock; it did not pass. SaaS, collection,
segments, billing, agent and landing were still pending at that point. The final
acceptance below supersedes that interim status.

### Platform acceptance — 14 October 2026

The primary agent completed a separate review of the actual public entries,
storage/adapters, host policies, worker ownership, browser lifetimes, billing,
credentials and distribution. The component boundaries and operator contracts
are documented in [PLATFORM.md](PLATFORM.md); Cloud, browser SDK and landing
have their own setup/readme files. No unresolved implementation finding remains
in the agreed scope. This is local acceptance with owned provider fixtures.

Review findings were fixed and verified:

- Identity-only actors created ghost retention cohorts. PostgreSQL and native
  ClickHouse regressions failed first, then passed after adding observed-event
  markers and canonical first-observation propagation.
- Editing a paused segment reenabled it. The browser regression now checks that
  the saved segment remains paused and its scoped Zapier configuration works.
- Parallel agent writes overwrote one pending review. Real SDK unit scenarios
  now verify serialized approval/decline, permission rechecks and cancellation.
- Selected-project revocation retained the old cache/draft when the role stayed
  Admin. Membership now includes sorted grants; the browser disposes the scope
  and clears the revoked route. The test failed before the fix and passes after.
- SMTP failures lacked a persistent retry deadline. The real Apalis/SMTP test
  checks failure, deadline, process restart and successful resumed delivery.
- Empty migration rollback was unconditionally refused. Repeated empty down/up
  works; populated identity/segment/namespace data is protected by guarded locks.
- SPA direct links returned HTML with 404. Owned HTTP tests cover 200 navigation
  and preserved API rejection; Ferment also retains JSON 404 for unknown API paths.
- The non-root Docker image needed a writable development-mail directory. The
  production image smoke checks delivery, sessions, reports and restart.

| Check                                                   | Actual result / evidence                                                                                                                                                                                                       |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Core Rust, PostgreSQL + native ClickHouse 26.9          | 52 passed, zero failed/ignored; `/tmp/clickman-core-platform-final-2.log`                                                                                                                                                      |
| Core formatting / strict all-target Clippy              | Passed; `/tmp/clickman-core-{rustfmt,clippy}-final*.log`                                                                                                                                                                       |
| Shared client / standalone units                        | 5 client + 4 transport cases passed; `/tmp/clickman-core-{client,web}-unit-final.log`                                                                                                                                          |
| Svelte/types, formatting, packed external consumer      | Passed; `/tmp/clickman-core-{web-check,format,package}-final.log`                                                                                                                                                              |
| Standalone and `/analytics` browser mounts              | 10 cases each passed; `/tmp/clickman-core-browser-platform-3.log`, `/tmp/clickman-core-prefix-final.log`                                                                                                                       |
| Cloud backend                                           | 8 passed, zero failed/ignored; `/tmp/clickman-saas-backend-final-2.log` includes native quota/inbox/report and concurrent empty bootstrap                                                                                      |
| Cloud agent SDK / browser / types / formatting / Clippy | 7 SDK unit + 4 browser cases passed; `/tmp/clickman-saas-{unit, browser, check, format, clippy}-final*.log`                                                                                                                    |
| Browser collection                                      | 42 unit + 2 real IndexedDB browser cases; `/tmp/clickman-js-tests-2.log`, `/tmp/clickman-js-browser-1.log`; real Rust ingestion/identify in Cloud browser suite                                                                |
| Cloud dump/restore and process restart                  | Passed with original cookie/source/ref and resumed worker reports; `/tmp/clickman-saas-recovery-final.log`                                                                                                                     |
| Docker release image / non-root API + worker            | Built and smoke passed; `/tmp/clickman-saas-docker-build-final-2.log`, `/tmp/clickman-saas-container-final-3.log`                                                                                                              |
| Ferment compatibility                                   | Rust compile and zero-error Svelte check; 3 owned HTTP/worker cases + 27 browser cases, zero skipped/flaky/failed; `/tmp/ferment-clickman-platform-extended-green-2.log`, `/tmp/ferment-clickman-platform-browser-final-2.log` |
| Landing                                                 | Astro zero diagnostics, 9 static routes and 2 browser cases; `/tmp/clickman-landing-{check,build,browser,format}-final.log`                                                                                                    |

The shared dashboard is packed as 0.2.1 and the browser SDK as 0.1.0. Cloud,
Ferment and the landing consume portable archives, with frozen-lock installs.
The landing uses Astro 5/Tailwind 4 and Cloudflare Pages Git builds, matching the
Ferment landing's stack/hosting model. Its demo uses the actual shared dashboard
with explicitly fictional readonly data. A Cloud URL and site URL are configured
at deployment; no live hosted service or invented paid price is advertised.

The owned 20,000-event/2,000-actor sample measured PostgreSQL
ingest/rotation/reports at 808/1229/195 ms and Docker ClickHouse at
1335/1651/336 ms. ClickHouse was slower in this sample; it establishes neither
a production capacity limit nor a speedup. See the benchmark command/example
and `/tmp/clickman-platform-backend-benchmark-20000.log`.

Stripe, Fireworks, SMTP and webhook failure/replay are covered with local
provider fixtures, including real client SDKs/transports. Live Stripe-account
Checkout/webhook/Portal, actual Fireworks model inference and production SMTP
credentials still require operator configuration and account-level acceptance.
Zapier support is Catch Hook delivery, without a marketplace OAuth application.
GitHub workflows are committed source and locally exercised checks; they have
not been dispatched for these changes. No npm publish, push or production
deployment is part of this acceptance. Ferment uses the staff-session mode and
remains uncommitted.
