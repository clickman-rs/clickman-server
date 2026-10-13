# Clickman platform

## Component boundaries

| Component          | Responsibility                                                                                                                               |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `clickman-server`  | Open-source Rust engine, analytics adapters, GraphQL, ingestion, segments, webhook delivery, shared Svelte dashboard                         |
| `clickman-saas`    | Rust account/API/worker host, organizations/projects, roles, Stripe, usage budgets, encrypted credentials, SvelteKit shell and browser agent |
| `clickman-js`      | Consent-controlled browser collection, IndexedDB queue, identity linking and explicit page/navigation tracking                               |
| `clickman-landing` | Astro/Tailwind static marketing/docs/demo; Cloudflare Pages Git hosting                                                                      |
| Ferment            | Existing staff-session host of the same engine/dashboard; no Cloud billing or agent dependency                                               |

The engine owns no SaaS accounts or subscription tables. The browser dashboard
and the Cloud agent use the same typed GraphQL documents and Cachebay client.
The agent SDK loop runs in the browser; the Rust SaaS host authenticates and
budgets Fireworks inference without running an agent loop or storing prompts.

## Analytics adapters

`Storage` is a facade over PostgreSQL `MetadataStore` and an
`Arc<dyn AnalyticsAdapter>`. Metadata owns projects, namespaces, settings, source
and access keys, actors/identity links, funnels, cached reports, segments and
durable deliveries. The adapter owns physical analytics storage and implements
acceptance, rotation, reports, observations, retention, erasure and health.

`Storage::connect` keeps PostgreSQL as the default. Embedded hosts can call
`with_analytics_adapter(...)`; standalone binaries use
`clickman_storage::adapters::from_environment()`:

```sh
CLICKMAN_ANALYTICS_BACKEND=clickhouse
CLICKMAN_CLICKHOUSE_URL=https://clickhouse.example.com:8443
CLICKMAN_CLICKHOUSE_DATABASE=clickman
CLICKMAN_CLICKHOUSE_USER=clickman
CLICKMAN_CLICKHOUSE_PASSWORD=...
```

API and worker must use the same backend/configuration. PostgreSQL remains
required. The ClickHouse operator needs permission to create the configured
database/table and perform inserts, reads and deletion mutations. Changing an
existing project's backend does not migrate historical events automatically.

PostgreSQL rotation commits compressed chunks, activity and inbox acknowledgement
in one transaction. The ClickHouse adapter first durably accepts the batch into
the PostgreSQL inbox, then writes native `ReplacingMergeTree` rows, then
acknowledges rotation in PostgreSQL. Replay after network acknowledgement loss
is deduplicated with `FINAL`. Its key includes project, message ID and original
receipt time, preserving a fresh acceptance after the raw dedup window. Native
queries have deadlines; erasure/retention wait for synchronous mutations.

An external database cannot participate atomically in a caller's PostgreSQL
transaction. The remote adapter rejects transaction-bound remote mutation/report
operations; use PostgreSQL when host transaction atomicity is required.
Metadata-only operations and scoped observation reads still use host access.
See [embedding](EMBEDDING.md) for request storage/commit ownership.

A new adapter implements `AnalyticsAdapter` and must run the existing
conformance scenarios in `crates/engine/tests/adapters.rs`: report parity,
replay/restart, late commits, revocation, identity merge, erasure and retention.
Segment observations project only the event names and predicate fields needed
by saved rules. Shared Rust analytics defines funnel/segment semantics rather
than duplicating those rules in database-specific SQL.

## Namespaces and host policy

Project UUIDs remain globally unique. `(namespace, slug)` is unique; standalone
uses the empty namespace, while Cloud assigns `saas:<organization ID>`. A
request-local `ProjectCreator` capability creates and attaches an authorized
project inside the host transaction without granting instance-root access.

`ProjectPolicy` restricts access-key issuance and retention for session-only
hosts. `IntegrationPolicy` restricts external credential references. Defaults
preserve standalone behavior. Cloud disables project access-key login and
enforces its plan retention. Ferment retains its staff-session endpoint; raw
`/analytics/graphql` intentionally keeps the default deny authenticator.

## Identities and privacy

The v1 batch format is unchanged. Native `*` remains an uncounted actor;
browser `anon:<UUID>` is a distinct pseudonymous visitor. `/v1/identify` links
that visitor to one opaque account ID within its project. Repeated links are
safe, cross-account rebinds are refused, and reports/segments use the canonical
actor. Identity-only placeholders do not create cohort membership.

