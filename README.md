# bitemporal-runtime

Bitemporal truth primitives for append-supersede temporal data.

`bitemporal-runtime` is the only Rust implementation of strict
bitemporal truth — separate `valid_time` and `recorded_time`,
append-supersede, as-of queries. As of release, **zero analogues
exist on crates.io**.

## When to use it

Bitemporal data modeling is for domains where you need to answer
"what did we believe at time T about the fact that was valid at
time V" — audit, regulatory reporting, scientific reproducibility,
financial historical reconstruction, version-of-truth queries.

If you have one timeline (just `updated_at` or just `valid_from`),
you don't need this crate. If you have two, and you need both, you
do.

## Quick Start

```rust
use bitemporal_runtime::{append_supersede, as_of_query, BitemporalRecord};
use chrono::{TimeZone, Utc};

fn main() {
    // Build a fact at two points in time — first we believed one
    // thing, then a corrected version came in.
    let t0 = Utc.timestamp_opt(1000, 0).unwrap();
    let t1 = Utc.timestamp_opt(2000, 0).unwrap();
    let t2 = Utc.timestamp_opt(3000, 0).unwrap();

    let mut records: Vec<BitemporalRecord<&str>> = Vec::new();
    append_supersede(
        &mut records,
        BitemporalRecord {
            id: "us-presidents".to_string(),
            valid_time: t0,
            recorded_time: t0,
            value: "Roosevelt",
        },
    ).unwrap();
    let receipts = append_supersede(
        &mut records,
        BitemporalRecord {
            id: "us-presidents".to_string(),
            valid_time: t0,
            recorded_time: t1,
            value: "Truman",
        },
    ).unwrap();

    // Supersession receipts are the audit handle for the change.
    // SHA-256 digests bind every record field (id, temporal, value).
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].superseded.superseded_id, "us-presidents");
    assert_eq!(receipts[0].superseding_id, "us-presidents");

    // "As of T=1500, what did we believe was the value of
    // `us-presidents` at T=2000?" — answer: Truman, because that
    // was the latest recorded version by 1500.
    let as_of = as_of_query(&records, t1, Utc.timestamp_opt(1500, 0).unwrap());
    assert_eq!(as_of[0].value, "Roosevelt"); // only v1 was known by T=1500

    // "As of T=2500, what did we believe was valid at T=3000?" —
    // answer: the latest known version, v2 (Truman).
    let as_of = as_of_query(&records, t2, Utc.timestamp_opt(2500, 0).unwrap());
    assert_eq!(as_of[0].value, "Truman");
}
```

## Overview

This crate provides first-class support for bitemporal data modeling:

- **valid_time**: When a fact is true in the business domain
- **recorded_time**: When the system captured the fact
- **append-supersede**: Updates append new rows instead of mutating existing ones

## Key types

- `BitemporalRecord<T>` — a temporal record with `valid_time`, `recorded_time`, and domain value
- `SupersessionReceipt` — cryptographic SHA-256 receipt for supersession events
- `InMemoryDb` — in-memory store for testing
- `SqliteDb` (feature `sqlite`) — durable SQLite-backed store

## Core functions

- `append_supersede()` — append a new record, emit receipts for superseded prior versions
- `as_of_query()` — query records valid at a given `valid_time` as of a given `recorded_time`
- `temporal_snapshot()` — retrieve full state as of a given `recorded_time`

## Cargo features

- `sqlite` — enables the `SqliteDb` durable store (adds `rusqlite` dep)
- `schema` — enables JSON Schema generation for the public types (adds `schemars` dep)

Both features are off by default to keep the dep graph small.

## MSRV

Rust 1.75 (2021 edition).

## Dependencies

Default: `chrono`, `serde`, `serde_json`, `sha2`, `thiserror`.
With `sqlite`: adds `rusqlite`.
With `schema`: adds `schemars`.

## License

MIT OR Apache-2.0 (dual-licensed). See `LICENSE-MIT` and
`LICENSE-APACHE` for the full texts.

## Changelog

See `CHANGELOG.md` for the release history.
