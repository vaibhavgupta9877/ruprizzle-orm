# What's new in 1.6

`1.6.0` (2026-10-03) is additive over `1.5.1`, with one behaviour change in Ruprizzle
Studio. It also includes every fix from `1.5.2`, which was prepared but never
published. The full list is in the [changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md).

```toml
[dependencies]
ruprizzle = "1.6"
```

```bash
cargo install ruprizzle-cli --features studio   # --features studio only if you want Studio
```

## Studio requires its session token

Every Studio request, reads included, now needs the per-run session token.

```bash
ruprizzle studio
# ⚡ Ruprizzle Studio running at: http://127.0.0.1:5555/studio?token=3f9c…
```

- **Browser:** open the printed URL. Studio stores the token in an `HttpOnly`,
  `SameSite=Strict` cookie and redirects to `/studio`; later visits in that browser
  work without the query string until Studio restarts.
- **Scripts:** send the token as `x-studio-token: <token>` or
  `Authorization: Bearer <token>`.
- **A fixed token:** `ruprizzle studio --token <value>` or set
  `RUPRIZZLE_STUDIO_TOKEN`. Otherwise a new random token is generated each run.
- Writes (`--allow-writes`) additionally need a same-origin `Origin` header and the
  token in the `x-studio-token` header, which the Studio page sends for you; the
  cookie alone cannot perform a write.

**What changes for you:** a bookmark to a bare `/studio` URL now returns `401`. Open
the printed URL instead. The token travels over plain HTTP and there are no user
accounts, so keep Studio on loopback.

## `with_deleted()` on tree queries

Hierarchy queries skip soft-deleted nodes and everything reached only through them.
`with_deleted()` includes them:

```rust
let everything = db
    .category()
    .descendants(root_id)
    .with_deleted()
    .all()
    .await?;
```

It has no effect on models without a `@deletedAt` column.

## Background replica health checks

Replica health used to change only when you called `check_health()`. You can now run it
on a timer:

```rust
use std::time::Duration;

let routed = /* RoutedPool::builder(...)...build() */;
let health = routed.spawn_health_checks(Duration::from_secs(10));
// ... health checks run every 10 s while `health` is alive ...
drop(health); // or health.stop()
```

It must be called inside a Tokio runtime. Keep the handle alive: dropping it stops the
checks.

## `@renamedFrom` onto an existing column name

When a `@renamedFrom` target name is still used by an existing column, the migration
now drops that column before the rename, instead of producing a `RENAME` that fails at
apply time. The drop needs `--accept-data-loss`. See
[Migrations](MigrationsGuide.md#rename-detection).

## Under the hood

- Studio's templates use `askama` 0.14 (newer releases need Rust 1.88; the MSRV stays
  1.85).
- CI runs the integration and Studio suites on MariaDB 11.4 as well as MySQL 8.4, and
  Studio's MySQL read-only path has its own test.
- `ruprizzle-turso` and `ruprizzle-d1` have opt-in live smoke tests
  (`cargo test -p ruprizzle-turso --test live`), which run when the provider
  credentials are set. They have not yet been run against the hosted services.
