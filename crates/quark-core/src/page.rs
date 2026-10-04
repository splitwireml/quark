//! Executes one page of a dataset or SQL query: counts, null fractions and JSON rows.

use std::time::Instant;

use duckdb::arrow::record_batch::RecordBatch;
use duckdb::types::{Null, ToSqlOutput};
use duckdb::{Connection, ToSql, params_from_iter};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::engine::{EngineInner, datasets, describe};
use crate::error::{ApiError, ApiResult};
use crate::guard::check_select;
use crate::naming::page_count;
use crate::query::{BuiltQuery, ColumnMeta, QueryRequest, build_query};
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

fn bind(params: &[Value]) -> impl duckdb::Params {
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

/// The page's Arrow batches, the total row count and each column's null fraction.
struct Fetched {
    batches: Vec<RecordBatch>,
    total_rows: u64,
    null_fractions: Vec<f64>,
}

fn fetch(
    conn: &Connection,
    built: &BuiltQuery,
    columns: &[ColumnMeta],
    lead: &[Value],
    request: &QueryRequest,
) -> duckdb::Result<Fetched> {
    let mut params = lead.to_vec();
    params.extend(built.params.iter().cloned());
    let relation = &built.relation;

    let total_rows = conn.query_row(
        &format!("SELECT count(*) FROM {relation}"),
        bind(&params),
        |row| row.get(0),
    )?;

    let null_fractions = if columns.is_empty() {
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
            bind(&params),
            |row| {
                (0..columns.len())
                    .map(|index| Ok(row.get::<_, Option<f64>>(index)?.unwrap_or(0.0)))
                    .collect()
            },
        )?
    };

    params.push(Value::from(request.page_size));
    params.push(Value::from(
        (request.page - 1).saturating_mul(request.page_size),
    ));
    let mut statement = conn.prepare(&format!(
        "SELECT * FROM ({}) AS result LIMIT ? OFFSET ?",
        built.ordered
    ))?;
    let batches = statement.query_arrow(bind(&params))?.collect();
    Ok(Fetched {
        batches,
        total_rows,
        null_fractions,
    })
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
    if format == Format::Arrow {
        return Err(ApiError::not_implemented("Not in the desktop build yet"));
    }
    let conn = &inner.conn;
    let resolved = resolve(conn, target)?;
    let built = build_query(
        &resolved.table,
        &resolved.display_table,
        &resolved.columns,
        request,
    )?;
    let fetched = fetch(conn, &built, &resolved.columns, &resolved.lead, request)
        .map_err(|error| ApiError::unprocessable(format!("{}{error}", resolved.error_prefix)))?;

    let has_controls = !request.filters.is_empty()
        || !request.sorts.is_empty()
        || !request.dedupe_columns.is_empty();
    let sql = match resolved.plain_sql {
        Some(plain) if !has_controls => plain,
        _ => built.display,
    };
    let columns = resolved
        .columns
        .iter()
        .zip(&fetched.null_fractions)
        .map(|(meta, &null_fraction)| ColumnSummary {
            name: meta.name.clone(),
            type_name: meta.type_name.clone(),
            numeric: is_numeric(&meta.type_name),
            profile_kind: profile_kind(&meta.type_name),
            null_fraction,
        })
        .collect();
    let meta = PageMeta {
        columns,
        page: request.page,
        page_size: request.page_size,
        total_rows: fetched.total_rows,
        // validate() guarantees page_size >= 1.
        total_pages: page_count(fetched.total_rows, request.page_size.unsigned_abs()),
        elapsed_ms: (started.elapsed().as_secs_f64() * 1_000_000.0).round() / 1000.0,
        sql,
    };
    let mut body = serde_json::to_value(&meta).map_err(|_| ApiError::internal("Internal error"))?;
    body["rows"] = Value::Array(rows_json(&fetched.batches, &resolved.columns));
    Ok(PageBody::Json(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Dirs, Engine};
    use crate::ids::dataset_id;
    use crate::query::{Direction, Filter, Sort};
    use crate::registry::SourceRecord;
    use duckdb::Connection;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn inner(setup: &str) -> EngineInner {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(setup).unwrap();
        EngineInner {
            conn,
            mounted: BTreeMap::new(),
        }
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
}
