# ruprizzle-d1

Cloudflare D1 adapter for the [ruprizzle](https://github.com/vaibhavgupta9877/ruprizzle-orm) ORM.

`D1Pool` implements `ruprizzle::Executor` against a D1 database through the
[Cloudflare REST API](https://developers.cloudflare.com/api/resources/d1/): each
statement is one `POST /accounts/{account}/d1/database/{database}/query`,
authenticated with an API token.

```rust,no_run
use ruprizzle::Executor;
use ruprizzle::value::Value;
use ruprizzle_d1::D1Pool;

# async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
let pool = D1Pool::builder()
    .account_id(std::env::var("CLOUDFLARE_ACCOUNT_ID")?)
    .database_id(std::env::var("D1_DATABASE_ID")?)
    .api_token(std::env::var("CLOUDFLARE_API_TOKEN")?)
    .build()?;

let rows = pool
    .fetch_all_raw("SELECT id, email FROM users WHERE id = ?".into(), vec![Value::I64(7)])
    .await?;
# Ok(())
# }
```

The token needs the `D1:edit` permission on the account. It is sent as a bearer
credential and is redacted from `Debug` output.

## When to use it

This is the adapter for code that talks to D1 **from outside** a Worker — a CLI, a
migration job, a server. Inside a Worker you have a D1 binding, which is faster and
needs no token; this crate does not use bindings.

## What it does not do

- **No interactive transactions.** Every statement is one HTTP request, so `BEGIN` and
  `COMMIT` sent separately would not share a connection.
- **No binary parameters.** D1's HTTP interface takes only JSON scalars, so a `Bytes`
  value is refused rather than mangled. Store it hex- or base64-encoded in a text
  column.
- **No streaming transport.** `stream_raw` is a streaming *interface*: the whole result
  set arrives in one response, so memory use is that of the full result.
- **No column type information.** D1 answers in JSON, so an integer column and a text
  column holding digits are told apart by JSON type alone.

## Dialect

SQLite. Placeholders are `?`, and booleans bind as `1` and `0` — D1 refuses a JSON
boolean parameter.

## Testing

`cargo test -p ruprizzle-d1` runs the envelope unit tests and an end-to-end suite
against a local HTTP server, which asserts the request the adapter builds — path,
bearer token and bound parameters — as well as how it reads rows, change counts,
Cloudflare's error envelope and transport failures. No network access and no
Cloudflare account are needed.

## License

Licensed under either of MIT or Apache-2.0.
