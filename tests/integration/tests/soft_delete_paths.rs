//! Soft-deleted rows stay hidden on every read path, not only `SelectQuery` (K4).
//!
//! `SelectQuery` adds `deleted_at IS NULL` through `effective_filter`. These are
//! the paths that compile their own SQL and used to skip it: an include with a
//! per-parent `take`, the rows an m2m write reloads, the right-hand side of a
//! join, and recursive hierarchy queries. The relation filters (`_some`,
//! `_none`, `_every`) are generated code and are covered in `ruprizzle-codegen`.

use ruprizzle::{
    Column, Encodable, HierarchyQuery, IncludeList, InsertQuery, M2mAction, M2mWrite, Model,
    Related, SelectQuery,
};
use ruprizzle_testkit::both_dbs;

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Author {
    id: i64,
    name: String,
    #[sqlx(skip)]
    notes: Related<Vec<Note>>,
    #[sqlx(skip)]
    tags: Vec<Tag>,
}

impl Model for Author {
    const TABLE: &'static str = "authors";
}

/// Soft-deletable. The struct does not carry `deleted_at`: `SELECT *` returns
/// it and the derived `FromRow` ignores columns it does not name.
#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Note {
    id: i64,
    title: String,
    author_id: i64,
}

impl Model for Note {
    const TABLE: &'static str = "notes";
    const DELETED_AT_COLUMN: Option<&'static str> = Some("deleted_at");
}

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Tag {
    id: i64,
    name: String,
}

impl Model for Tag {
    const TABLE: &'static str = "tags";
    const DELETED_AT_COLUMN: Option<&'static str> = Some("deleted_at");
}

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
#[allow(dead_code)]
struct AuthorTag {
    author_id: i64,
    tag_id: i64,
}

impl Model for AuthorTag {
    const TABLE: &'static str = "author_tags";
}

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Node {
    id: i64,
    parent_id: Option<i64>,
    name: String,
}

impl Model for Node {
    const TABLE: &'static str = "nodes";
    const DELETED_AT_COLUMN: Option<&'static str> = Some("deleted_at");
}

#[cfg(feature = "postgres-tokio-postgres")]
mod tokio_rows {
    use super::*;
    ruprizzle::tokio_postgres_default_row!(Author);
    ruprizzle::tokio_postgres_default_row!(Note);
    ruprizzle::tokio_postgres_default_row!(Tag);
    ruprizzle::tokio_postgres_default_row!(AuthorTag);
    ruprizzle::tokio_postgres_default_row!(Node);
}

#[cfg(feature = "sqlite-rusqlite")]
mod rusqlite_rows {
    use super::*;
    use ruprizzle::rusqlite::{FromOwnedRow, FromRusqliteRow, Row, RusqliteRow, get};

    macro_rules! rusqlite_row {
        ($ty:ty, |$r:ident, $g:ident| $body:expr) => {
            impl FromRusqliteRow for $ty {
                fn from_rusqlite_row($r: &RusqliteRow) -> Result<Self, ruprizzle::Error> {
                    fn $g<T: ruprizzle::rusqlite::FromValue>(
                        r: &RusqliteRow,
                        i: usize,
                    ) -> Result<T, ruprizzle::Error> {
                        get::<T>(r, i)
                    }
                    Ok($body)
                }
            }
            impl FromOwnedRow for $ty {
                fn from_owned_row($r: &Row) -> Result<Self, ruprizzle::Error> {
                    fn $g<T: ruprizzle::rusqlite::FromValue>(
                        r: &Row,
                        i: usize,
                    ) -> Result<T, ruprizzle::Error> {
                        r.get::<T>(i)
                    }
                    Ok($body)
                }
            }
        };
    }

