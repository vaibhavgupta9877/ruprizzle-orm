//! Query construction benchmark (no I/O).
//!
//! This measures the overhead of turning the typed query builder into SQL and
//! binds. It does not touch a database, so it is safe to run anywhere.

use std::borrow::Cow;
use std::pin::Pin;
use std::task::{Context, Poll};

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use futures_core::Stream;
use ruprizzle::executor::BoxRowStream;
use ruprizzle::{Column, Executor, Model, SelectQuery, Value};
use ruprizzle_core::ir::Provider;
use ruprizzle_dialect::{DbDialect, dialect_for};

#[derive(Default, sqlx::FromRow)]
#[allow(dead_code)]
struct User {
    id: i64,
    email: String,
    age: i64,
    name: String,
    created_at: i64,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(User);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            email: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            age: ::ruprizzle::rusqlite::get::<i64>(row, 2)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 3)?,
            created_at: ::ruprizzle::rusqlite::get::<i64>(row, 4)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            email: row.get::<String>(1)?,
            age: row.get::<i64>(2)?,
            name: row.get::<String>(3)?,
            created_at: row.get::<i64>(4)?,
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "users";
}

const ID: Column<User, i64> = Column::new("users", "id");
const EMAIL: Column<User, String> = Column::new("users", "email");
const AGE: Column<User, i64> = Column::new("users", "age");

struct NoopExecutor;

impl Executor for NoopExecutor {
    fn dialect(&self) -> &dyn DbDialect {
        dialect_for(Provider::Sqlite)
    }

    fn fetch_all_raw(
        &self,
        _sql: Cow<'static, str>,
        _binds: Vec<Value>,
    ) -> ruprizzle::BoxFuture<'_, Result<ruprizzle::executor::RowBatch, ruprizzle::Error>> {
        Box::pin(async { Ok(ruprizzle::executor::RowBatch::Any(Vec::new())) })
    }

    fn execute_raw(
        &self,
        _sql: Cow<'static, str>,
        _binds: Vec<Value>,
    ) -> ruprizzle::BoxFuture<'_, Result<u64, ruprizzle::Error>> {
        Box::pin(async { Ok(0) })
    }

    fn stream_raw(&self, _sql: Cow<'static, str>, _binds: Vec<Value>) -> BoxRowStream<'_> {
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

    c.bench_function("to_sql_select_by_pk", |b| {
        b.iter(|| {
            let q = SelectQuery::<User>::new(black_box(&exec))
                .filter(ID.eq(500i64))
                .limit(1)
                .offset(0);
            let _ = q.to_sql().unwrap();
        })
    });

    c.bench_function("to_sql_select_filter_order", |b| {
        b.iter(|| {
            let q = SelectQuery::<User>::new(black_box(&exec))
                .filter(AGE.gt(18i64).and(EMAIL.contains("@example.com")))
                .order_by(AGE.asc())
                .order_by(EMAIL.asc())
                .limit(1000)
                .offset(0);
            let _ = q.to_sql().unwrap();
        })
    });

    c.bench_function("to_sql_select_in_list", |b| {
        let ids: Vec<i64> = (1..=50).collect();
        b.iter(|| {
            let q = SelectQuery::<User>::new(black_box(&exec))
                .filter(ID.in_set(ids.clone()))
                .order_by(ID.asc())
                .limit(50);
            let _ = q.to_sql().unwrap();
        })
    });

    c.bench_function("to_sql_select_complex_filter", |b| {
        b.iter(|| {
            let q = SelectQuery::<User>::new(black_box(&exec))
                .filter(
                    AGE.gt(18i64)
                        .and(EMAIL.contains("example.com"))
                        .and(ID.between(100i64, 900i64)),
                )
                .order_by(AGE.asc())
                .order_by(EMAIL.asc())
                .limit(100);
            let _ = q.to_sql().unwrap();
        })
    });

    c.bench_function("to_sql_select_paginated", |b| {
        b.iter(|| {
            let q = SelectQuery::<User>::new(black_box(&exec))
                .filter(AGE.gt(18i64).and(EMAIL.contains("example.com")))
                .order_by(AGE.asc())
                .order_by(EMAIL.asc())
                .limit(20)
                .offset(500);
            let _ = q.to_sql().unwrap();
        })
    });

    c.bench_function("prepare_select_by_pk", |b| {
        b.iter(|| {
            let q = SelectQuery::<User>::new(black_box(&exec))
                .filter(ID.eq(500i64))
                .limit(1)
                .offset(0);
            let _ = q.prepare().unwrap();
        })
    });

    let prepared = SelectQuery::<User>::new(&exec)
        .filter(ID.eq(500i64))
        .limit(1)
        .offset(0)
        .prepare()
        .unwrap();
    c.bench_function("prepared_rebind_select_by_pk", |b| {
        b.iter(|| {
            let mut p = prepared.clone();
            p.bind(0, black_box(123i64));
        })
    });
}

criterion_group!(benches, query_construction);
criterion_main!(benches);
