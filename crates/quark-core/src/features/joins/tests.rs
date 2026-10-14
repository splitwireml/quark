//! Tests for join previews: request checks, workspace rules, relationship classification and counts.

use super::model::{Identity, JoinReference, JoinWorkspaceRequest, Relationship};
use super::service::{preview, preview_on, validate};
use crate::error::ApiResult;
use crate::runtime::cache::stats::{StatsEntry, stats_key};
use crate::runtime::engine::{Engine, EngineKind};
use crate::runtime::state::fixtures::*;
use crate::shared::ids::dataset_id;
use duckdb::Connection;
use serde_json::json;

fn engine(setup: &str) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(setup).unwrap();
    let kind = EngineKind::Node {
        node_id: "n1".to_owned(),
    };
    Engine::new(kind, conn).unwrap()
}

fn dataset(node: &str, name: &str) -> JoinReference {
    JoinReference {
        node_id: node.to_owned(),
        dataset: Some(dataset_id("main", name)),
        sql: None,
        name: None,
    }
}

fn view(node: &str, sql: &str) -> JoinReference {
    JoinReference {
        node_id: node.to_owned(),
        dataset: None,
        sql: Some(sql.to_owned()),
        name: Some("Side".to_owned()),
    }
}

fn request(
    left: JoinReference,
    right: JoinReference,
    left_keys: &[&str],
    right_keys: &[&str],
) -> JoinWorkspaceRequest {
    let owned = |keys: &[&str]| keys.iter().map(|key| (*key).to_owned()).collect();
    JoinWorkspaceRequest {
        left,
        right,
        left_keys: owned(left_keys),
        right_keys: owned(right_keys),
    }
}

fn run(engine: &Engine, request: &JoinWorkspaceRequest) -> ApiResult<super::model::JoinPreview> {
    engine
        .read(None, |conn| preview_on(engine, conn, request))
        .unwrap()
}

const TABLES: &str = "
    CREATE TABLE l(id INTEGER, code VARCHAR);
    INSERT INTO l VALUES (1, 'a'), (2, 'b'), (3, 'c'), (NULL, 'd');
    CREATE TABLE r_unique(id INTEGER);
    INSERT INTO r_unique VALUES (1), (2), (9);
    CREATE TABLE r_dupes(id INTEGER);
    INSERT INTO r_dupes VALUES (1), (1), (2);
    CREATE TABLE l_dupes(id INTEGER);
    INSERT INTO l_dupes VALUES (1), (1), (2);";

#[test]
fn key_validation_order_matches_python() {
    let same = || dataset("n1", "l");
    let bad_reference = JoinReference {
        sql: Some("SELECT 1".to_owned()),
        ..dataset("n1", "l")
    };
    // Key counts are judged before duplicates, and both before the references.
    let counts = request(bad_reference.clone(), same(), &["id", "id"], &["id"]);
    let duplicates = request(bad_reference.clone(), same(), &["id", "id"], &["id", "id"]);
    let references = request(bad_reference, same(), &["id"], &["id"]);
    let outcome = |request| failure(validate(&request));
    assert_eq!(
        outcome(counts),
        (422, "Join key counts must match".to_owned())
    );
    assert_eq!(
        outcome(duplicates),
        (422, "Join keys must be unique".to_owned())
    );
    assert_eq!(
        outcome(references),
        (
            422,
            "Join reference requires exactly one of dataset or sql".to_owned()
        )
    );
    let neither = JoinReference {
        dataset: None,
        ..same()
    };
    assert!(validate(&request(neither, same(), &[], &[])).is_err());
    assert!(validate(&request(same(), same(), &["id"], &["id"])).is_ok());
}

#[test]
fn sql_join_needs_one_workspace() {
    let root = tempfile::tempdir().unwrap();
    let state = started(
        root.path(),
        &[
            entry(root.path(), "a1", Some("a")),
            entry(root.path(), "b1", Some("b")),
        ],
        &[project("a"), project("b")],
    );
    let side = |node: &str| view(node, "SELECT 1 AS id");

    let across = request(side("project_a"), side("project_b"), &["id"], &["id"]);
    assert_eq!(
        failure(preview(&state, &across)),
        (422, "SQL joins require one project workspace".to_owned())
    );
    let unknown = request(side("project_a"), side("missing"), &["id"], &["id"]);
    assert_eq!(
        failure(preview(&state, &unknown)),
        (404, "Node not found".to_owned())
    );

    let within = request(side("project_a"), side("project_a"), &["id"], &["id"]);
    let result = preview(&state, &within).unwrap();
    assert_eq!(result.node_id, "project_a");
    assert_eq!(
        result.left,
        Identity::View {
            name: "Side".to_owned(),
            sql: "SELECT 1 AS id".to_owned()
        }
    );
    assert_eq!(
        (result.left_rows, result.output_rows, result.relationship),
        (1, 1, Relationship::OneToOne)
    );
}

