//! SQL dialect abstraction.
//!
//! This crate is the seam between the dialect-agnostic [`Schema`] produced by the
//! parser and the SQL that a specific database understands. It is used by both
//! the migration engine (which needs DDL) and the codegen crate (which needs Rust
//! types and dialect-specific warnings).
//!
//! # Design
//!
//! * The [`DbDialect`] trait is **object-safe** and the single interface the rest
//!   of the workspace talks to.
//! * Every DDL operation returns a [`Vec<Stmt>`] because one logical change can
//!   require several physical statements, especially on SQLite.
//! * [`Stmt`] carries `destructive` and `transactional` metadata so the migration
//!   planner does not have to rediscover per-dialect quirks.
//! * Helper functions such as [`full_create_table`] and [`full_alter_column`] take
//!   a full [`Schema`] because generating correct foreign keys and table rebuilds
//!   needs resolved relation information that a single [`Model`] does not carry.
//!
//! # Adding a dialect
//!
//! 1. Add a variant to [`Provider`](ruprizzle_core::ir::Provider) in `core`.
//! 2. Implement [`DbDialect`] for a new type.
//! 3. Add it to [`dialect_for`].
//! 4. Add a row to the conformance suite so the new backend cannot rot.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]
#![allow(
    clippy::doc_markdown,
    clippy::struct_excessive_bools,
    clippy::too_many_lines
)]

mod common;
mod mysql;
mod postgres;
mod sqlite;

use ruprizzle_core::ir::{
    EnumDef, Field, IndexDef, Model, ResolvedRelation, ScalarType, Schema, UniqueDef,
};

pub use common::{
    check_schema_capabilities, dialect_for, full_alter_column, full_alter_column_with_source,
    full_create_table,
};
pub use mysql::MySqlDialect;
pub use postgres::PostgresDialect;
pub use sqlite::SqliteDialect;

/// A single DDL statement plus metadata the migration planner needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stmt {
    /// The SQL text to execute.
    pub sql: String,
    /// Whether the statement can silently delete user data.
    pub destructive: bool,
    /// Whether the statement must run inside a transaction.
    pub transactional: bool,
    /// A comment surfaced in the generated migration file.
    pub note: Option<String>,
}

impl Stmt {
    /// Creates a plain, transactional, non-destructive statement.
    #[must_use]
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            destructive: false,
            transactional: true,
            note: None,
        }
    }

    /// Adds a note to the statement.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Marks the statement as destructive.
    #[must_use]
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Marks the statement as non-transactional.
    #[must_use]
    pub fn non_transactional(mut self) -> Self {
        self.transactional = false;
        self
    }
}

/// What a dialect can and cannot do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Native `CREATE TYPE` for enums.
    pub native_enums: bool,
    /// Native UUID storage.
    pub native_uuid: bool,
    /// `ALTER TABLE ... ALTER COLUMN` can change the type.
    pub alter_column_type: bool,
    /// `ALTER TABLE ... DROP COLUMN` is supported.
    pub drop_column: bool,
    /// Foreign keys can be added after the table is created.
    pub add_fk_after_create: bool,
    /// `RETURNING` is supported.
    pub returning: bool,
    /// Partial indexes (`WHERE ...`) are supported.
    pub partial_indexes: bool,
    /// `DEFERRABLE` foreign keys are supported.
    pub deferrable_fks: bool,
    /// How `Json` is stored.
    pub json_type: JsonSupport,
    /// Maximum number of bind parameters in a single statement.
    pub max_query_params: u32,
    /// PostGIS extension is enabled.
    pub postgis: bool,
    /// `ROW_NUMBER() OVER (PARTITION BY ...)` and friends are supported.
    ///
    /// The relation loader needs this to honour a per-parent `take` in a single
    /// query. Without it the loader falls back to one query per parent, which is
    /// correct but no longer bounded.
    pub window_functions: bool,
}

/// How a dialect stores JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonSupport {
    /// A native JSON type.
    Native,
    /// Stored as `TEXT` and parsed on read.
    TextEncoded,
    /// Not supported at all.
    None,
}

/// The Rust type a column maps to, as seen by codegen.
///
/// In v1 this is the same for every dialect — the whole point of the abstraction
/// is that application code does not change when you switch `provider`. The
/// dialect reports this type so codegen does not have to hard-code the matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustType {
    /// `String`.
    String,
    /// `i32`.
    Int,
    /// `i64`.
    BigInt,
    /// `f64`.
    Float,
    /// `rust_decimal::Decimal`.
    Decimal,
    /// `bool`.
    Boolean,
    /// `chrono::DateTime<Utc>`.
    DateTime,
    /// `chrono::NaiveDate`.
    Date,
    /// `chrono::NaiveTime`.
    Time,
    /// `uuid::Uuid`.
    Uuid,
    /// `serde_json::Value`.
    Json,
    /// `Vec<u8>`.
    Bytes,
    /// A generated enum with the given name.
    Enum(String),
    /// `Option<T>`.
    Option(Box<RustType>),
    /// `Vec<T>` for scalar or enum lists.
    Vec(Box<RustType>),
}

/// An error produced while turning an IR field into a SQL column.
#[derive(Debug, thiserror::Error)]
pub enum DialectError {
    /// A `@db.*` native type is not supported by this dialect.
    #[error("`@db.{name}` is not supported on {dialect}")]
    UnsupportedNativeType {
        /// The dialect that rejected the type.
        dialect: String,
        /// Name of the native type, e.g. `VarChar`.
        name: String,
        /// Physical column the type was applied to.
        column: String,
    },