Erasing an account erases its linked visitor family, analytics data and queued
segment/webhook payloads. Already delivered data needs erasure at its recipient.
Source keys are public ingestion credentials; allowed origins are an additional
browser restriction rather than a private-key authentication mechanism.

## Segments and funnel stages

Saved segment definitions have three forms:

```json
{
  "key": "buyers",
  "name": "Buyers",
  "rule": { "kind": "event", "filter": { "event": "purchase" }, "range": "30d" }
}
```

```json
{
  "key": "checkout_dropoff",
  "name": "Checkout drop-off",
  "rule": {
    "kind": "funnel",
    "funnel_key": "checkout",
    "at_step": 2,
    "exclude_later": true,
    "wait": "24h"
  }
}
```

```json
{
  "key": "no_purchase",
  "name": "No purchase after view",
  "rule": {
    "kind": "inactive_after",
    "entry": { "event": "view" },
    "absent": { "event": "purchase" },
    "after": "24h",
    "range": "30d"
  }
}
```

Filters share funnel event/property validation; `at_step` is 1-based. Funnel
membership uses that funnel's range/window and first qualifying path. Timed
conditions are reevaluated even without a new batch. Events at the same
millisecond use deterministic order: timestamp, matching step position, event
name, then serialized fields. A timestamp tie cannot establish the original
application order; supply distinct event times when that distinction matters.

The worker compares current membership with durable state. A transition from
absent to present creates one `segment.entered` entry, with a stable ID and
ordinal; a later exit/reentry creates a new ordinal. Renaming preserves state.
Rule/enabled changes create a revision and cancel pending old-rule deliveries.
Historical members are silent by default; `fireExisting` explicitly enables
backfill. Referenced funnel steps cannot be removed without updating segments.

## Signed webhooks and Zapier

A segment can have `webhook` or `zapier` destinations. Zapier uses its Catch Hook
URL with the same JSON payload/delivery contract; no Zapier marketplace OAuth
application is claimed. Standalone config uses either a literal URL or an
endpoint credential reference, plus a signing reference scoped to the project:

```json
{
  "url": "https://receiver.example.com/clickman",
  "signing_ref": "CLICKMAN_<PROJECT_UUID_WITHOUT_HYPHENS_UPPERCASE>_WEBHOOK_SIGNING"
}
```

Set the referenced environment secret on the worker; use at least 16 bytes.
Cloud encrypts both endpoint and signing key and stores only vault references
in engine configuration. An embedded host can supply `SecretResolver` to
`WebhookSender::with_secrets` and the Mixpanel dispatcher.

Delivery includes `X-Clickman-Event-Id` and
`X-Clickman-Signature: t=<Unix seconds>,v1=<hex HMAC-SHA256>`. Verify HMAC over
the exact bytes `<timestamp>.<raw JSON body>`, compare in constant time and
reject stale timestamps. Deduplicate by the JSON entry ID/header; retries and
manual replay retain the same entry ID and generate a fresh signature time.
Payload contains project, segment revision, opaque external ID, qualification
and entry times, and ordinal.

Any 2xx acknowledges delivery. Failures persist a 5-second exponential retry
deadline capped at one hour, with failure after 12 attempts. The dashboard shows
attempt/status logs and permits replay. Transport defaults to public HTTPS,
pins validated DNS addresses, rejects private/metadata networks, disables
redirects/proxies and bounds DNS/connect/exchange waits. Only an embedded
operator can opt into private networks using `allow_private_networks(true)`;
project configuration and Cloud never enable it.

## Operations and verification

Run API and worker as separate processes with shared configuration. Back up
PostgreSQL, the SaaS encryption key and (in remote mode) ClickHouse. Coordinate
database snapshots with ingestion/maintenance paused; restoring PostgreSQL
alone cannot restore native analytics that have left the inbox.

`scripts/test-platform --clickhouse` owns both test databases; browser runners
own their PostgreSQL clusters. The local 20,000-event/2,000-actor sample measured
PostgreSQL ingest/rotation/reports at 808/1229/195 ms and ClickHouse at
1335/1651/336 ms. PostgreSQL ran locally and ClickHouse in Docker on this Mac;
this sample does not establish production capacity or a ClickHouse speedup.
Load-test the intended event volume, segment windows and deployment hardware.

The recorded code/tests and provider-fixture boundaries are in
[platform acceptance](DASHBOARD.md). Registry publication, pushes and live
Stripe/Fireworks/SMTP account acceptance are separate from these local checks.
