//! Primary / Read-Replica Routing Tests.
//!
//! Each pool is its own SQLite file holding a different number of `marker`
//! rows (primary 1, replica A 2, replica B 3), so the row count of a routed
//! `SELECT` says which database actually served it. Comparing `provider()`
//! cannot: every pool here is SQLite.

use futures_util::StreamExt as _;
use ruprizzle::prelude::*;
use ruprizzle::{Pool, RoutedPoolBuilder};

const PRIMARY: usize = 1;
const REPLICA_A: usize = 2;
const REPLICA_B: usize = 3;

struct Cluster {
    _dir: tempfile::TempDir,
    primary: Pool,
    a: Pool,
    b: Pool,
}

async fn marked(dir: &tempfile::TempDir, name: &str, rows: usize) -> Pool {
    let path = dir.path().join(format!("{name}.db"));
    let url = format!(
        "sqlite://{}?mode=rwc",
        path.display().to_string().replace('\\', "/")
    );
    let pool = ruprizzle::connect(&url).await.unwrap();
    Executor::execute_raw(
        &pool,
        "CREATE TABLE marker (id INTEGER PRIMARY KEY)".into(),
        vec![],
    )
    .await
    .unwrap();
    for id in 1..=rows {
        Executor::execute_raw(
            &pool,
            format!("INSERT INTO marker (id) VALUES ({id})").into(),
            vec![],
        )
        .await
        .unwrap();
    }
    pool
}

async fn cluster() -> Cluster {
    let dir = tempfile::tempdir().unwrap();
    let primary = marked(&dir, "primary", PRIMARY).await;
    let a = marked(&dir, "replica_a", REPLICA_A).await;
    let b = marked(&dir, "replica_b", REPLICA_B).await;
    Cluster {
        _dir: dir,
        primary,
        a,
        b,
    }
}

impl Cluster {
    fn builder(&self) -> RoutedPoolBuilder {
        RoutedPool::builder(self.primary.clone())
            .add_replica(self.a.clone())
            .add_replica(self.b.clone())
    }
}

/// Which database served a routed read, by its marker row count.
async fn served_by(routed: &RoutedPool) -> usize {
    Executor::fetch_all_raw(routed, "SELECT id FROM marker".into(), vec![])
        .await
        .unwrap()
        .len()
}

async fn rows_in(pool: &Pool) -> usize {
    Executor::fetch_all_raw(pool, "SELECT id FROM marker".into(), vec![])
        .await
        .unwrap()
        .len()
}

#[tokio::test]
async fn round_robin_reads_alternate_between_healthy_replicas() {
    let c = cluster().await;
    let routed = c.builder().build();
    assert_eq!(routed.load_balancing(), LoadBalancing::RoundRobin);

    let mut served = Vec::new();
    for _ in 0..4 {
        served.push(served_by(&routed).await);
    }
    assert_eq!(served, vec![REPLICA_A, REPLICA_B, REPLICA_A, REPLICA_B]);

    routed.check_health().await;
    assert!(routed.replicas().iter().all(ReplicaPool::is_healthy));
}

#[tokio::test]
async fn an_unhealthy_replica_is_skipped() {
    let c = cluster().await;
    let routed = c.builder().build();
    routed.replicas()[0].set_healthy(false);

    for _ in 0..4 {
        assert_eq!(served_by(&routed).await, REPLICA_B);
    }
}

#[tokio::test]
async fn with_fallback_on_the_primary_serves_when_no_replica_is_healthy() {
    let c = cluster().await;
    let routed = c.builder().fallback_to_primary(true).build();
    for rep in routed.replicas() {
        rep.set_healthy(false);
    }

    assert_eq!(served_by(&routed).await, PRIMARY);
    assert_eq!(rows_in(routed.select_replica()).await, PRIMARY);
}