    /// A `@db.*` native type got an argument it does not accept.
    #[error("`@db.{name}({args})` is not valid: {reason}")]
    InvalidNativeArg {
        /// Name of the native type.
        name: String,
        /// Arguments as written.
        args: String,
        /// Why it is wrong.
        reason: String,
    },
}

/// The SQL dialect abstraction.
///
/// The trait is object-safe: `&dyn DbDialect` can be passed around, and
/// `dialect_for` returns a cheap `'static` reference. Implementations live in
/// [`PostgresDialect`], [`MySqlDialect`], and [`SqliteDialect`].
pub trait DbDialect: Send + Sync {
    /// The canonical name of the dialect, e.g. `"postgres"`.
    fn name(&self) -> &'static str;

    // ---- identifiers & literals ----

    /// Quote an identifier: `"users"` (PostgreSQL) / `` `users` `` (SQLite).
    fn quote_ident(&self, s: &str) -> String;

    /// Positional placeholder: `$1` (PostgreSQL) / `?` (SQLite).
    fn placeholder(&self, index: usize) -> String;

    // ---- type mapping ----

    /// SQL type for a column, before nullability or defaults are added.
    ///
    /// # Errors
    ///
    /// Returns an error when a `@db.*` native type is unsupported by this dialect.
    fn column_type(&self, f: &Field) -> Result<String, DialectError>;

    /// Rust type for a column, used by codegen.
    fn rust_type(&self, f: &Field) -> RustType;

    // ---- DDL ----

    /// `CREATE TABLE` for the body of a model.
    ///
    /// Foreign keys are intentionally emitted separately. For the full
    /// statement that a new schema needs, use [`full_create_table`].
    fn create_table(&self, schema: &Schema, m: &Model) -> Vec<Stmt>;

    /// `DROP TABLE`.
    fn drop_table(&self, table: &str) -> Vec<Stmt>;

    /// `ALTER TABLE ... ADD COLUMN`.
    fn add_column(&self, schema: &Schema, m: &Model, f: &Field) -> Vec<Stmt>;

    /// `ALTER TABLE ... DROP COLUMN`.
    fn drop_column(&self, table: &str, col: &str) -> Vec<Stmt>;

    /// Change a column.
    ///
    /// On PostgreSQL this is a direct `ALTER TABLE`. On SQLite it is the
    /// beginning of a table rebuild; for the complete rebuild sequence use
    /// [`full_alter_column`].
    fn alter_column(&self, schema: &Schema, m: &Model, from: &Field, to: &Field) -> Vec<Stmt>;

    /// `CREATE INDEX`.
    fn create_index(&self, m: &Model, ix: &IndexDef) -> Vec<Stmt>;

    /// `DROP INDEX`.
    fn drop_index(&self, table: &str, name: &str) -> Vec<Stmt>;

    /// `CREATE UNIQUE INDEX` or `ALTER TABLE ... ADD CONSTRAINT ... UNIQUE`.
    fn add_unique(&self, m: &Model, uq: &UniqueDef) -> Vec<Stmt>;

    /// `DROP INDEX` or `ALTER TABLE ... DROP CONSTRAINT ...`.
    fn drop_unique(&self, table: &str, name: &str) -> Vec<Stmt>;

    /// Add a foreign key for a resolved relation.
    ///
    /// * PostgreSQL returns a standalone `ALTER TABLE ... ADD CONSTRAINT`.
    /// * SQLite returns the inline `CONSTRAINT ... FOREIGN KEY` clause that
    ///   must be embedded in `CREATE TABLE`.
    fn add_foreign_key(&self, m: &Model, r: &ResolvedRelation) -> Vec<Stmt>;

    /// `ALTER TABLE ... DROP CONSTRAINT` (PostgreSQL) or a no-op (SQLite).
    fn drop_foreign_key(&self, m: &Model, r: &ResolvedRelation) -> Vec<Stmt>;

    /// `CREATE TYPE ... AS ENUM` (PostgreSQL) or a no-op (SQLite).
    fn create_enum(&self, e: &EnumDef) -> Vec<Stmt>;

    /// Add a variant to an existing enum.
    fn alter_enum_add_variant(&self, e: &EnumDef, v: &str) -> Vec<Stmt>;

    // ---- DML fragments used by the query builder ----

    /// Whether `RETURNING` is supported.
    fn returning_supported(&self) -> bool;

    /// The `ON CONFLICT` clause for an upsert.
    fn upsert_clause(&self, conflict: &[String], update: &[String]) -> String;

    /// The `LIMIT ... OFFSET ...` fragment.
    fn limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> String;

    /// Cast an expression to a scalar type.
    fn cast_expr(&self, expr: &str, ty: ScalarType) -> String;

    // ---- capabilities ----

    /// The capabilities of this dialect.
    fn capabilities(&self) -> Capabilities;

    /// Whether `RIGHT JOIN` is supported.
    fn supports_right_join(&self) -> bool {
        true
    }

    /// Whether `FULL OUTER JOIN` is supported.
    fn supports_full_join(&self) -> bool {
        true
    }

    /// Whether `INTERSECT` is supported.
    fn supports_intersect(&self) -> bool {
        true
    }

    /// Whether `EXCEPT` is supported.
    fn supports_except(&self) -> bool {
        true
    }
}