    rusqlite_row!(Author, |r, g| Author {
        id: g(r, 0)?,
        name: g(r, 1)?,
        notes: Related::default(),
        tags: Vec::new(),
    });
    rusqlite_row!(Note, |r, g| Note {
        id: g(r, 0)?,
        title: g(r, 1)?,
        author_id: g(r, 2)?,
    });
    rusqlite_row!(Tag, |r, g| Tag {
        id: g(r, 0)?,
        name: g(r, 1)?,
    });
    rusqlite_row!(AuthorTag, |r, g| AuthorTag {
        author_id: g(r, 0)?,
        tag_id: g(r, 1)?,
    });
    rusqlite_row!(Node, |r, g| Node {
        id: g(r, 0)?,
        parent_id: g(r, 1)?,
        name: g(r, 2)?,
    });
}

const AUTHOR_ID: Column<Author, i64> = Column::new("authors", "id");
const AUTHOR_NAME: Column<Author, String> = Column::new("authors", "name");
const NOTE_ID: Column<Note, i64> = Column::new("notes", "id");
const NOTE_AUTHOR_ID: Column<Note, i64> = Column::new("notes", "author_id");

const SETUP: &str = "CREATE TABLE authors (id BIGINT PRIMARY KEY, name TEXT NOT NULL);
     CREATE TABLE notes (id BIGINT PRIMARY KEY, title TEXT NOT NULL, author_id BIGINT NOT NULL, deleted_at TEXT NULL);
     CREATE TABLE tags (id BIGINT PRIMARY KEY, name TEXT NOT NULL, deleted_at TEXT NULL);
     CREATE TABLE author_tags (author_id BIGINT NOT NULL, tag_id BIGINT NOT NULL);
     CREATE TABLE nodes (id BIGINT PRIMARY KEY, parent_id BIGINT NULL, name TEXT NOT NULL, deleted_at TEXT NULL)";

fn notes() -> IncludeList<'static, Author, Note, i64, ()> {
    IncludeList::new(
        |a| a.id,
        |a, notes| a.notes = notes,
        NOTE_AUTHOR_ID,
        |n| n.author_id,
    )
}

/// Authors 1 and 2. Author 1 has notes 11, 12 (deleted) and 13; author 2 has
/// only note 21, which is deleted.
async fn seed_notes(db: &ruprizzle_testkit::TestDb) -> ruprizzle_testkit::Result {
    db.execute("INSERT INTO authors (id, name) VALUES (1, 'ann'), (2, 'bo')")
        .await?;
    db.execute(
        "INSERT INTO notes (id, title, author_id) VALUES \
         (11, 'a', 1), (12, 'b', 1), (13, 'c', 1), (21, 'd', 2)",
    )
    .await?;
    db.execute("UPDATE notes SET deleted_at = CURRENT_TIMESTAMP WHERE id IN (12, 21)")
        .await?;
    Ok(())
}

fn note_ids(author: &Author) -> Vec<i64> {
    let mut ids: Vec<i64> = author.notes.get().iter().map(|n| n.id).collect();
    ids.sort_unstable();
    ids
}

both_dbs! {
    setup = SETUP;
    /// `take(n)` compiles to a `ROW_NUMBER()` window instead of a `SelectQuery`.
    /// It must hide the same rows as the plain include does.
    async fn include_with_take_hides_soft_deleted_children(db: TestDb) {
        seed_notes(&db).await?;
        let pool = db.pool();

        for include in [notes(), notes().order_by(NOTE_ID.asc()).take(5)] {
            let mut authors: Vec<Author> = SelectQuery::<Author>::new(pool)
                .include(include)
                .exec()
                .await?;
            authors.sort_by_key(|a| a.id);
            assert_eq!(note_ids(&authors[0]), vec![11, 13]);
            assert_eq!(note_ids(&authors[1]), Vec::<i64>::new());
        }
    }
}

