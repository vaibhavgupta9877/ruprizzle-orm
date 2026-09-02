//! Query Result and AST Plan Caching Tests.

use ruprizzle::compile::PlanCache;
use ruprizzle::prelude::*;
use std::time::Duration;

#[test]
fn in_memory_query_cache_ttl_and_tags() {
    let cache = InMemoryCache::new(500);

    // Basic set/get
    cache.set(
        "q1",
        b"result_data".to_vec(),
        Some(Duration::from_millis(50)),
        &["users"],
    );
    assert_eq!(cache.get("q1"), Some(b"result_data".to_vec()));
    assert_eq!(cache.len(), 1);

    // Tag invalidation
    cache.set("q2", b"other_data".to_vec(), None, &["users", "posts"]);
    cache.set("q3", b"posts_only".to_vec(), None, &["posts"]);
    assert_eq!(cache.len(), 3);

    cache.invalidate_tag("users");
    assert_eq!(cache.get("q1"), None);
    assert_eq!(cache.get("q2"), None);
    assert_eq!(cache.get("q3"), Some(b"posts_only".to_vec()));

    // Clear
    cache.clear();
    assert_eq!(cache.len(), 0);
}

#[test]
fn plan_cache_ast_queries() {
    let plan_cache = PlanCache::new();

    let ast_key = "SELECT:users:filter=id_eq";
    let sql_plan = "SELECT id, email FROM users WHERE id = $1";

    assert_eq!(plan_cache.get(ast_key), None);
    plan_cache.insert(ast_key.to_string(), sql_plan.to_string());

    assert_eq!(plan_cache.get(ast_key), Some(sql_plan.to_string()));
    assert_eq!(plan_cache.len(), 1);

    plan_cache.clear();
    assert_eq!(plan_cache.len(), 0);
}