#[test]
fn project_sources_are_not_default_project_nodes() {
    let root = tempfile::tempdir().unwrap();
    let state = started(
        root.path(),
        &[entry(root.path(), "a1", Some("a"))],
        &[project("a")],
    );
    let datasets = request(dataset("a1", "l"), dataset("a1", "l"), &["id"], &["id"]);
    assert_eq!(
        failure(preview(&state, &datasets)),
        (404, "Node not found".to_owned())
    );
}

#[test]
fn relationship_classification() {
    let engine = engine(TABLES);
    let relationship = |left: &str, right: &str| {
        let join = request(dataset("n1", left), dataset("n1", right), &["id"], &["id"]);
        run(&engine, &join).unwrap()
    };
    // The NULL key of `l` is ignored, so `l` is unique on `id`.
    let one_to_one = relationship("l", "r_unique");
    assert_eq!(one_to_one.relationship, Relationship::OneToOne);
    assert_eq!(
        (
            one_to_one.left_rows,
            one_to_one.right_rows,
            one_to_one.output_rows
        ),
        (4, 3, 2)
    );
    assert_eq!(
        relationship("l", "r_dupes").relationship,
        Relationship::OneToMany
    );
    assert_eq!(
        relationship("l_dupes", "r_unique").relationship,
        Relationship::ManyToOne
    );
    let many = relationship("l_dupes", "r_dupes");
    assert_eq!(many.relationship, Relationship::ManyToMany);
    assert_eq!(many.output_rows, 5);
    assert!(many.cartesian_risk);
    assert!(!one_to_one.cartesian_risk);
    assert_eq!(
        serde_json::to_value(&many).unwrap()["relationship"],
        json!("many_to_many")
    );

    let composite = request(
        dataset("n1", "l"),
        dataset("n1", "l"),
        &["id", "code"],
        &["id", "code"],
    );
    assert_eq!(run(&engine, &composite).unwrap().output_rows, 3);
    let missing = request(
        dataset("n1", "l"),
        dataset("n1", "r_unique"),
        &["code"],
        &["code"],
    );
    assert_eq!(
        failure(run(&engine, &missing)),
        (422, "Join key not found".to_owned())
    );
    let unknown = request(dataset("n1", "nope"), dataset("n1", "l"), &["id"], &["id"]);
    assert_eq!(
        failure(run(&engine, &unknown)),
        (404, "Dataset not found".to_owned())
    );
}

#[test]
fn cartesian_preview_multiplies_counts() {
    let engine = engine(TABLES);
    let join = request(
        dataset("n1", "l"),
        view("n1", "SELECT * FROM r_dupes"),
        &[],
        &[],
    );
    let result = run(&engine, &join).unwrap();
    assert_eq!(result.relationship, Relationship::Cartesian);
    assert_eq!(
        (result.left_rows, result.right_rows, result.output_rows),
        (4, 3, 12)
    );
    assert!(result.cartesian_risk);
    assert_eq!(result.node_id, "n1");
    assert_eq!(
        result.left,
        Identity::Dataset {
            schema: "main".to_owned(),
            name: "l".to_owned()
        }
    );
}

#[test]
fn counts_come_from_the_count_cache_when_present() {
    let engine = engine(TABLES);
    let key = stats_key("(SELECT * FROM \"main\".\"l\")", &[]);
    engine.stats().insert(
        key,
        StatsEntry {
            columns: Vec::new(),
            total_rows: 99,
        },
    );
    let join = request(dataset("n1", "l"), dataset("n1", "r_unique"), &[], &[]);
    let result = run(&engine, &join).unwrap();
    assert_eq!(
        (result.left_rows, result.right_rows, result.output_rows),
        (99, 3, 297)
    );
}