both_dbs! {
    setup = SETUP;
    /// The join's right-hand side is live-only, and the predicate sits in `ON`,
    /// so a `LEFT JOIN` keeps an author whose only note is deleted.
    async fn join_right_hand_side_hides_soft_deleted_rows(db: TestDb) {
        seed_notes(&db).await?;
        let pool = db.pool();

        let query = SelectQuery::<Author>::new(pool)
            .left_join::<Note>(AUTHOR_ID.on(NOTE_AUTHOR_ID));
        let compiled = query.to_sql()?;
        let on = compiled.sql.split(" ON ").nth(1).expect("has an ON clause");
        assert!(on.contains("deleted_at"), "{}", compiled.sql);

        // ann x {11, 13} plus bo x NULL. Before the fix: ann x 3 plus bo x 21.
        let rows = ruprizzle::Executor::fetch_all_raw(pool, compiled.sql, compiled.binds).await?;
        assert_eq!(rows.len(), 3);

        // with_deleted() opts the right-hand side out as well.
        let compiled = SelectQuery::<Author>::new(pool)
            .with_deleted()
            .left_join::<Note>(AUTHOR_ID.on(NOTE_AUTHOR_ID))
            .to_sql()?;
        let rows = ruprizzle::Executor::fetch_all_raw(pool, compiled.sql, compiled.binds).await?;
        assert_eq!(rows.len(), 4);
    }
}

both_dbs! {
    setup = SETUP;
    /// A deleted node is dropped, and so is everything reached only through it.
    async fn hierarchy_prunes_soft_deleted_nodes(db: TestDb) {
        // 1 -> 2 (deleted) -> 3,  and 1 -> 4
        db.execute(
            "INSERT INTO nodes (id, parent_id, name) VALUES \
             (1, NULL, 'root'), (2, 1, 'gone'), (3, 2, 'under-gone'), (4, 1, 'kept')",
        )
        .await?;
        db.execute("UPDATE nodes SET deleted_at = CURRENT_TIMESTAMP WHERE id = 2")
            .await?;
        let pool = db.pool();

        let mut down: Vec<i64> = HierarchyQuery::<Node>::descendants(pool, "nodes", "id", "parent_id", 1_i64)
            .all()
            .await?
            .iter()
            .map(|n| n.id)
            .collect();
        down.sort_unstable();
        assert_eq!(down, vec![1, 4]);

        let up: Vec<i64> = HierarchyQuery::<Node>::ancestors(pool, "nodes", "id", "parent_id", 3_i64)
            .all()
            .await?
            .iter()
            .map(|n| n.id)
            .collect();
        assert_eq!(up, vec![3], "the walk stops at the deleted parent");

        let from_deleted = HierarchyQuery::<Node>::descendants(pool, "nodes", "id", "parent_id", 2_i64)
            .all()
            .await?;
        assert!(from_deleted.is_empty());
    }
}

both_dbs! {
    setup = SETUP;
    /// After an m2m write, the attached relation is reloaded. It shows what an
    /// include would, so a deleted tag is not in it.
    async fn m2m_reload_hides_soft_deleted_targets(db: TestDb) {
        db.execute("INSERT INTO tags (id, name) VALUES (1, 'live'), (2, 'dead')")
            .await?;
        db.execute("UPDATE tags SET deleted_at = CURRENT_TIMESTAMP WHERE id = 2")
            .await?;
        let pool = db.pool();

        let write: M2mWrite<'_, Author, Tag, AuthorTag> = M2mWrite::new(
            M2mAction::Attach,
            |a: &Author| a.id.to_value(),
            "author_tags",
            "author_id",
            "tag_id",
            "tags",
            "id",
            vec![1_i64.to_value(), 2_i64.to_value()],
            |a: &mut Author, tags: Vec<Tag>| a.tags = tags,
        );
        let author: Author = InsertQuery::<Author>::new(pool)
            .set(AUTHOR_ID, 7)
            .set(AUTHOR_NAME, "cy")
            .with_m2m(write)
            .exec()
            .await?;

        let tag_ids: Vec<i64> = author.tags.iter().map(|t| t.id).collect();
        assert_eq!(tag_ids, vec![1]);
    }
}
