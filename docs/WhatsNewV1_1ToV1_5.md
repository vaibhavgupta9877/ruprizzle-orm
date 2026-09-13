# What's new in v1.1 – v1.5

This page covers the feature line developed on `dev-v2-x` after `1.0.0`.

> **Release status.** All of it shipped as **`1.5.0`** (tag `v1.5.0`). The workspace
> never moved off `1.0.0` while these five milestones landed, so the intermediate
> numbers were never cut and one minor version carries the whole line.
>
> This line was assessed and initially **blocked**; the findings and what each of them
> became are in
> [`ProjectPlan/v2/ProductionReadinessV1_5.md`](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/ProjectPlan/v2/ProductionReadinessV1_5.md).

**Upgrading from `1.0.0-rc.1`?** Most applications need no code changes: queries,
the generated client and existing migrations keep working. A few lower-level types did
change shape (new enum variants, new struct fields, `#[non_exhaustive]` diagnostics).
[Upgrading from 1.0.0-rc.1](UpgradingFromRc1.md) lists each one with its fix.

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

## v1.5 — Ruprizzle Studio

`ruprizzle studio` (behind the non-default `studio` Cargo feature) starts an embedded
Axum + HTMX workbench on `127.0.0.1:5555` with no Node or npm dependency.

```
ruprizzle studio --allow-writes
```

| Screen | What it does |
|---|---|
| Dashboard and ERD | Render the parsed schema: models, fields and the relation graph |
| Table browser | `SELECT`s the model's columns, ordered by primary key, 50 rows a page; the filter box compiles to a parameterised `LIKE` across the scalar columns |
| Cell editor | `UPDATE … WHERE pk = ?`, then re-reads the stored value — what you see afterwards is what the database kept, not what you typed |
| Row insert / delete | Parameterised `INSERT` and `DELETE`. A delete that matched nothing returns `404`, not `200` |
| Relation drawer | `SELECT`s the record the foreign key points at; a dangling key says so |
| SQL sandbox | Runs your statement and shows the real columns, rows and affected count, or the database's own error message |
| `EXPLAIN` viewer | The plan the database returns for your statement. `EXPLAIN` only, never `ANALYZE`, so the statement is not executed. Costs and row estimates appear only where the plan carries them |
| Migration safety diff | Reads the live catalog and classifies each difference against the data: a dropped column counts its non-`NULL` rows and is `DESTRUCTIVE` when data would be lost; a `NOT NULL` addition to a populated table is `CAUTION` with the row count that will fail the migration |

When Studio cannot reach the database, every screen says so and renders nothing else.
That matters most on the diff: it shows a failure banner rather than a green badge, so
an unreachable database can never be mistaken for a safe migration.

Two limits worth knowing:

- **Studio has no authentication.** It binds `127.0.0.1` by default. `--allow-writes`
  on a non-loopback host is refused unless you pass `--yes-i-know`, because that
  combination publishes `INSERT`, `UPDATE` and `DELETE` to anyone who can reach the
  port.
- **The production check is a name check.** It matches `prod`, `aws.neon.tech` and
  `turso.io` in the URL. It will not catch an RDS endpoint or a bare IP. Treat it as a
  speed bump.

### `ruprizzle-turso` and `ruprizzle-d1` — adapters over their providers’ HTTP APIs

Both are real drivers, published, and tested end to end against a local HTTP server
so neither the tests nor a contributor need an account.

| Crate | Protocol | Endpoint |
|---|---|---|
| `ruprizzle-turso` | [Hrana 2 over HTTP](https://github.com/tursodatabase/libsql/blob/main/docs/HRANA_3_SPEC.md) | `POST {url}/v2/pipeline` |
| `ruprizzle-d1` | [Cloudflare REST API](https://developers.cloudflare.com/api/resources/d1/) | `POST /accounts/{account}/d1/database/{database}/query` |

```rust,no_run
let pool = TursoPool::builder()
    .url("libsql://your-db.turso.io")
    .auth_token(std::env::var("TURSO_AUTH_TOKEN")?)
    .build()?;
```

Both speak the SQLite dialect, bind parameters positionally with `?`, and store
booleans as `1` and `0`. Credentials are sent as bearer tokens and are redacted from
`Debug` output.

What they do not do, in both cases for the same reason — one HTTP request per
statement, with no server-side session:

- **No interactive transactions.** `BEGIN` and `COMMIT` sent separately would not
  share a connection. Batch the work into one statement, or use a SQLite file
  through `ruprizzle::connect`.
- **No streaming transport.** `stream_raw` is a streaming *interface*; the whole
  result set arrives in one response.

Turso also has no embedded-replica support — that needs the native libSQL library —
and D1 takes no binary parameter over HTTP, so a `Bytes` value is refused rather than
mangled. Inside a Cloudflare Worker you have a D1 binding, which is faster and needs
no token; `ruprizzle-d1` is for talking to D1 from outside one.

### There is no Neon adapter

`ruprizzle-neon` has been deleted. Neon speaks ordinary Postgres over TLS, so its
connection string goes straight to `ruprizzle::connect` and an adapter crate would
have been a wrapper around nothing. The stub was carrying a promise the provider
never needed.

The genuinely useful piece of this milestone is underneath: the runtime `Executor` trait
is now decoupled from `sqlx`, so a third-party backend can implement it directly.
