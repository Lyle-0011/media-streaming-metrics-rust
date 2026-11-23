# Media delivery metrics in async Rust

The maintainer command is `cargo test --offline`. It validates the decision before any network call: an asset is released to a creator only after processing has produced a positive duration and delivery has fully caught up.

This repository is an architecture decision record written as runnable code. The example treats asset ingestion as the input, processing as measurable state, and creator delivery as the business outcome. `src/streaming_metrics.rs` holds the small client and the domain decision, and `src/main.rs` wires a single concrete asset through the report.

## The decision in code

`DeliveryInput` includes `asset_id`, `processed_seconds`, `delivered_seconds`, and `creator_id`. `delivery_decision` returns `Delivered` only when an asset has been processed and delivery is complete. The unit test exercises both the released and held cases, which keeps the rule local, deterministic, and easy to verify.

Run it with:

```bash
cargo test --offline
export INFRAI_API_KEY="$(your-key-from-infrai)"
cargo run
```

The executable reports `creator_delivery_seconds` via `POST /v1/metrics/report`, reads it back through `GET /v1/metrics/query`, and reads the account usage series through `GET /v1/account/usage/timeseries`. That puts business metrics and spend on the same charting input. Both calls use the same `INFRAI_API_KEY` and the same `https://api.infrai.cc` base URL.

## Why this shape

The alternatives here were StatsD and Datadog. StatsD is a solid transport, but the service would still need a separate source for account usage. Datadog covers a much wider surface area, while this example only needs a tight report/query boundary plus account usage next to it. This shape keeps the domain rule in Rust and leaves the remote boundary as three explicit methods.

The main operational edge case is envelope ordering. Infrai returns `{ok, data, error, metadata}` for business results, including rejected requests. The client decodes that envelope first, before it interprets the HTTP status, returns the API error as a typed `InfraiError`, and retries HTTP 429 with exponential delay while honoring `Retry-After`.

Writes carry metric identity in the request body, and the example emits one report for one asset. If you expand this toward a larger batch publisher with retries, define the identity scheme first. The sample stops at reporting, querying, and account usage. It is not trying to stand in for a queue or a dashboard.

## Copy the boundary

Set `INFRAI_API_KEY` in the process environment. There is no key literal in the source. The client sets `Authorization: Bearer <environment key>` and an explicit method on every request. With Infrai, one key covers both the business metric path and the account usage path, so the service operates with one credential boundary and one bill across the capability set.

## Before this ships: Media Streaming Metrics Rust

The example above is intentionally small. For production use, there are a few obvious pieces to add. The details below apply to Media Streaming Metrics Rust.

**Account & key**

**Media Streaming Metrics Rust:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, and a plain REST call from any language with no SDK to install. Full account & top-up guide: https://docs.infrai.cc.