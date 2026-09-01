use rocklake_catalog::CatalogMetrics;

#[test]
fn connection_metrics_render_with_deprecated_aliases() {
    let metrics = CatalogMetrics::new(10);
    metrics.set_connections_open(3);
    metrics.set_connections_idle(2);
    metrics.set_queries_in_flight(1);

    let output = metrics.render_prometheus();
    for line in [
        "rocklake_connections_open 3",
        "rocklake_connections_idle 2",
        "rocklake_queries_in_flight 1",
        "rocklake_active_sessions 3",
        "rocklake_idle_sessions 2",
    ] {
        assert!(output.contains(line), "missing {line}");
    }
    assert!(output.contains(
        "# HELP rocklake_active_sessions Deprecated alias for rocklake_connections_open."
    ));
    assert!(output
        .contains("# HELP rocklake_idle_sessions Deprecated alias for rocklake_connections_idle."));
}

#[test]
fn legacy_session_setters_update_connection_metrics() {
    let metrics = CatalogMetrics::new(10);
    metrics.set_active_sessions(4);
    metrics.set_idle_sessions(2);

    assert_eq!(
        metrics
            .connections_open
            .load(std::sync::atomic::Ordering::Relaxed),
        4
    );
    assert_eq!(
        metrics
            .connections_idle
            .load(std::sync::atomic::Ordering::Relaxed),
        2
    );
}
