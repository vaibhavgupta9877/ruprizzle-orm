//! Tree queries on cyclic parent links (K7).
//!
//! Cycle protection used to be only a depth cap of 100: on `A -> B -> A` the
//! `UNION ALL` emitted A and B about fifty times each. It now tracks the keys
//! on each path, so every node comes back once.

use ruprizzle::{HierarchyQuery, Model};
use ruprizzle_testkit::both_dbs;

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Node {
    id: i64,
    parent_id: Option<i64>,
}

impl Model for Node {
    const TABLE: &'static str = "nodes";
    const COLUMNS: &'static [&'static str] = &["id", "parent_id"];
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Node);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Node {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ruprizzle::rusqlite::get::<i64>(row, 0)?,
            parent_id: ruprizzle::rusqlite::get::<Option<i64>>(row, 1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Node {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            parent_id: row.get::<Option<i64>>(1)?,
        })
    }
}

const SETUP: &str = "CREATE TABLE nodes (id BIGINT PRIMARY KEY, parent_id BIGINT NULL)";

/// `1 -> 2 -> 3 -> 1` is a cycle. 4 and 11 hang off it; 11 shares a prefix
/// with 1, so a path check that matched substrings would drop it. 5 is its own
/// parent.
const SEED: &str = "INSERT INTO nodes (id, parent_id) VALUES \
     (1, 3), (2, 1), (3, 2), (4, 2), (11, 1), (5, 5)";

fn ids(nodes: &[Node]) -> Vec<i64> {
    let mut ids: Vec<i64> = nodes.iter().map(|n| n.id).collect();
    ids.sort_unstable();
    ids
}

both_dbs! {
    setup = SETUP;
    async fn a_cycle_returns_each_node_once(db: TestDb) {
        db.execute(SEED).await?;
        let pool = db.pool();

        let down = HierarchyQuery::<Node>::descendants(pool, "nodes", "id", "parent_id", 1_i64)
            .all()
            .await?;
        assert_eq!(ids(&down), vec![1, 2, 3, 4, 11]);

        // 4 -> 2 -> 1 -> 3, and 3's parent 2 is already on the path.
        let up = HierarchyQuery::<Node>::ancestors(pool, "nodes", "id", "parent_id", 4_i64)
            .order_by_depth_asc()
            .all()
            .await?;
        assert_eq!(up.iter().map(|n| n.id).collect::<Vec<_>>(), vec![4, 2, 1, 3]);

        let own_parent = HierarchyQuery::<Node>::descendants(pool, "nodes", "id", "parent_id", 5_i64)
            .all()
            .await?;
        assert_eq!(ids(&own_parent), vec![5]);
    }
}

both_dbs! {
    setup = SETUP;
    /// Without protection only `max_depth` stops the walk, and the cycle repeats.
    async fn without_protection_max_depth_still_bounds_a_cycle(db: TestDb) {
        db.execute(SEED).await?;
        let pool = db.pool();

        let down = HierarchyQuery::<Node>::descendants(pool, "nodes", "id", "parent_id", 5_i64)
            .cycle_protection(false)
            .max_depth(3)
            .all()
            .await?;
        assert_eq!(down.len(), 4, "5 at depths 0..=3");
    }
}
