# ClickMan protocol

This directory is the canonical version 1 wire and device-queue contract. The
[protocol guide](../docs/PROTOCOL.md) describes the HTTP API. `queue.sql` and
`queue-upgrade.sql` define portable device storage, and `fixtures/` covers
normalization, sanitization, batching, retries, identity, traits, and lifecycle
events.

The Swift, Kotlin, and Rust SDK repositories keep local snapshots of this
contract so they can build and test independently. Update the specification,
SQL, and fixtures together when changing the protocol, then run each client's
conformance suite before committing the corresponding snapshots.

Servers select the project from its source write key. Version 1 batch bodies
do not contain a project identifier.
