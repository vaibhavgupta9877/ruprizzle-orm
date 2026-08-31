# What's new in v1.1 – v1.5

This page covers the feature line developed on `dev-v2-x` after `1.0.0`.

> **Release status.** None of this is on crates.io yet. The workspace version is still
> `1.0.0` and no tag has been cut. Everything in **v1.1 – v1.4** below is implemented and
> passes the full gate (`build`, `clippy -D warnings`, `doc -D warnings`, `fmt`, `deny`,
> `test --workspace`). **v1.5 is blocked** — see [v1.5](#v15--ruprizzle-studio-and-edge-adapters-blocked)
> at the bottom of this page and the assessment in
> [`ProjectPlan/v2/ProductionReadinessV1_5.md`](../ProjectPlan/v2/ProductionReadinessV1_5.md).

Every v1.1–v1.4 addition is **backwards compatible**. Nothing in `1.0.0`'s public API
changed shape, and no existing query needs rewriting.

---

## v1.1 — Query expressiveness, rich types and search

### Postgres array filters

Array columns get a dedicated filter set, compiled to native Postgres array operators
rather than to a subquery:

| Method | Meaning |
|---|---|
| `has(value)` | the array contains this one element |
| `has_every(values)` | the array contains **all** of these |
| `has_some(values)` | the array contains **any** of these |
| `contains(values)` | the array is a superset of these |
| `contained_by(values)` | the array is a subset of these |
| `overlaps(values)` | the arrays intersect |
| `is_empty()` / `is_not_empty()` | length is / is not zero |

```rust
let rust_posts = Post::find_many()
    .filter(Post::tags.has("rust"))
    .fetch_all(&pool)
    .await?;

let both = Post::find_many()
    .filter(Post::tags.has_every(["rust", "orm"]))
    .fetch_all(&pool)
    .await?;
```

As always, `.to_sql()` shows you exactly what will be sent.

### Full-text search

`matches()` on a text column compiles to each backend's native full-text construct:

| Dialect | Compiles to |
|---|---|
| Postgres | `to_tsvector('english', col) @@ plainto_tsquery('english', $1)` |
| MySQL | `MATCH(col) AGAINST (?)` |
| SQLite | `col MATCH ?`, with a `LIKE` fallback when no FTS table is in play |

```rust
let hits = Post::find_many()
    .filter(Post::body.matches("database migrations"))
    .fetch_all(&pool)
    .await?;
```

### Soft deletes

Mark a nullable `DateTime` field with `@deletedAt` and the model becomes soft-deletable:

```prisma
model Post {
  id        Uuid      @id @default(uuid7())
  title     String
  deletedAt DateTime? @deletedAt @map("deleted_at")

  @@map("posts")
}
```

`@deletedAt` requires a `DateTime?` field; the parser rejects anything else with a
diagnostic. Once it is present, every generated query filters
`WHERE deleted_at IS NULL` by default, and two builder methods opt out:

```rust
Post::find_many().fetch_all(&pool).await?;                  // live rows only
Post::find_many().with_deleted().fetch_all(&pool).await?;   // live + deleted
Post::find_many().only_deleted().fetch_all(&pool).await?;   // the recycle bin
```

---

## v1.2 — Developer tooling and zero-database CI

### `ruprizzle check` — offline query validation

`ruprizzle-check` validates SQL against your schema **without a live database**, so it
runs in CI with no service container. It reports:

- `UnknownTable` — with a did-you-mean suggestion when the name is close
- `UnknownColumn` — scoped to the table it was expected on, also with a suggestion
- `TypeMismatch` — a bind parameter whose type cannot match the column

Each carries a source location, so failures point at the line that wrote the query
rather than at the SQL string.

### LSP 2.0

`ruprizzle-lsp` gains broader completion and hover across schema attributes. The VS
Code extension lives in `editor/vscode`.

> ⚠️ The LSP currently offers a `@@tenant` completion and hover text for it, but the
> attribute is **not implemented** — the parser rejects it. Row-level security and
> multi-tenancy are not available. Ignore that suggestion.

### Declarative seeding

`ruprizzle seed` applies a JSON document in a single transaction. The top-level object
maps model or table names to arrays of rows; every row must carry the model's primary
key, and existing rows are **updated on primary-key conflict**, so re-running a seed is
safe and idempotent.

```json
{
  "User": [
    { "id": "0192...", "email": "ada@example.com", "name": "Ada" }
  ],
  "Post": [
    { "id": "0192...", "title": "Hello", "authorId": "0192..." }
  ]
}
```

---

## v1.3 — Relations, trees and nested writes

### Implicit many-to-many

Join tables for many-to-many relations are inferred from the schema instead of being
declared by hand, matching Prisma's implicit-m2m behaviour. The link operations are
modelled by `M2mAction` / `M2mWrite`.

### Nested relational writes

Related records can be created and linked inside a single mutation, via `NestedCreate`,
`NestedConnectOrCreate` and the `RelNestedOp` set — `create`, `connect`,
`connectOrCreate`, `set` and `disconnect`. `NestedCreate::set_if` takes an `Option`, so
optional fields do not need a branch at the call site.

### Tree hierarchies

Self-referencing models get recursive-CTE traversal through `HierarchyQuery`:

```rust
let subtree = Category::hierarchy()
    .descendants(root_id, /* max_depth */ Some(5))
    .fetch(&pool)
    .await?;

subtree.count();               // nodes in the result
subtree.max_subtree_depth();   // deepest level reached
subtree.flatten();             // depth-first Vec<&Category>
```

`ancestors()` walks the other direction, and `HierarchyQuery::to_sql()` prints the CTE
like every other builder.

---

## v1.4 — Observability, caching and scaled routing

### OpenTelemetry and Metrics 2.0

Spans follow the OpenTelemetry database-client semantic conventions, so traces land in
the right place in any OTel-aware backend without per-field mapping. Prometheus metrics
remain behind the existing `metrics` Cargo feature.

### Primary / read-replica routing

`RoutedPool` sends `SELECT` traffic to replicas and keeps writes and transactions on the
primary:

```rust
let pool = RoutedPool::builder(primary)
    .add_replica(replica_a)
    .add_replica(replica_b)
    .load_balancing(LoadBalancing::LeastConnections)
    .build();
```

Three strategies are available — `RoundRobin`, `LeastConnections` and `Random`.
Unhealthy replicas are skipped, and when no replica is healthy the router falls back to
the primary rather than failing. `begin()` always opens the transaction on the primary,
so read-your-writes inside a transaction is never at risk.

### Query result cache

A `QueryCache` trait with an `InMemoryCache` implementation:

```rust
let cache = InMemoryCache::new(10_000);

cache.set("users:active", bytes, Some(Duration::from_secs(60)), &["users"]);
let hit = cache.get("users:active");

cache.invalidate_tag("users");   // drop everything tagged `users`
cache.prune_expired();           // reclaim expired entries
```

Entries carry a TTL and a set of tags; `invalidate_tag` / `invalidate_tags` drop every
entry sharing a tag, which is the practical way to expire a table's worth of cached
reads after a write. The trait exists so a Redis or other shared backend can be dropped
in without touching call sites.

### PostGIS geospatial types

`Point`, `LineString`, `Polygon` and `MultiPolygon`, each with `to_wkt()` (and
`to_ewkt()` on `Point`), plus spatial filters and orderings on geometry columns:

```rust
let nearby = Store::find_many()
    .filter(Store::location.within_radius(&here, 5_000.0))   // metres
    .order_by(Store::location.distance_asc(&here))
    .fetch_all(&pool)
    .await?;
```

Also available: `intersects`, `within_polygon`, `contains`, `contains_point` and
`distance_desc`.

---

## v1.5 — Ruprizzle Studio and edge adapters (BLOCKED)

**This milestone is not usable and is not being released.** It is described here so the
state is not a surprise.

### Ruprizzle Studio

`ruprizzle studio` (behind the non-default `studio` Cargo feature) starts an embedded
Axum + HTMX workbench on `127.0.0.1:5555` with no Node or npm dependency.

**What works:** the dashboard, the model navigation and the interactive ERD. All three
read the parsed schema, which is real data.

**What does not work — do not rely on any of it:**

| Screen | Actual behaviour |
|---|---|
| Table browser | Five hardcoded rows; every cell is the literal string `Sample <field>` |
| Cell editor | Echoes your input back into the page. **No `UPDATE` is issued.** |
| Row delete | Returns `200 OK`. **Nothing is deleted.** |
| SQL sandbox | Never executes your SQL. Always renders "✓ Query Executed Successfully" and a fake one-row result. |
| EXPLAIN tree | A hardcoded three-node Postgres plan with invented costs. Your query is not read. |
| Migration safety diff | Reports **`SAFE` for every model, unconditionally**, without opening a database connection. |

Studio holds a connection pool and never reads it. The migration diff is the dangerous
one: it is consulted before running a destructive migration and it cannot report
anything but safe.

### `ruprizzle-turso`, `ruprizzle-d1`, `ruprizzle-neon`

**These are not functional database adapters.** None of the three declares a driver
dependency — there is no `libsql`, no Cloudflare `worker`, no HTTP client. All three
store rows in a process-local `HashMap` under invented column names and discard them
when the process exits. `TursoPool::sync()` returns hardcoded zeros without contacting a
primary, and the `auth_token` you pass is stored and never sent anywhere.

They are absent from the release pipeline and will not be published in this state. To
run against Turso, D1 or Neon today, use the standard Postgres or SQLite connection
paths with those providers' wire-compatible endpoints.

The genuinely useful piece of this milestone is underneath: the runtime `Executor` trait
is now decoupled from `sqlx`, so a third-party backend can implement it directly. That
work is sound; it is the adapters built on top of it that are not.
