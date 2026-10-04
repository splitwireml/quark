//! Executes one page of a dataset or SQL query: counts, null fractions and JSON rows.

use std::time::Instant;

use duckdb::arrow::datatypes::SchemaRef;
use duckdb::arrow::record_batch::RecordBatch;
use duckdb::types::{Null, ToSqlOutput};
use duckdb::{Connection, ToSql, params_from_iter};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::arrow::encode_page;
use crate::cache::results::{ResultAction, drop_statement, is_plain_scan};
use crate::cache::stats::{StatsEntry, stats_key};
use crate::engine::{EngineInner, datasets, describe};
use crate::error::{ApiError, ApiResult};
use crate::guard::check_select;
use crate::naming::page_count;
use crate::query::{ColumnMeta, QueryRequest, build_query};
use crate::sql::quote_ident;
use crate::values::{cell_json, is_numeric, profile_kind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageTarget {
    Dataset(String),
    Sql(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Arrow,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnSummary {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub numeric: bool,
    pub profile_kind: Option<&'static str>,
    pub null_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageMeta {
    pub columns: Vec<ColumnSummary>,
    pub page: i64,
    pub page_size: i64,
    pub total_rows: u64,
    pub total_pages: u64,
    pub elapsed_ms: f64,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PageBody {
    Json(Value),
    Arrow(Vec<u8>),
}

/// Binds a JSON value as a DuckDB parameter; arrays and objects bind as their JSON text.
struct Param<'a>(&'a Value);

impl ToSql for Param<'_> {
    fn to_sql(&self) -> duckdb::Result<ToSqlOutput<'_>> {
        Ok(match self.0 {
            Value::Null => Null.into(),
            Value::Bool(flag) => (*flag).into(),
            Value::Number(number) => match (number.as_i64(), number.as_u64()) {
                (Some(signed), _) => signed.into(),
                (None, Some(unsigned)) => unsigned.into(),
                _ => number.as_f64().unwrap_or_default().into(),
            },
            Value::String(text) => ToSqlOutput::from(text.as_str()),
            other => other.to_string().into(),
        })
    }
}

pub(crate) fn bind(params: &[Value]) -> impl duckdb::Params {
    params_from_iter(params.iter().map(Param))
}

/// What a target resolves to before the request's filters and sorts are applied.
struct Resolved {
    table: String,
    display_table: String,
    columns: Vec<ColumnMeta>,
    /// Parameters for placeholders inside `table`, bound before the filters'.
    lead: Vec<Value>,
    /// The `sql` to report when the request adds no filters, sorts or dedupe.
    plain_sql: Option<String>,
    error_prefix: &'static str,
}

fn internal(_: duckdb::Error) -> ApiError {
    ApiError::internal("Internal error")
}

/// DuckDB parser failures mean the text is not a single SELECT; anything else is a bad query.
fn describe_error(error: duckdb::Error) -> ApiError {
    let text = error.to_string();
    if text.starts_with("Parser Error") {
        ApiError::unprocessable("SQL accepts only one read-only SELECT query")
    } else {
        ApiError::unprocessable(format!("Invalid SQL query: {text}"))
    }
}

fn resolve(conn: &Connection, target: &PageTarget) -> ApiResult<Resolved> {
    match target {
        PageTarget::Dataset(id) => {
            let item = datasets(conn)
                .map_err(internal)?
                .into_iter()
                .find(|item| item.id == *id)
                .ok_or_else(|| ApiError::not_found("Dataset not found"))?;
            let table = format!("{}.{}", quote_ident(&item.schema), quote_ident(&item.name));
            let columns = describe(conn, &table, &[]).map_err(internal)?;
            Ok(Resolved {
                display_table: table.clone(),
                table,
                columns,
                lead: Vec::new(),
                plain_sql: None,
                error_prefix: "Invalid filter value: ",
            })
        }
        PageTarget::Sql(sql) => {
            let sql = check_select(conn, sql)?;
            let columns = describe(conn, "query(?)", &[&sql]).map_err(describe_error)?;
            Ok(Resolved {
                table: "query(?)".to_owned(),
                display_table: format!("({sql})"),
                columns,
                lead: vec![Value::String(sql.clone())],
                plain_sql: Some(sql),
                error_prefix: "Invalid SQL query: ",
            })
        }
    }
}

/// The page's Arrow schema and batches.
struct Fetched {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
}

/// Counts the relation's rows and measures each column's null fraction.
fn compute_stats(
    conn: &Connection,
    relation: &str,
    columns: &[ColumnMeta],
    params: &[Value],
) -> duckdb::Result<StatsEntry> {
    let total_rows = conn.query_row(
        &format!("SELECT count(*) FROM {relation}"),
        bind(params),
        |row| row.get(0),
    )?;

    let null_fractions: Vec<f64> = if columns.is_empty() {
        Vec::new()
    } else {
        let averages: Vec<String> = columns
            .iter()
            .map(|meta| {
                format!(
                    "avg(CASE WHEN {} IS NULL THEN 1.0 ELSE 0.0 END)",
                    quote_ident(&meta.name)
                )
            })
            .collect();
        conn.query_row(
            &format!("SELECT {} FROM {relation}", averages.join(", ")),
            bind(params),
            |row| {
                (0..columns.len())
                    .map(|index| Ok(row.get::<_, Option<f64>>(index)?.unwrap_or(0.0)))
                    .collect()
            },
        )?
    };

    let columns = columns
        .iter()
        .zip(null_fractions)
        .map(|(meta, null_fraction)| ColumnSummary {
            name: meta.name.clone(),
            type_name: meta.type_name.clone(),
            numeric: is_numeric(&meta.type_name),
            profile_kind: profile_kind(&meta.type_name),
            null_fraction,
        })
        .collect();
    Ok(StatsEntry {
        columns,
        total_rows,
    })
}

fn fetch_page(
    conn: &Connection,
    ordered: &str,
    params: &[Value],
    request: &QueryRequest,
) -> duckdb::Result<Fetched> {
    let paging = [
        Value::from(request.page_size),
        Value::from((request.page - 1).saturating_mul(request.page_size)),
    ];
    let params: Vec<Value> = params.iter().cloned().chain(paging).collect();
    let mut statement = conn.prepare(&format!(
        "SELECT * FROM ({ordered}) AS result LIMIT ? OFFSET ?"
    ))?;
    let arrow = statement.query_arrow(bind(&params))?;
    let schema = arrow.get_schema();
    let batches = arrow.collect();
    Ok(Fetched { schema, batches })
}

fn rows_json(batches: &[RecordBatch], columns: &[ColumnMeta]) -> Vec<Value> {
    let mut rows = Vec::with_capacity(batches.iter().map(RecordBatch::num_rows).sum());
    for batch in batches {
        for row in 0..batch.num_rows() {
            let object: Map<String, Value> = columns
                .iter()
                .zip(batch.columns())
                .map(|(meta, array)| {
                    let cell = cell_json(array.as_ref(), row, &meta.type_name, &chrono::Local);
                    (meta.name.clone(), cell)
                })
                .collect();
            rows.push(Value::Object(object));
        }
    }
    rows
}

/// Runs one page of `target` with the request's filters, sorts and dedupe applied.
pub fn run_page(
    inner: &mut EngineInner,
    target: &PageTarget,
    request: &QueryRequest,
    format: Format,
) -> ApiResult<PageBody> {
    let started = Instant::now();
    request.validate()?;
    let resolved = resolve(&inner.conn, target)?;
    let built = build_query(
        &resolved.table,
        &resolved.display_table,
        &resolved.columns,
        request,
    )?;
    let unprocessable =
        |error: duckdb::Error| ApiError::unprocessable(format!("{}{error}", resolved.error_prefix));

    let mut params = resolved.lead.clone();
    params.extend(built.params.iter().cloned());
    let has_controls = !request.filters.is_empty()
        || !request.sorts.is_empty()
        || !request.dedupe_columns.is_empty();
    // A scan with no controls reads its table or view as it is; a result table would only copy it.
    let is_plain = match &resolved.plain_sql {
        Some(sql) => is_plain_scan(request, sql, inner.mounted.values().flatten()),
        None => !has_controls,
    };
    let result_key = stats_key(&built.ordered, &params);
    let action = if is_plain {
        ResultAction::ServeDirect
    } else {
        inner.results.note_request(&result_key)
    };
    for table in inner.results.take_dropped() {
        if let Err(error) = inner.conn.execute_batch(&drop_statement(&table)) {
            tracing::warn!(table, %error, "could not drop an evicted result table");
        }
    }
    let saved = match &action {
        ResultAction::ServeFrom(table) => Some(format!("quark_results.{table}")),
        _ => None,
    };
    let saved_scan = saved.as_ref().map(|table| format!("SELECT * FROM {table}"));
    let (relation, ordered, bound) = match (&saved, &saved_scan) {
        (Some(table), Some(scan)) => (table.as_str(), scan.as_str(), [].as_slice()),
        _ => (
            built.relation.as_str(),
            built.ordered.as_str(),
            params.as_slice(),
        ),
    };

    let conn = &inner.conn;
    let key = stats_key(&built.relation, &params);
    let stats = match inner.stats.get(&key) {
        Some(stats) => stats,
        None => {
            inner.counters.count_queries += 1;
            let stats =
                compute_stats(conn, relation, &resolved.columns, bound).map_err(unprocessable)?;
            inner.stats.insert(key, stats.clone());
            stats
        }
    };
    let fetched = fetch_page(conn, ordered, bound, request).map_err(unprocessable)?;
    if let ResultAction::StartBuild(table) = &action {
        inner
            .results
            .start_build(conn, &result_key, table, &built.ordered, params);
    }

    let sql = match resolved.plain_sql {
        Some(plain) if !has_controls => plain,
        _ => built.display,
    };
    let meta = PageMeta {
        columns: stats.columns,
        page: request.page,
        page_size: request.page_size,
        total_rows: stats.total_rows,
        // validate() guarantees page_size >= 1.
        total_pages: page_count(stats.total_rows, request.page_size.unsigned_abs()),
        elapsed_ms: (started.elapsed().as_secs_f64() * 1_000_000.0).round() / 1000.0,
        sql,
    };
    let Ok(Value::Object(mut body)) = serde_json::to_value(&meta) else {
        return Err(ApiError::internal("Internal error"));
    };
    match format {
        Format::Json => {
            let rows = rows_json(&fetched.batches, &resolved.columns);
            body.insert("rows".to_owned(), Value::Array(rows));
            Ok(PageBody::Json(Value::Object(body)))
        }
        Format::Arrow => encode_page(&fetched.schema, &fetched.batches, &resolved.columns, body)
            .map(PageBody::Arrow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::results::ResultCache;
    use crate::engine::{Dirs, Engine};
    use crate::ids::dataset_id;
    use crate::query::{Direction, Filter, Sort};
    use crate::registry::SourceRecord;
    use arrow::ipc::reader::StreamReader;
    use duckdb::Connection;
    use serde_json::json;
    use std::io::Cursor;
    use std::thread;
    use std::time::{Duration, Instant};

    fn inner(setup: &str) -> EngineInner {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(setup).unwrap();
        EngineInner::new(conn)
    }

    fn dataset(name: &str) -> PageTarget {
        PageTarget::Dataset(dataset_id("main", name))
    }

    fn json_body(body: PageBody) -> Value {
        match body {
            PageBody::Json(value) => value,
            PageBody::Arrow(_) => panic!("expected a JSON body"),
        }
    }

    fn arrow_body(body: PageBody) -> Vec<u8> {
        match body {
            PageBody::Arrow(bytes) => bytes,
            PageBody::Json(_) => panic!("expected an Arrow body"),
        }
    }

    fn filter(column: &str, operator: &str, value: Value) -> Filter {
        Filter {
            column: column.to_owned(),
            operator: operator.to_owned(),
            value: Some(value),
            connector: Default::default(),
        }
    }

    #[test]
    fn pages_count_and_offsets() {
        let mut inner = inner("CREATE TABLE t AS SELECT range AS a FROM range(250)");
        let sorted = |page| QueryRequest {
            page,
            sorts: vec![Sort {
                column: "a".to_owned(),
                direction: Direction::Asc,
            }],
            ..QueryRequest::default()
        };

        let second =
            json_body(run_page(&mut inner, &dataset("t"), &sorted(2), Format::Json).unwrap());
        assert_eq!(second["page"], 2);
        assert_eq!(second["page_size"], 100);
        assert_eq!(second["total_rows"], 250);
        assert_eq!(second["total_pages"], 3);
        let rows = second["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 100);
        assert_eq!(rows[0], json!({"a": 100}));
        assert!(second["elapsed_ms"].as_f64().unwrap() >= 0.0);

        let last =
            json_body(run_page(&mut inner, &dataset("t"), &sorted(3), Format::Json).unwrap());
        assert_eq!(last["rows"].as_array().unwrap().len(), 50);

        let beyond =
            json_body(run_page(&mut inner, &dataset("t"), &sorted(4), Format::Json).unwrap());
        assert_eq!(beyond["rows"], json!([]));
        assert_eq!(beyond["total_rows"], 250);
    }

    fn count_queries(inner: &EngineInner) -> u64 {
        inner.counters.count_queries
    }

    fn page_of(page: i64, sorts: Vec<Sort>, filters: Vec<Filter>) -> QueryRequest {
        QueryRequest {
            page,
            sorts,
            filters,
            ..QueryRequest::default()
        }
    }

    fn sort_a(direction: Direction) -> Vec<Sort> {
        vec![Sort {
            column: "a".to_owned(),
            direction,
        }]
    }

    #[test]
    fn second_page_skips_count_and_nulls() {
        let mut inner = inner("CREATE TABLE t AS SELECT range AS a FROM range(250)");
        let run = |inner: &mut EngineInner, page| {
            json_body(
                run_page(
                    inner,
                    &dataset("t"),
                    &page_of(page, sort_a(Direction::Asc), Vec::new()),
                    Format::Json,
                )
                .unwrap(),
            )
        };

        let first = run(&mut inner, 1);
        let second = run(&mut inner, 2);

        assert_eq!(count_queries(&inner), 1);
        assert_eq!(second["total_rows"], 250);
        assert_eq!(second["columns"], first["columns"]);
        assert_eq!(second["rows"][0], json!({"a": 100}));
    }

    #[test]
    fn sort_change_reuses_stats() {
        let mut inner = inner("CREATE TABLE t AS SELECT range AS a FROM range(10)");
        let run = |inner: &mut EngineInner, sorts| {
            json_body(
                run_page(
                    inner,
                    &dataset("t"),
                    &page_of(1, sorts, Vec::new()),
                    Format::Json,
                )
                .unwrap(),
            )
        };

        let ascending = run(&mut inner, sort_a(Direction::Asc));
        let descending = run(&mut inner, sort_a(Direction::Desc));

        assert_eq!(count_queries(&inner), 1);
        assert_eq!(ascending["rows"][0], json!({"a": 0}));
        assert_eq!(descending["rows"][0], json!({"a": 9}));
        assert_eq!(descending["total_rows"], 10);
    }

    #[test]
    fn filter_change_misses() {
        let mut inner = inner("CREATE TABLE t AS SELECT range AS a FROM range(10)");
        let run = |inner: &mut EngineInner, bound| {
            json_body(
                run_page(
                    inner,
                    &dataset("t"),
                    &page_of(1, Vec::new(), vec![filter("a", ">", json!(bound))]),
                    Format::Json,
                )
                .unwrap(),
            )
        };

        let above_two = run(&mut inner, 2);
        let above_six = run(&mut inner, 6);
        let above_six_again = run(&mut inner, 6);

        assert_eq!(count_queries(&inner), 2);
        assert_eq!(above_two["total_rows"], 7);
        assert_eq!(above_six["total_rows"], 3);
        assert_eq!(above_six_again["total_rows"], 3);
    }

    #[test]
    fn arrow_format_carries_the_json_metadata() {
        let mut inner = inner(
            "CREATE TABLE t AS SELECT range::INTEGER AS a, DATE '2024-01-02' AS d FROM range(5)",
        );
        let request = QueryRequest {
            page_size: 2,
            ..QueryRequest::default()
        };
        let mut expected =
            json_body(run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap());

        let bytes =
            arrow_body(run_page(&mut inner, &dataset("t"), &request, Format::Arrow).unwrap());

        let reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
        let mut meta: Value = serde_json::from_str(&reader.schema().metadata()["quark"]).unwrap();
        let batches: Vec<RecordBatch> = reader.map(Result::unwrap).collect();
        assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 2);
        assert_eq!(
            meta.as_object_mut().unwrap().remove("json_columns"),
            Some(json!(["d"]))
        );
        for body in [&mut meta, &mut expected] {
            body.as_object_mut().unwrap().remove("elapsed_ms");
        }
        expected.as_object_mut().unwrap().remove("rows");
        assert_eq!(meta, expected);
    }

    #[test]
    fn null_fractions_per_column() {
        let mut inner = inner(
            "CREATE TABLE t (a INTEGER, b VARCHAR, c DATE);
             INSERT INTO t VALUES (1, 'x', NULL), (NULL, 'y', NULL), (3, 'z', NULL), (NULL, 'w', NULL)",
        );

        let body = json_body(
            run_page(
                &mut inner,
                &dataset("t"),
                &QueryRequest::default(),
                Format::Json,
            )
            .unwrap(),
        );

        assert_eq!(
            body["columns"],
            json!([
                {"name": "a", "type": "INTEGER", "numeric": true, "profile_kind": "numeric", "null_fraction": 0.5},
                {"name": "b", "type": "VARCHAR", "numeric": false, "profile_kind": "categorical", "null_fraction": 0.0},
                {"name": "c", "type": "DATE", "numeric": false, "profile_kind": "date", "null_fraction": 1.0},
            ])
        );
        assert_eq!(body["rows"][1], json!({"a": null, "b": "y", "c": null}));
    }

    #[test]
    fn header_only_csv_is_an_empty_page() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("empty.csv");
        std::fs::write(&file, "a,b\n").unwrap();
        let record = SourceRecord {
            id: "s1".to_owned(),
            name: "empty".to_owned(),
            kind: "file".to_owned(),
            source: file,
            project_id: None,
            dataset_name: Some("empty".to_owned()),
            sheets: None,
        };
        let dirs = Dirs {
            data: root.path().join("data"),
            cache: root.path().join("cache"),
        };
        let engine = Engine::open_node(&record, &dirs, 1).unwrap();

        let body = json_body(
            run_page(
                &mut engine.lock(),
                &dataset("empty"),
                &QueryRequest::default(),
                Format::Json,
            )
            .unwrap(),
        );

        assert_eq!(body["rows"], json!([]));
        assert_eq!(body["total_rows"], 0);
        assert_eq!(body["total_pages"], 0);
        let columns = body["columns"].as_array().unwrap();
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0]["name"], "a");
        assert_eq!(columns[1]["null_fraction"], 0.0);
    }

    #[test]
    fn sql_target_display_rules() {
        let mut inner = inner("");
        let sql = PageTarget::Sql("  select 7 as x  ".to_owned());

        let plain =
            json_body(run_page(&mut inner, &sql, &QueryRequest::default(), Format::Json).unwrap());
        assert_eq!(plain["sql"], "select 7 as x");
        assert_eq!(plain["rows"], json!([{"x": 7}]));

        let filtered = QueryRequest {
            filters: vec![filter("x", ">", json!(5))],
            ..QueryRequest::default()
        };
        let body = json_body(run_page(&mut inner, &sql, &filtered, Format::Json).unwrap());
        assert_eq!(
            body["sql"],
            "SELECT * FROM (SELECT * FROM (select 7 as x) WHERE \"x\" > 5)"
        );
        assert_eq!(body["total_rows"], 1);

        let sorted = QueryRequest {
            sorts: vec![Sort {
                column: "x".to_owned(),
                direction: Direction::Desc,
            }],
            ..QueryRequest::default()
        };
        let body = json_body(run_page(&mut inner, &sql, &sorted, Format::Json).unwrap());
        assert_eq!(
            body["sql"],
            "SELECT * FROM (SELECT * FROM (select 7 as x)) ORDER BY \"x\" DESC"
        );
    }

    #[test]
    fn sql_target_errors_map_to_422() {
        let mut inner = inner("");
        let run = |inner: &mut EngineInner, sql: &str| {
            run_page(
                inner,
                &PageTarget::Sql(sql.to_owned()),
                &QueryRequest::default(),
                Format::Json,
            )
            .unwrap_err()
        };

        let two = run(&mut inner, "select 1; select 2");
        assert_eq!(two.status(), 422);
        assert_eq!(two.detail(), "SQL accepts only one read-only SELECT query");

        let missing = run(&mut inner, "select * from missing_table");
        assert_eq!(missing.status(), 422);
        assert!(
            missing.detail().starts_with("Invalid SQL query: "),
            "{}",
            missing.detail()
        );

        let conversion = run(&mut inner, "select cast('x' as integer) as v");
        assert_eq!(conversion.status(), 422);
        assert!(
            conversion.detail().starts_with("Invalid SQL query: "),
            "{}",
            conversion.detail()
        );
    }

    #[test]
    fn parser_errors_while_describing_say_select_only() {
        let conn = Connection::open_in_memory().unwrap();

        let parser = describe(&conn, "query(?)", &[&"selec 1"]).unwrap_err();
        let other = describe(&conn, "query(?)", &[&"select * from missing"]).unwrap_err();

        assert_eq!(
            describe_error(parser).detail(),
            "SQL accepts only one read-only SELECT query"
        );
        assert!(
            describe_error(other)
                .detail()
                .starts_with("Invalid SQL query: ")
        );
    }

    #[test]
    fn invalid_value_maps_to_422() {
        let mut inner = inner("CREATE TABLE t AS SELECT 1 AS a");
        let request = QueryRequest {
            filters: vec![filter("a", "=", json!("abc"))],
            ..QueryRequest::default()
        };

        let error = run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap_err();

        assert_eq!(error.status(), 422);
        assert!(
            error.detail().starts_with("Invalid filter value: "),
            "{}",
            error.detail()
        );
    }

    #[test]
    fn unknown_dataset_is_404() {
        let mut inner = inner("");

        let error = run_page(
            &mut inner,
            &dataset("nope"),
            &QueryRequest::default(),
            Format::Json,
        )
        .unwrap_err();

        assert_eq!(error.status(), 404);
        assert_eq!(error.detail(), "Dataset not found");
    }

    const ROWS: &str = "CREATE TABLE t AS SELECT range AS id, range * 7 AS h FROM range(1000)";

    fn sorted_by_id(page: i64, direction: Direction) -> QueryRequest {
        QueryRequest {
            page,
            page_size: 10,
            sorts: vec![Sort {
                column: "id".to_owned(),
                direction,
            }],
            ..QueryRequest::default()
        }
    }

    fn without_elapsed(body: PageBody) -> Value {
        let mut body = json_body(body);
        body.as_object_mut().unwrap().remove("elapsed_ms");
        body
    }

    /// The ordered SQL of `sorted_by_id` over `main.<table>`, which keys the result cache.
    fn id_key(table: &str, direction: &str) -> String {
        stats_key(
            &format!(r#"SELECT * FROM (SELECT * FROM "main"."{table}") ORDER BY "id" {direction}"#),
            &[],
        )
    }

    fn wait_until_ready(inner: &mut EngineInner, key: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !matches!(inner.results.note_request(key), ResultAction::ServeFrom(_)) {
            assert!(Instant::now() < deadline, "the result never became ready");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn result_tables(conn: &Connection) -> i64 {
        count(
            conn,
            "SELECT count(*) FROM duckdb_tables() WHERE schema_name = 'quark_results'",
        )
    }

    fn has_results_schema(conn: &Connection) -> bool {
        count(
            conn,
            "SELECT count(*) FROM duckdb_schemas() WHERE schema_name = 'quark_results'",
        ) > 0
    }

    #[test]
    fn pages_from_results_equal_direct_pages() {
        let pages = [1, 2, 100];
        let direct: Vec<Value> = pages
            .iter()
            .map(|page| {
                let mut fresh = inner(ROWS);
                let request = sorted_by_id(*page, Direction::Asc);
                without_elapsed(
                    run_page(&mut fresh, &dataset("t"), &request, Format::Json).unwrap(),
                )
            })
            .collect();
        let mut inner = inner(ROWS);
        let first = sorted_by_id(1, Direction::Asc);
        run_page(&mut inner, &dataset("t"), &first, Format::Json).unwrap();
        run_page(&mut inner, &dataset("t"), &first, Format::Json).unwrap();
        wait_until_ready(&mut inner, &id_key("t", "ASC"));
        // Direct reads now see different values, so equal pages must come from the result table.
        inner.conn.execute_batch("UPDATE t SET h = -1").unwrap();

        for (page, expected) in pages.iter().zip(direct) {
            let request = sorted_by_id(*page, Direction::Asc);
            let body = run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
            assert_eq!(without_elapsed(body), expected, "page {page}");
        }
        assert_eq!(count_queries(&inner), 1);
    }

    #[test]
    fn result_tables_are_not_datasets() {
        let mut inner = inner(ROWS);
        let request = sorted_by_id(1, Direction::Asc);
        run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
        run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
        wait_until_ready(&mut inner, &id_key("t", "ASC"));
        assert_eq!(result_tables(&inner.conn), 1);

        let names: Vec<String> = datasets(&inner.conn)
            .unwrap()
            .into_iter()
            .map(|item| item.name)
            .collect();

        assert_eq!(names, ["t"]);
    }

    #[test]
    fn plain_scans_build_no_result() {
        let mut inner = inner(ROWS);

        for _ in 0..3 {
            let request = QueryRequest::default();
            run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
        }

        assert!(!has_results_schema(&inner.conn));
    }

    #[test]
    fn evicted_results_are_dropped() {
        let mut inner = inner(ROWS);
        inner.results = ResultCache::with_capacity(1);
        let ascending = sorted_by_id(1, Direction::Asc);
        run_page(&mut inner, &dataset("t"), &ascending, Format::Json).unwrap();
        run_page(&mut inner, &dataset("t"), &ascending, Format::Json).unwrap();
        wait_until_ready(&mut inner, &id_key("t", "ASC"));
        assert_eq!(result_tables(&inner.conn), 1);

        let descending = sorted_by_id(1, Direction::Desc);
        run_page(&mut inner, &dataset("t"), &descending, Format::Json).unwrap();

        assert_eq!(result_tables(&inner.conn), 0);
    }

    #[test]
    fn engine_rebuild_drops_results() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("rows.csv");
        let csv: String = (0..50).map(|id| format!("{id}\n")).collect();
        std::fs::write(&file, format!("id\n{csv}")).unwrap();
        let record = SourceRecord {
            id: "s1".to_owned(),
            name: "rows".to_owned(),
            kind: "file".to_owned(),
            source: file,
            project_id: None,
            dataset_name: Some("rows".to_owned()),
            sheets: None,
        };
        let dirs = Dirs {
            data: root.path().join("data"),
            cache: root.path().join("cache"),
        };
        let request = sorted_by_id(1, Direction::Asc);
        let first = Engine::open_node(&record, &dirs, 1).unwrap();
        {
            let mut inner = first.lock();
            run_page(&mut inner, &dataset("rows"), &request, Format::Json).unwrap();
            run_page(&mut inner, &dataset("rows"), &request, Format::Json).unwrap();
            wait_until_ready(&mut inner, &id_key("rows", "ASC"));
            assert_eq!(result_tables(&inner.conn), 1);
        }

        drop(first);
        let second = Engine::open_node(&record, &dirs, 2).unwrap();
        let mut inner = second.lock();
        assert_eq!(result_tables(&inner.conn), 0);
        run_page(&mut inner, &dataset("rows"), &request, Format::Json).unwrap();

        assert_eq!(count_queries(&inner), 1);
        assert_eq!(result_tables(&inner.conn), 0);
    }
}
