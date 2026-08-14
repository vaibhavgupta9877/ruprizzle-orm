//! Many-to-many relation loading through an explicit join model.

use ruprizzle::{
    Column, Encodable, IncludeMany, InsertQuery, M2mAction, M2mWrite, Model, Related, SelectQuery,
    UpdateQuery,
};
use ruprizzle_testkit::both_dbs;
use sqlx::FromRow;

#[derive(Debug, Clone, Default, FromRow)]
struct Post {
    id: i64,
    #[sqlx(skip)]
    tags: Related<Vec<Tag>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Post);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            tags: Related::default(),
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Post {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            tags: Related::default(),
        })
    }
}

impl Model for Post {
    const TABLE: &'static str = "m2m_posts";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id"];
}

#[derive(Debug, Clone, Default, FromRow)]
struct Tag {
    id: i64,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Tag);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Tag {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Tag {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
        })
    }
}

impl Model for Tag {
    const TABLE: &'static str = "m2m_tags";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id"];
}

#[derive(Debug, Clone, Default, FromRow)]
struct PostTag {
    post_id: i64,
    tag_id: i64,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(PostTag);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for PostTag {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            post_id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            tag_id: ::ruprizzle::rusqlite::get::<i64>(row, 1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for PostTag {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            post_id: row.get::<i64>(0)?,
            tag_id: row.get::<i64>(1)?,
        })
    }
}

impl Model for PostTag {
    const TABLE: &'static str = "m2m_post_tags";
    const PRIMARY_KEY: &'static str = "post_id";
    const COLUMNS: &'static [&'static str] = &["post_id", "tag_id"];
}

const POST_ID: Column<Post, i64> = Column::new("m2m_posts", "id");
const TAG_ID: Column<Tag, i64> = Column::new("m2m_tags", "id");
const POST_TAG_POST_ID: Column<PostTag, i64> = Column::new("m2m_post_tags", "post_id");
const POST_TAG_TAG_ID: Column<PostTag, i64> = Column::new("m2m_post_tags", "tag_id");

fn post_tags() -> IncludeMany<'static, Post, Tag, PostTag, i64, i64, ()> {
    IncludeMany::new(
        |p| p.id,
        |p, tags| p.tags = tags,
        |j| j.post_id,
        |j| j.tag_id,
        |t| t.id,
        POST_TAG_POST_ID,
        POST_TAG_TAG_ID,
        TAG_ID,
    )
}

fn post_tags_write(
    action: M2mAction,
    ids: impl IntoIterator<Item = i64>,
) -> M2mWrite<'static, Post, Tag, PostTag> {
    M2mWrite::new(
        action,
        |p: &Post| Encodable::to_value(&p.id),
        "m2m_post_tags",
        "post_id",
        "tag_id",
        "m2m_tags",
        "id",
        ids.into_iter().map(|id| Encodable::to_value(&id)).collect(),
        |p: &mut Post, tags: Vec<Tag>| p.tags = Related::Loaded(tags),
    )
}

const SETUP_SQL: &str = r#"
CREATE TABLE m2m_posts (
    id BIGINT PRIMARY KEY
);

CREATE TABLE m2m_tags (
    id BIGINT PRIMARY KEY
);

CREATE TABLE m2m_post_tags (
    post_id BIGINT NOT NULL,
    tag_id BIGINT NOT NULL,
    PRIMARY KEY (post_id, tag_id)
);

INSERT INTO m2m_posts (id) VALUES (1), (2);
INSERT INTO m2m_tags (id) VALUES (10), (20), (30);
INSERT INTO m2m_post_tags (post_id, tag_id) VALUES
    (1, 10),
    (1, 20),
    (2, 20),
    (2, 30);
"#;

both_dbs! {
    setup = SETUP_SQL;
    async fn many_to_many_include_loads_and_distributes(db: TestDb) {
        let posts: Vec<Post> = SelectQuery::<Post>::new(db.pool())
            .include(post_tags())
            .exec()
            .await?;

        assert_eq!(posts.len(), 2);
        let post1 = posts.iter().find(|p| p.id == 1).unwrap();
        let post2 = posts.iter().find(|p| p.id == 2).unwrap();

        let post1_tags: Vec<_> = post1.tags.get().iter().map(|t| t.id).collect();
        let post2_tags: Vec<_> = post2.tags.get().iter().map(|t| t.id).collect();

        assert_eq!(post1_tags, vec![10, 20]);
        assert_eq!(post2_tags, vec![20, 30]);
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn many_to_many_nested_writes(db: TestDb) {
        // Attach an extra tag to an existing post.
        UpdateQuery::<Post>::new(db.pool())
            .filter(POST_ID.eq(1i64))
            .with_m2m(post_tags_write(M2mAction::Attach, [30]))
            .exec()
            .await?;

        let post1 = SelectQuery::<Post>::new(db.pool())
            .filter(POST_ID.eq(1i64))
            .include(post_tags())
            .exec_one()
            .await?;
        let post1_tags: Vec<_> = post1.tags.get().iter().map(|t| t.id).collect();
        assert_eq!(post1_tags, vec![10, 20, 30]);

        // Set replaces all tags.
        UpdateQuery::<Post>::new(db.pool())
            .filter(POST_ID.eq(1i64))
            .with_m2m(post_tags_write(M2mAction::Set, [10]))
            .exec()
            .await?;

        let post1 = SelectQuery::<Post>::new(db.pool())
            .filter(POST_ID.eq(1i64))
            .include(post_tags())
            .exec_one()
            .await?;
        let post1_tags: Vec<_> = post1.tags.get().iter().map(|t| t.id).collect();
        assert_eq!(post1_tags, vec![10]);

        // Detach removes a tag.
        UpdateQuery::<Post>::new(db.pool())
            .filter(POST_ID.eq(1i64))
            .with_m2m(post_tags_write(M2mAction::Detach, [10]))
            .exec()
            .await?;

        let post1 = SelectQuery::<Post>::new(db.pool())
            .filter(POST_ID.eq(1i64))
            .include(post_tags())
            .exec_one()
            .await?;
        assert!(post1.tags.get().is_empty());

        // Insert a new post with tags.
        let post = InsertQuery::<Post>::new(db.pool())
            .set(POST_ID, 3i64)
            .with_m2m(post_tags_write(M2mAction::Set, [10, 20]))
            .exec()
            .await?;

        let post3_tags: Vec<_> = post.tags.get().iter().map(|t| t.id).collect();
        assert_eq!(post3_tags, vec![10, 20]);
    }
}
