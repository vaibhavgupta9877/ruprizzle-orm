# ruprizzle-turso

Turso / libSQL adapter for the [ruprizzle](https://github.com/vaibhavgupta9877/ruprizzle-orm) ORM.

`TursoPool` implements `ruprizzle::Executor` by sending each statement to a libSQL
server over the [Hrana 2 HTTP protocol](https://github.com/tursodatabase/libsql/blob/main/docs/HRANA_3_SPEC.md):
`POST {url}/v2/pipeline` with the SQL and its bound parameters, and one result set
back. It works against Turso's hosted databases and against any `sqld` you run
yourself.

```rust,no_run
use ruprizzle::Executor;
use ruprizzle::value::Value;
use ruprizzle_turso::TursoPool;

# async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
let pool = TursoPool::builder()
    .url("libsql://your-db.turso.io")
    .auth_token(std::env::var("TURSO_AUTH_TOKEN")?)
    .build()?;

let rows = pool
    .fetch_all_raw("SELECT id, email FROM users WHERE id = ?".into(), vec![Value::I64(7)])
    .await?;
# Ok(())
# }
```

`libsql://` and `wss://` URLs are rewritten to `https://`; `http://` is accepted for a
local `sqld` without TLS. The token is sent as a bearer credential and is redacted
from `Debug` output.

## What it does not do

- **No interactive transactions.** Every statement is one HTTP request with no
  server-side session, so `BEGIN` and `COMMIT` sent separately would land on unrelated
  connections. Batch the work into one statement, or use a SQLite file through
  `ruprizzle::connect` when you need multi-statement transactions.
- **No embedded replicas.** Keeping a local SQLite file in sync with a remote primary
  needs the native libSQL library, which this crate does not link. If you already have
  a replica file, point `ruprizzle::connect` at it as an ordinary SQLite database.
- **No streaming transport.** `stream_raw` is a streaming *interface*: the whole result
  set arrives in one response, so memory use is that of the full result.
- **No array parameters.** SQLite has no array type, so binding one is an error rather
  than a silent flattening.

## Dialect

SQLite. Placeholders are `?`, and booleans bind as `1` and `0`, which is how SQLite
stores them.

## Testing

`cargo test -p ruprizzle-turso` runs the protocol unit tests and an end-to-end suite
against a local HTTP server, which asserts the request the adapter builds — path,
bearer token, bound parameters, and the closing pipeline step — as well as how it
reads rows, affected-row counts, statement errors and transport failures. No network
access and no Turso account are needed.

## License

Licensed under either of MIT or Apache-2.0.
