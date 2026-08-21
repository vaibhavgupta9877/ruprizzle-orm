//! Integration tests for Recursive CTE Tree Hierarchies (v1.3.0).

use ruprizzle::prelude::*;

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Category {
    id: String,
    parent_id: Option<String>,
    name: String,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Category);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Category {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<String>(row, 0)?,
            parent_id: ::ruprizzle::rusqlite::get::<Option<String>>(row, 1)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Category {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<String>(0)?,
            parent_id: row.get::<Option<String>>(1)?,
            name: row.get::<String>(2)?,
        })
    }
}

impl Model for Category {
    const TABLE: &'static str = "categories";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "parent_id", "name"];
}

const CAT_ID: Column<Category, String> = Column::new("categories", "id");
const CAT_PARENT_ID: Column<Category, Option<String>> = Column::new("categories", "parent_id");
const CAT_NAME: Column<Category, String> = Column::new("categories", "name");

async fn setup_db() -> (Pool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let file = path.to_str().unwrap().replace('\\', "/");
    let driver = if std::env::var("RUPRIZZLE_TEST_RUSQLITE").is_ok() {
        "&driver=rusqlite"
    } else {
        ""
    };
    let url = format!("sqlite:///{}?mode=rwc{}", file, driver);
    let pool = ruprizzle::connect(&url).await.unwrap();

    pool.execute_raw(
        "CREATE TABLE categories (id TEXT PRIMARY KEY, parent_id TEXT, name TEXT);"
            .to_string()
            .into(),
        vec![],
    )
    .await
    .unwrap();

    let items = vec![
        ("electronics", None, "Electronics"),
        ("computers", Some("electronics"), "Computers"),
        ("laptops", Some("computers"), "Laptops"),
        ("gaming_laptops", Some("laptops"), "Gaming Laptops"),
        ("desktops", Some("computers"), "Desktops"),
        ("audio", Some("electronics"), "Audio"),
        ("headphones", Some("audio"), "Headphones"),
    ];

    for (id, parent_id, name) in items {
        InsertQuery::<Category>::new(&pool)
            .set(CAT_ID, id)
            .set_optional(CAT_PARENT_ID, parent_id.map(|s| s.to_string()))
            .set(CAT_NAME, name)
            .exec()
            .await
            .unwrap();
    }

    (pool, dir)
}

#[tokio::test]
async fn test_ancestors_query() {
    let (pool, _dir) = setup_db().await;

    let ancestors = HierarchyQuery::<Category>::ancestors(
        &pool,
        "categories",
        "id",
        "parent_id",
        "gaming_laptops",
    )
    .order_by_depth_asc()
    .all()
    .await
    .expect("ancestors query should execute");

    let names: Vec<&str> = ancestors.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Gaming Laptops", "Laptops", "Computers", "Electronics"]
    );
}

#[tokio::test]
async fn test_descendants_query_with_depth_limit() {
    let (pool, _dir) = setup_db().await;

    // Descendants with max_depth = 2 (Root depth 0, children depth 1, grandchildren depth 2)
    let descendants = HierarchyQuery::<Category>::descendants(
        &pool,
        "categories",
        "id",
        "parent_id",
        "computers",
    )
    .max_depth(2)
    .order_by_depth_asc()
    .all()
    .await
    .unwrap();

    let names: Vec<&str> = descendants.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"Computers"));
    assert!(names.contains(&"Laptops"));
    assert!(names.contains(&"Desktops"));
    assert!(names.contains(&"Gaming Laptops"));
}

#[tokio::test]
async fn test_in_memory_hierarchy_node_tree_reconstruction() {
    let (pool, _dir) = setup_db().await;

    let root_items = HierarchyQuery::<Category>::ancestors(
        &pool,
        "categories",
        "id",
        "parent_id",
        "electronics",
    )
    .max_depth(1)
    .all()
    .await
    .unwrap();

    let root = root_items.into_iter().next().unwrap();

    let all_descendants = HierarchyQuery::<Category>::descendants(
        &pool,
        "categories",
        "id",
        "parent_id",
        "electronics",
    )
    .all()
    .await
    .unwrap();

    // Exclude root
    let descendants: Vec<Category> = all_descendants
        .into_iter()
        .filter(|c| c.id != root.id)
        .collect();

    let tree = HierarchyNode::from_flat(
        root,
        descendants,
        |c| c.id.clone().to_value(),
        |c| c.parent_id.clone().map(|p| p.to_value()),
    );

    assert_eq!(tree.item.name, "Electronics");
    assert_eq!(tree.depth, 0);
    assert_eq!(tree.children.len(), 2); // Computers, Audio
    assert_eq!(tree.count(), 7); // Total 7 categories in tree
    assert_eq!(tree.max_subtree_depth(), 3); // Root -> Computers -> Laptops -> Gaming Laptops

    let flattened = tree.flatten();
    assert_eq!(flattened.len(), 7);
}