/// K6: `fallback_to_primary(false)` used to be stored and never read.
#[tokio::test]
async fn with_fallback_off_a_read_never_reaches_the_primary() {
    let c = cluster().await;
    let routed = c.builder().fallback_to_primary(false).build();
    assert!(!routed.falls_back_to_primary());
    for rep in routed.replicas() {
        rep.set_healthy(false);
    }

    let err = Executor::fetch_all_raw(&routed, "SELECT id FROM marker".into(), vec![])
        .await
        .expect_err("no healthy replica and no fallback");
    assert!(err.to_string().contains("fallback_to_primary"), "{err}");

    let mut stream = Executor::stream_raw(&routed, "SELECT id FROM marker".into(), vec![]);
    assert!(stream.next().await.expect("one item").is_err());
    drop(stream);

    // `select_replica` cannot fail, so it hands back a replica, not the primary.
    assert_eq!(rows_in(routed.select_replica()).await, REPLICA_A);

    // A router with no replicas has nothing to fall back from.
    let alone = RoutedPool::builder(c.primary.clone())
        .fallback_to_primary(false)
        .build();
    assert_eq!(served_by(&alone).await, PRIMARY);
}

/// K6: a raw write through the router used to be sent to a replica.
#[tokio::test]
async fn raw_writes_and_locking_reads_go_to_the_primary() {
    let c = cluster().await;
    let routed = c.builder().build();

    let returned = Executor::fetch_all_raw(
        &routed,
        "INSERT INTO marker (id) VALUES (100) RETURNING id".into(),
        vec![],
    )
    .await
    .unwrap();
    assert_eq!(returned.len(), 1);

    let mut stream = Executor::stream_raw(
        &routed,
        "INSERT INTO marker (id) VALUES (101) RETURNING id".into(),
        vec![],
    );
    while let Some(row) = stream.next().await {
        row.unwrap();
    }
    drop(stream);

    Executor::execute_raw(
        &routed,
        "INSERT INTO marker (id) VALUES (102)".into(),
        vec![],
    )
    .await
    .unwrap();

    assert_eq!(rows_in(&c.primary).await, PRIMARY + 3);
    assert_eq!(rows_in(&c.a).await, REPLICA_A);
    assert_eq!(rows_in(&c.b).await, REPLICA_B);
}

/// K6: `active_conns` used to be set only by hand, so `LeastConnections`
/// always chose the first replica.
#[tokio::test]
async fn least_connections_counts_reads_in_flight() {
    let c = cluster().await;
    let routed = c
        .builder()
        .load_balancing(LoadBalancing::LeastConnections)
        .build();
    let active = |i: usize| routed.replicas()[i].active_connections();

    // Both idle: the tie goes to the first replica, and the count is released
    // once the read returns.
    assert_eq!(served_by(&routed).await, REPLICA_A);
    assert_eq!((active(0), active(1)), (0, 0));

    // An open stream holds its replica until the stream is dropped.
    let held = Executor::stream_raw(&routed, "SELECT id FROM marker".into(), vec![]);
    assert_eq!((active(0), active(1)), (1, 0));

    assert_eq!(served_by(&routed).await, REPLICA_B);
    assert_eq!(served_by(&routed).await, REPLICA_B);

    drop(held);
    assert_eq!((active(0), active(1)), (0, 0));
    assert_eq!(served_by(&routed).await, REPLICA_A);
}

/// K6: `Random` used to be a fixed stride, which for two replicas is plain
/// alternation.
#[tokio::test]
async fn random_is_not_a_fixed_pattern() {
    let c = cluster().await;
    let routed = c.builder().load_balancing(LoadBalancing::Random).build();

    let mut served = Vec::new();
    for _ in 0..64 {
        served.push(served_by(&routed).await);
    }
    assert!(served.iter().all(|&s| s == REPLICA_A || s == REPLICA_B));
    assert!(served.contains(&REPLICA_A) && served.contains(&REPLICA_B));
    // Strict alternation over 64 fair draws has probability 2^-63.
    let alternates = served.windows(2).all(|w| w[0] != w[1]);
    assert!(!alternates, "{served:?}");
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

#[tokio::test]
async fn background_health_checks_restore_a_replica_until_stopped() {
    let c = cluster().await;
    let routed = c.builder().build();
    routed.replicas()[0].set_healthy(false);

    let task = routed.spawn_health_checks(std::time::Duration::from_millis(10));
    let restored = async {
        while !routed.replicas()[0].is_healthy() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), restored)
        .await
        .expect("the supervisor pings the live replica and marks it healthy");

    task.stop();
    routed.replicas()[0].set_healthy(false);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(
        !routed.replicas()[0].is_healthy(),
        "a stopped supervisor no longer touches the flags"
    );
}
