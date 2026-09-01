use std::sync::Arc;
use std::time::Duration;

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::metrics::CatalogMetrics;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_pgwire::server::{run_server_with_shutdown, ServerConfig};
use std::sync::atomic::Ordering;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};

async fn catalog(dir: &TempDir) -> Arc<Mutex<CatalogStore>> {
    let path = dir.path().to_str().unwrap().to_string();
    let store = Arc::new(LocalFileSystem::new_with_prefix(&path).unwrap());
    Arc::new(Mutex::new(
        CatalogStore::open(OpenOptions {
            object_store: store,
            path: ObjectPath::from(""),
            encryption: None,
        })
        .await
        .unwrap(),
    ))
}

async fn start_server(
    mut config: ServerConfig,
    catalog: Arc<Mutex<CatalogStore>>,
) -> (
    u16,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    config.bind_addr = listener.local_addr().unwrap();
    drop(listener);

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(run_server_with_shutdown(config, catalog, shutdown_rx));
    tokio::time::sleep(Duration::from_millis(50)).await;
    (port, shutdown_tx, server)
}

async fn connect(
    port: u16,
) -> (
    tokio_postgres::Client,
    tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
) {
    let connection_string = format!("host=127.0.0.1 port={port} user=test dbname=rocklake");
    for _ in 0..20 {
        match tokio_postgres::connect(&connection_string, tokio_postgres::NoTls).await {
            Ok((client, connection)) => {
                return (client, tokio::spawn(connection));
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }
    panic!("server did not accept a client connection");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capacity_rejection_is_prompt_and_uses_53300() {
    let dir = TempDir::new().unwrap();
    let catalog = catalog(&dir).await;
    let (port, shutdown_tx, server) = start_server(
        ServerConfig {
            max_sessions: 1,
            ..ServerConfig::default()
        },
        catalog,
    )
    .await;

    let (first_client, first_connection) = connect(port).await;
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        tokio_postgres::connect(
            &format!("host=127.0.0.1 port={port} user=test dbname=rocklake"),
            tokio_postgres::NoTls,
        ),
    )
    .await
    .expect("capacity rejection should be prompt");
    let error = match result {
        Ok((_client, _connection)) => panic!("the second connection must be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        error.as_db_error().map(|error| error.code().code()),
        Some("53300")
    );

    drop(first_client);
    let _ = first_connection.await;
    let _ = shutdown_tx.send(());
    server.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_connection_timeout_closes_an_inactive_socket() {
    let dir = TempDir::new().unwrap();
    let catalog = catalog(&dir).await;
    let (port, shutdown_tx, server) = start_server(
        ServerConfig {
            idle_connection_timeout: Duration::from_millis(50),
            ..ServerConfig::default()
        },
        catalog,
    )
    .await;
    let (_client, connection) = connect(port).await;

    tokio::time::timeout(Duration::from_secs(1), connection)
        .await
        .expect("idle connection should close")
        .expect("connection task should not panic")
        .ok();

    let _ = shutdown_tx.send(());
    server.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_query_is_not_closed_as_idle() {
    let dir = TempDir::new().unwrap();
    let catalog = catalog(&dir).await;
    let catalog_guard = catalog.lock().await;
    let metrics = Arc::new(CatalogMetrics::new(10));
    let (port, shutdown_tx, server) = start_server(
        ServerConfig {
            idle_connection_timeout: Duration::from_millis(100),
            metrics: Some(metrics.clone()),
            ..ServerConfig::default()
        },
        catalog.clone(),
    )
    .await;
    let (client, connection) = connect(port).await;
    let query = tokio::spawn(async move {
        client
            .simple_query("SELECT max(snapshot_id) FROM ducklake_snapshot")
            .await
    });

    for _ in 0..200 {
        if metrics.queries_in_flight.load(Ordering::Relaxed) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(metrics.queries_in_flight.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.connections_open.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.connections_idle.load(Ordering::Relaxed), 0);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !query.is_finished(),
        "the active query must outlive idle timeout"
    );
    drop(catalog_guard);
    query.await.unwrap().unwrap();
    let _ = shutdown_tx.send(());
    let _ = connection.await;
    server.await.unwrap().unwrap();
    assert_eq!(metrics.connections_open.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.connections_idle.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.queries_in_flight.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_closes_idle_and_drains_active_query() {
    let dir = TempDir::new().unwrap();
    let catalog = catalog(&dir).await;
    let catalog_guard = catalog.lock().await;
    let metrics = Arc::new(CatalogMetrics::new(10));
    let (port, shutdown_tx, server) = start_server(
        ServerConfig {
            drain_timeout: Duration::from_millis(500),
            metrics: Some(metrics.clone()),
            ..ServerConfig::default()
        },
        catalog.clone(),
    )
    .await;
    let (idle_client, idle_connection) = connect(port).await;
    let (client, connection) = connect(port).await;
    let query = tokio::spawn(async move {
        client
            .simple_query("SELECT max(snapshot_id) FROM ducklake_snapshot")
            .await
    });

    for _ in 0..100 {
        if metrics.queries_in_flight.load(Ordering::Relaxed) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(metrics.queries_in_flight.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.connections_open.load(Ordering::Relaxed), 2);
    assert_eq!(metrics.connections_idle.load(Ordering::Relaxed), 1);
    shutdown_tx.send(()).unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !server.is_finished(),
        "shutdown must wait for the active query"
    );
    let _ = tokio::time::timeout(Duration::from_secs(1), idle_connection)
        .await
        .expect("shutdown must close idle connections");

    drop(catalog_guard);
    query.await.unwrap().unwrap();
    let _ = connection.await;
    drop(idle_client);
    server.await.unwrap().unwrap();
}
