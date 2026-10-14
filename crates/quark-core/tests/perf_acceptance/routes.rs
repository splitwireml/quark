//! The spec section 9 route cases: each asserts one confirmed cap on the shared 2M-row CSV.
// clippy.toml allows unwrap only inside `#[test]` fns; the helpers below are test code too.

use super::*;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn stats_meet_their_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let uri = format!("{}/columns/price/stats", perf.base());
    json_case(perf, "stats, numeric column, filter on", ms(300), |run| {
        let filters = json!([{"column": "qty", "operator": ">", "value": 10 + run}]);
        ("POST", uri.clone(), Some(json!({ "filters": filters })))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn category_values_meet_their_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let uri = format!("{}/columns/category/values", perf.base());
    // The route has no filter, so each run asks for a different page size; `limit` is in the cache key.
    json_case(perf, "category values, first page", ms(150), |run| {
        ("GET", format!("{uri}?limit={}", 200 - run), None)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn bar_line_histogram_and_metric_charts_meet_their_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let specs = [
        (
            "bar chart",
            json!({"chart": "bar", "metric": "avg", "encodings": {"category": "category", "value": "price"}}),
        ),
        (
            "line chart",
            json!({"chart": "line", "metric": "avg", "encodings": {"x": "ordered", "y": "price"}}),
        ),
        (
            "histogram chart",
            json!({"chart": "histogram", "encodings": {"value": "price"}}),
        ),
        (
            "metric chart",
            json!({"chart": "metric", "metric": "avg", "encodings": {"value": "price"}}),
        ),
    ];
    for (case, spec) in specs {
        chart_case(perf, case, ms(500), spec).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn scatter_and_box_charts_meet_their_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let specs = [
        (
            "scatter chart",
            json!({"chart": "scatter", "encodings": {"x": "qty", "y": "price"}}),
        ),
        (
            "box chart",
            json!({"chart": "box", "encodings": {"value": "price", "group": "region"}}),
        ),
    ];
    for (case, spec) in specs {
        chart_case(perf, case, ms(800), spec).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn find_next_meets_its_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let uri = format!("/api/nodes/{NODE}/sql/find");
    let mut samples = Vec::with_capacity(RUNS);
    for run in 0..RUNS {
        // A new term each run, so no request repeats; the numbered copy of the View is shared.
        let term = format!("Item {}", 77 + run);
        let search = json!({"sql": perf.select_all(), "term": term, "columns": ["name"]});
        let first = timed_json(&perf.app, &uri, search.clone()).await;
        let cell = &first["match"];
        let mut next = search.clone();
        next["after_row"] = cell["row"].clone();
        next["after_column"] = cell["column_index"].clone();
        samples.push(timed(&perf.app, "POST", &uri, Some(next), "application/json").await);
    }
    assert_cap("find next after the first search", samples, ms(200));
}

async fn timed_json(app: &Router, uri: &str, body: Value) -> Value {
    let request = Request::post(uri).header(header::CONTENT_TYPE, "application/json");
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

/// Whether the node's engine reads its source from a columnar cache file. An engine opened
/// before the import finished scans the CSV; once the import is done the next open reads the cache.
fn reads_cache(state: &AppState, node_id: &str) -> bool {
    let engine = state.engine_for_node(node_id).unwrap();
    let views: i64 = engine
        .primary()
        .conn
        .query_row(
            "SELECT count(*) FROM duckdb_views() WHERE NOT internal AND contains(sql, 'cache_')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    views > 0
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn join_preview_meets_its_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let copy = perf.csv.with_file_name("perf-right.csv");
    std::fs::copy(&perf.csv, &copy).unwrap();
    register_upload(&perf.state, None, "right", "perf-right.csv", &copy).unwrap();
    // Both sources must read their columnar cache, so the case never times a live CSV scan.
    wait_until("the right source's columnar import", || {
        reads_cache(&perf.state, "right")
    });
    let right = datasets(&perf.state, "right").unwrap().remove(0).info.id;
    let body = json!({
        "left": {"node_id": NODE, "dataset": perf.dataset},
        "right": {"node_id": "right", "dataset": right},
        "left_keys": ["id"],
        "right_keys": ["id"],
    });
    json_case(perf, "join preview, one-to-one key", ms(1000), |_| {
        (
            "POST",
            "/api/join-workspaces".to_owned(),
            Some(body.clone()),
        )
    })
    .await;
}

/// Downloads an export and returns how long the whole body took.
async fn timed_export(perf: &Perf, body: Value) -> Duration {
    timed(&perf.app, "POST", "/api/exports", Some(body), "*/*").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn csv_export_of_two_million_rows_meets_its_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let mut samples = Vec::new();
    for _ in 0..3 {
        samples.push(timed_export(perf, perf.export_body("csv", ROWS)).await);
    }
    assert_cap("csv export, 2M rows", samples, Duration::from_secs(5));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn xlsx_export_of_one_million_rows_meets_its_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let elapsed = timed_export(perf, perf.export_body("xlsx", 1_000_000)).await;
    assert_cap(
        "xlsx export, 1,000,000 rows",
        vec![elapsed],
        Duration::from_secs(15),
    );
}

/// 100,000 rows by 10 columns: numbers in odd columns and text in even ones.
fn write_workbook(path: &Path) {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet_with_constant_memory();
    sheet.set_name("Orders").unwrap();
    for column in 0..WORKBOOK_COLUMNS {
        sheet
            .write_string(0, column, format!("column_{column}"))
            .unwrap();
    }
    for row in 0..WORKBOOK_ROWS {
        for column in 0..WORKBOOK_COLUMNS {
            let at = row + 1;
            if column % 2 == 1 {
                sheet
                    .write_number(
                        at,
                        column,
                        f64::from(row * u32::from(WORKBOOK_COLUMNS) + u32::from(column)),
                    )
                    .unwrap();
            } else {
                sheet
                    .write_string(at, column, format!("value {row}-{column}"))
                    .unwrap();
            }
        }
    }
    workbook.save(path).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn workbook_confirm_of_100000_rows_meets_its_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let workbook = perf.csv.with_file_name("orders.xlsx");
    write_workbook(&workbook);
    let mut samples = Vec::new();
    for run in 0..3 {
        let stage = format!("orders{run}");
        let staged = perf.csv.with_file_name(format!("{stage}.xlsx"));
        std::fs::copy(&workbook, &staged).unwrap();
        let name = format!("{stage}.xlsx");
        let public = register_upload(&perf.state, None, &stage, &name, &staged).unwrap();
        let body = json!({"sheets": public["sheets"].as_array().unwrap()[..1]});
        let uri = format!("/api/nodes/upload/{stage}/confirm");
        samples.push(timed(&perf.app, "POST", &uri, Some(body), "application/json").await);
    }
    assert_cap(
        "workbook confirm, 100,000 x 10",
        samples,
        Duration::from_secs(3),
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
async fn page_during_an_export_meets_its_cap() {
    let _serial = SERIAL.lock().await;
    let perf = perf();
    let uri = format!("{}/query", perf.base());
    // Opening the dataset fills the count cache, as it does for a user who scrolls to a deeper page.
    timed(&perf.app, "POST", &uri, Some(json!({"page": 1})), ARROW).await;
    for attempt in 0..3 {
        let app = perf.app.clone();
        let body = perf.export_body("csv", ROWS);
        let export =
            tokio::spawn(
                async move { timed(&app, "POST", "/api/exports", Some(body), "*/*").await },
            );
        tokio::time::sleep(ms(300)).await;
        let page = timed(
            &perf.app,
            "POST",
            &uri,
            // Row 1,000,000 plus 100 rows per attempt: a page no request has asked for before.
            Some(json!({"page": 10_001 + attempt})),
            ARROW,
        )
        .await;
        let overlapped = !export.is_finished();
        export.await.unwrap();
        if overlapped {
            assert_cap("page at row 1,000,000 during export", vec![page], ms(60));
            return;
        }
    }
    panic!("no export outlasted the timed page");
}
