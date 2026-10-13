//! Tests for project workspaces and source mounting.

use super::*;
use std::fs;

const DATA_SCHEMA: &str = "source_WyJzMSIsIld5SnRZV2x1SWl3aVpHRjBZU0pkIl0";

fn dirs_in(root: &Path) -> Dirs {
    Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    }
}

fn project() -> ProjectRecord {
    ProjectRecord {
        id: "p1".to_owned(),
        name: "P1".to_owned(),
        node_id: "project_p1".to_owned(),
    }
}

fn record(file: &Path, dataset_name: Option<&str>) -> SourceRecord {
    SourceRecord {
        id: "s1".to_owned(),
        name: "Claims".to_owned(),
        kind: "file".to_owned(),
        source: file.to_path_buf(),
        project_id: Some("p1".to_owned()),
        dataset_name: dataset_name.map(str::to_owned),
        sheets: None,
    }
}

fn workspace(source: &SourceRecord, root: &Path) -> Engine {
    Engine::open_workspace(&project(), std::slice::from_ref(source), &dirs_in(root), 1).unwrap()
}

fn csv(root: &Path, name: &str) -> std::path::PathBuf {
    let file = root.join(name);
    fs::write(&file, "a,b\n1,x\n2,y\n").unwrap();
    file
}

#[test]
fn workspace_views_have_python_ids_and_sql() {
    let root = tempfile::tempdir().unwrap();
    let source = record(&csv(root.path(), "claims.csv"), None);
    let engine = workspace(&source, root.path());

    let views = engine.mount(&project(), &source).unwrap();

    let sql = format!("SELECT * FROM \"{DATA_SCHEMA}\".\"data\"");
    assert_eq!(
        views,
        [ViewInfo {
            id: "WyJwMSIsInMxIiwiV3lKdFlXbHVJaXdpWkdGMFlTSmQiXQ".to_owned(),
            project_id: "p1".to_owned(),
            source_id: "s1".to_owned(),
            source_name: "Claims".to_owned(),
            node_id: "project_p1".to_owned(),
            name: "data".to_owned(),
            schema: "main".to_owned(),
            kind: "VIEW".to_owned(),
            columns: vec!["a".to_owned(), "b".to_owned()],
            sql: sql.clone(),
        }]
    );
    let json = serde_json::to_value(&views[0]).unwrap();
    assert_eq!(json["type"], "VIEW");
    assert!(json.get("kind").is_none());
    let rows: i64 = engine
        .lock()
        .conn
        .query_row(&format!("SELECT count(*) FROM ({sql})"), [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 2);
    assert_eq!(
        engine.kind,
        EngineKind::Workspace {
            project_id: "p1".to_owned(),
            node_id: "project_p1".to_owned(),
        }
    );
}

#[test]
fn mount_is_idempotent() {
    let root = tempfile::tempdir().unwrap();
    let source = record(&csv(root.path(), "claims.csv"), Some("claims"));
    let engine = workspace(&source, root.path());

    let first = engine.mount(&project(), &source).unwrap();
    let second = engine.mount(&project(), &source).unwrap();

    assert_eq!(first, second);
    assert_eq!(engine.lock().mounted.len(), 1);
}

#[test]
fn duckdb_sources_mount_every_table_read_only() {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("x.duckdb");
    Connection::open(&database)
        .unwrap()
        .execute_batch(
            "CREATE TABLE items (a INTEGER);
             INSERT INTO items VALUES (1), (2);
             CREATE VIEW v AS SELECT a FROM items;
             CREATE SCHEMA other;
             CREATE TABLE other.t (b VARCHAR, c DOUBLE)",
        )
        .unwrap();
    let source = record(&database, None);
    let engine = workspace(&source, root.path());

    let views = engine.mount(&project(), &source).unwrap();

    let summary: Vec<_> = views
        .iter()
        .map(|view| (view.schema.as_str(), view.name.as_str(), view.kind.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            ("main", "items", "TABLE"),
            ("main", "v", "VIEW"),
            ("other", "t", "TABLE")
        ]
    );
    assert_eq!(views[2].columns, ["b", "c"]);
    let guard = engine.lock();
    let rows: i64 = guard
        .conn
        .query_row(
            &views[0].sql.replace("SELECT *", "SELECT count(*)"),
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rows, 2);
    let alias = quote_ident(&database_alias("s1"));
    assert!(
        guard
            .conn
            .execute_batch(&format!("INSERT INTO {alias}.main.items VALUES (3)"))
            .is_err()
    );
    assert!(
        guard
            .conn
            .execute_batch(&format!("CREATE TABLE {alias}.main.more (a INTEGER)"))
            .is_err()
    );
}

#[test]
fn workspace_cannot_read_unlisted_files() {
    let root = tempfile::tempdir().unwrap();
    let listed = csv(root.path(), "listed.csv");
    let unlisted = csv(root.path(), "unlisted.csv");
    let source = record(&listed, None);
    let engine = workspace(&source, root.path());
    let rows = |file: &Path| {
        let scan = scan_expression(&file.to_string_lossy()).unwrap();
        engine
            .lock()
            .conn
            .query_row(&format!("SELECT count(*) FROM {scan}"), [], |row| {
                row.get::<_, i64>(0)
            })
    };

    assert_eq!(rows(&listed).unwrap(), 2);
    assert!(rows(&unlisted).is_err());

    let empty = Engine::open_workspace(&project(), &[], &dirs_in(root.path()), 1).unwrap();
    let scan = scan_expression(&listed.to_string_lossy()).unwrap();
    assert!(
        empty
            .lock()
            .conn
            .query_row(&format!("SELECT count(*) FROM {scan}"), [], |row| {
                row.get::<_, i64>(0)
            })
            .is_err()
    );
}

#[test]
fn workspace_allow_list_is_exact() {
    let root = tempfile::tempdir().unwrap();
    let flat = record(&csv(root.path(), "claims.csv"), None);
    let database = root.path().join("x.duckdb");
    Connection::open(&database)
        .unwrap()
        .execute_batch("CREATE TABLE items (a INTEGER)")
        .unwrap();
    let other = SourceRecord {
        id: "s2".to_owned(),
        source: database.clone(),
        ..record(&database, None)
    };
    let key = build_cache(&flat, root.path());
    let cache = key.final_path(&dirs_in(root.path()).cache);
    let relative = |file: &str| {
        let name = root.path().file_name().unwrap().to_str().unwrap();
        let file = file.replace('\\', "/");
        file.split_once(name).unwrap().1.to_owned()
    };
    let mut expected: Vec<String> = [
        flat.source.to_string_lossy().into_owned(),
        database.to_string_lossy().into_owned(),
        format!("{}.wal", database.display()),
        cache.to_string_lossy().into_owned(),
        format!("{}.wal", cache.display()),
    ]
    .iter()
    .map(|file| relative(file))
    .collect();
    expected.sort();

    let sources = [flat, other];
    let engine = Engine::open_workspace(&project(), &sources, &dirs_in(root.path()), 1).unwrap();
    let mut allowed: Vec<String> = engine
        .lock()
        .conn
        .prepare("SELECT unnest(current_setting('allowed_paths'))")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|file| relative(&file.unwrap()))
        .collect();
    // DuckDB keeps the list as a set, so only its contents are observable.
    allowed.sort();

    assert_eq!(allowed, expected);
}

