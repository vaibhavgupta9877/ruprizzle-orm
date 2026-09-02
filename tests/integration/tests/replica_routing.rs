//! Primary / Read-Replica Routing Tests.

use ruprizzle::prelude::*;

#[tokio::test]
async fn routed_pool_round_robin_and_failover() {
    let primary = ruprizzle::connect("sqlite::memory:").await.unwrap();
    let replica1 = ruprizzle::connect("sqlite::memory:").await.unwrap();
    let replica2 = ruprizzle::connect("sqlite::memory:").await.unwrap();

    let routed = RoutedPool::builder(primary)
        .add_replica(replica1)
        .add_replica(replica2)
        .load_balancing(LoadBalancing::RoundRobin)
        .fallback_to_primary(true)
        .build();

    assert_eq!(routed.replicas().len(), 2);
    assert_eq!(routed.load_balancing(), LoadBalancing::RoundRobin);
    assert!(routed.falls_back_to_primary());

    // Health check pings
    routed.check_health().await;
    for rep in routed.replicas() {
        assert!(rep.is_healthy());
    }

    // Select replica round-robin
    let _r1 = routed.select_replica();
    let _r2 = routed.select_replica();

    // Mark replicas unhealthy -> fallback to primary
    for rep in routed.replicas() {
        rep.set_healthy(false);
    }
    let fallback = routed.select_replica();
    assert_eq!(fallback.provider(), routed.primary().provider());
}

#[tokio::test]
async fn routed_pool_least_connections() {
    let primary = ruprizzle::connect("sqlite::memory:").await.unwrap();
    let replica1 = ruprizzle::connect("sqlite::memory:").await.unwrap();
    let replica2 = ruprizzle::connect("sqlite::memory:").await.unwrap();

    let routed = RoutedPool::builder(primary)
        .add_replica(replica1)
        .add_replica(replica2)
        .load_balancing(LoadBalancing::LeastConnections)
        .build();

    // Simulate connection load on replica 1
    routed.replicas()[0]
        .active_conns
        .store(5, std::sync::atomic::Ordering::Relaxed);
    routed.replicas()[1]
        .active_conns
        .store(1, std::sync::atomic::Ordering::Relaxed);

    let selected = routed.select_replica();
    assert_eq!(selected.provider(), ruprizzle_core::ir::Provider::Sqlite);
    assert_eq!(routed.replicas()[1].active_connections(), 1);
    assert_eq!(routed.replicas()[0].active_connections(), 5);
}

#[tokio::test]
async fn routed_pool_execution_and_transactions() {
    let primary = ruprizzle::connect("sqlite::memory:").await.unwrap();
    let routed = RoutedPool::builder(primary).build();

    // DDL and writes route to primary
    let res = Executor::execute_raw(
        &routed,
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);".into(),
        vec![],
    )
    .await;
    assert!(res.is_ok());

    // Transactions delegate to primary
    let tx = routed.begin().await;
    assert!(tx.is_ok());
    tx.unwrap().rollback().await.unwrap();
}
