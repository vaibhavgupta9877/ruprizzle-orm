//! Query construction benchmark (no I/O).
//!
//! This measures the overhead of turning the typed query builder into SQL and
//! binds. It does not touch a database, so it is safe to run anywhere.

use std::pin::Pin;
use std::task::{Context, Poll};

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use futures_core::Stream;
use ruprizzle::executor::BoxRowStream;
use ruprizzle::{Column, Executor, Model, SelectQuery, Value};
use ruprizzle_dialect::{DbDialect, SqliteDialect};

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct User {
    id: i64,
    email: String,
    age: i32,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

const ID: Column<User, i64> = Column::new("users", "id");
const EMAIL: Column<User, String> = Column::new("users", "email");
const AGE: Column<User, i32> = Column::new("users", "age");

struct NoopExecutor;

impl Executor for NoopExecutor {
    fn dialect(&self) -> Box<dyn DbDialect> {
        Box::new(SqliteDialect)
    }

    fn fetch_all_raw(
        &self,
        _sql: String,
        _binds: Vec<Value>,
    ) -> ruprizzle::BoxFuture<'_, Result<ruprizzle::executor::RowBatch, ruprizzle::Error>> {
        Box::pin(async { Ok(ruprizzle::executor::RowBatch::Any(Vec::new())) })
    }

    fn execute_raw(
        &self,
        _sql: String,
        _binds: Vec<Value>,
    ) -> ruprizzle::BoxFuture<'_, Result<u64, ruprizzle::Error>> {
        Box::pin(async { Ok(0) })
    }

    fn stream_raw(&self, _sql: String, _binds: Vec<Value>) -> BoxRowStream<'_> {
        Box::pin(NoopStream)
    }
}

struct NoopStream;

impl Stream for NoopStream {
    type Item = Result<ruprizzle::executor::RawRow, ruprizzle::Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }
}

fn query_construction(c: &mut Criterion) {
    let exec = NoopExecutor;

    c.bench_function("select_by_pk", |b| {
        b.iter(|| {
            let q = SelectQuery::<User>::new(black_box(&exec))
                .filter(ID.eq(1))
                .limit(1)
                .offset(0);
            let _ = q.to_sql();
        })
    });

    c.bench_function("select_with_filter_and_order", |b| {
        b.iter(|| {
            let q = SelectQuery::<User>::new(black_box(&exec))
                .filter(AGE.gte(18).and(EMAIL.contains("example")))
                .order_by(AGE.desc())
                .order_by(EMAIL.asc())
                .limit(100)
                .offset(0);
            let _ = q.to_sql();
        })
    });
}

criterion_group!(benches, query_construction);
criterion_main!(benches);