fn build_cache(source: &SourceRecord, root: &Path) -> ColumnarKey {
    let job = import_job(source).unwrap();
    let dirs = dirs_in(root);
    fs::create_dir_all(dirs.columnar()).unwrap();
    Connection::open(job.key.final_path(&dirs.cache))
        .unwrap()
        .execute_batch(&format!(
            "CREATE TABLE data AS SELECT * FROM {}",
            job.scan_expression
        ))
        .unwrap();
    job.key
}

fn rows(engine: &Engine, view_sql: &str) -> Vec<String> {
    engine
        .lock()
        .conn
        .prepare(&format!(
            "SELECT CAST(t AS VARCHAR) FROM ({view_sql}) t ORDER BY 1"
        ))
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn definition(engine: &Engine) -> String {
    engine
        .lock()
        .conn
        .query_row(
            "SELECT sql FROM duckdb_views() WHERE NOT internal AND view_name = 'data'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

fn alias(key: &ColumnarKey) -> String {
    format!("cache_{}", &key.as_str()[..16])
}

#[test]
fn ready_cache_is_attached_read_only_with_identical_rows() {
    let root = tempfile::tempdir().unwrap();
    let source = record(&csv(root.path(), "claims.csv"), None);
    let live = workspace(&source, root.path());
    let live_views = live.mount(&project(), &source).unwrap();
    let key = build_cache(&source, root.path());

    let cached = workspace(&source, root.path());
    let views = cached.mount(&project(), &source).unwrap();

    assert_eq!(views, live_views);
    assert_eq!(rows(&cached, &views[0].sql), rows(&live, &views[0].sql));
    assert!(definition(&cached).contains(&alias(&key)));
    assert!(!definition(&live).contains("cache_"));
    let insert = format!("INSERT INTO {}.main.data VALUES (3, 'z')", alias(&key));
    assert!(cached.lock().conn.execute_batch(&insert).is_err());
}

#[test]
fn missing_cache_file_mounts_live() {
    let root = tempfile::tempdir().unwrap();
    let source = record(&csv(root.path(), "claims.csv"), None);
    let key = build_cache(&source, root.path());
    fs::remove_file(key.final_path(&dirs_in(root.path()).cache)).unwrap();

    let engine = workspace(&source, root.path());
    let views = engine.mount(&project(), &source).unwrap();

    assert!(!definition(&engine).contains("cache_"));
    assert_eq!(rows(&engine, &views[0].sql).len(), 2);
}

#[test]
fn edited_source_mounts_live_until_it_has_a_new_cache() {
    let root = tempfile::tempdir().unwrap();
    let file = csv(root.path(), "claims.csv");
    let source = record(&file, None);
    build_cache(&source, root.path());
    fs::write(&file, "a,b\n1,x\n2,y\n3,z\n").unwrap();

    let engine = workspace(&source, root.path());
    let views = engine.mount(&project(), &source).unwrap();

    assert!(!definition(&engine).contains("cache_"));
    assert_eq!(rows(&engine, &views[0].sql).len(), 3);
}

#[test]
fn engines_with_a_ready_cache_list_only_their_view() {
    let root = tempfile::tempdir().unwrap();
    let source = record(&csv(root.path(), "claims.csv"), None);
    let key = build_cache(&source, root.path());
    let listed = |engine: &Engine| -> Vec<(String, String)> {
        crate::relation::describe::datasets(&engine.lock().conn)
            .unwrap()
            .into_iter()
            .map(|item| (item.name, item.kind))
            .collect()
    };
    let only_view = [("data".to_owned(), "VIEW".to_owned())];

    let node = Engine::open_node(&source, &dirs_in(root.path()), 1).unwrap();
    let workspace = workspace(&source, root.path());
    workspace.mount(&project(), &source).unwrap();

    assert!(definition(&node).contains(&alias(&key)));
    assert!(definition(&workspace).contains(&alias(&key)));
    assert_eq!(listed(&node), only_view);
    assert_eq!(listed(&workspace), only_view);
    assert_eq!(rows(&node, "SELECT * FROM data").len(), 2);
}

#[test]
fn only_flat_files_have_an_import_job() {
    let root = tempfile::tempdir().unwrap();
    let flat = record(&csv(root.path(), "claims.csv"), None);
    let job = import_job(&flat).unwrap();
    assert_eq!(job.source_id, "s1");
    assert_eq!(job.source_path, flat.source);
    assert!(job.scan_expression.starts_with("read_csv_auto("));
    for name in ["x.duckdb", "x.db", "x.xlsx", "x.txt"] {
        let file = root.path().join(name);
        fs::write(&file, "x").unwrap();
        assert!(import_job(&record(&file, None)).is_none(), "{name}");
    }
    assert!(import_job(&record(&root.path().join("gone.csv"), None)).is_none());
}
