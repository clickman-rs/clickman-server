# Projects, storage, and background processing

This guide describes project isolation, access keys, event storage, maintenance, and delivery to external destinations. Start with the [README](../README.md) to run the API and worker.

## Projects and access

Every event, dictionary, chunk, activity map, report, funnel, source key, access key and delivery entry belongs to a project. Composite foreign keys prevent cross-project dictionary references. Message IDs are deduplicated **within a project**, so two projects may independently receive the same message. Report and funnel GraphQL IDs include their project, keeping Cachebay identities separate. The projects query returns a Relay connection containing all accessible projects.

Standalone authentication has an instance admin token from the environment and database-backed project reader/admin tokens. Tokens are stored as SHA-256 digests, returned once, and can be revoked. Project admins administer their project; creating projects requires the instance token. Source write keys can only ingest events and cannot open GraphQL. An embedding application can supply its own user accounts and login provider. The standalone service uses access tokens and does not require those integrations.

SDKs use the unchanged [v1 batch protocol](PROTOCOL.md). The server derives the project from the write key; callers cannot choose another project in a batch. The independent repositories are `clickman-swift`, `clickman-kotlin` and `clickman-rust`; each contains its own contract fixtures and durable queue implementation. An SDK endpoint may include the engine mount, such as `https://example.com/analytics`.

## Storage and jobs

Events rotate into up to 4096-event chunks: delta-encoded UTC times, actor IDs and property columns, encoded as MessagePack and Zstandard. Changing property keys and large values produce smaller chunks to bound the column matrix before allocation. PostgreSQL stores the payload externally without recompressing it. Activity uses 366-bit annual maps. Reports include DAU/WAU/MAU, exact D1/D7/D30 cohort retention, distinct users per event, ordered funnels, median transition times and property breakdowns. Anonymous events contribute event counts and do not contribute identified-user metrics.

Rotation processes unrotated rows and marks them atomically with the chunk and activity updates, rather than advancing a receive-time cursor. This includes late commits and supports retry after failure. Project row locks serialize rotations, erasure, report refresh and delivery. Ingest takes a shared project lock, so erasure cannot race an unfinished ingest transaction.

Destinations have a durable outbox. A failed page keeps raw events and records a persistent retry deadline. Delivery is at least once: a network success followed by process death can resend the page. The Mixpanel adapter supplies stable `$insert_id` values. Credentials are environment references scoped to a project: `CLICKMAN_<PROJECT_UUID_WITHOUT_HYPHENS_UPPERCASE>_MIXPANEL_USERNAME` and `CLICKMAN_<PROJECT_UUID_WITHOUT_HYPHENS_UPPERCASE>_MIXPANEL_SECRET`. The UI supplies these names. Set them on the worker (or embedded host); project admins cannot reference other projects' or the host's secrets. Custom embedded destinations implement `clickman_destinations::Dispatcher`. Adding a destination backfills retained raw events. Removing it releases its retention hold. The cron tick is ephemeral; all unfinished work and retry state are in PostgreSQL and are recovered on startup.

Retention deletes complete months per project. Erasure deletes raw events and pending delivery entries, rewrites affected chunks, clears activity and leaves an actor tombstone. Cached reports are invalidated; they are recomputed on the next cycle. Future events for that external ID create a new actor. Already-forwarded data must be erased at its destination.
