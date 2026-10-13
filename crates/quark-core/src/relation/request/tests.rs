use super::*;
use serde_json::json;

fn query(value: serde_json::Value) -> Result<QueryRequest, serde_json::Error> {
    serde_json::from_value(value)
}

#[test]
fn defaults_apply() {
    let request = query(json!({})).unwrap();
    assert_eq!(request.page, 1);
    assert_eq!(request.page_size, 100);
    assert!(request.filters.is_empty());
    assert!(request.sorts.is_empty());
    assert!(request.dedupe_columns.is_empty());

    let filter: Filter = serde_json::from_value(json!({"column": "a", "operator": "="})).unwrap();
    assert_eq!(filter.value, None);
    assert_eq!(filter.connector, Connector::And);
    let null: Filter =
        serde_json::from_value(json!({"column": "a", "operator": "=", "value": null})).unwrap();
    assert_eq!(null.value, None);

    let sql: SqlQueryRequest = serde_json::from_value(json!({"sql": "SELECT 1"})).unwrap();
    assert_eq!(sql.query(), request);
}

#[test]
fn unknown_fields_are_rejected() {
    assert!(query(json!({"page": 1, "extra": true})).is_err());
    let sql = json!({"sql": "SELECT 1", "extra": true});
    assert!(serde_json::from_value::<SqlQueryRequest>(sql).is_err());
}

#[test]
fn paging_bounds() {
    let at = |page, page_size| QueryRequest {
        page,
        page_size,
        ..QueryRequest::default()
    };
    assert!(at(1, 1).validate().is_ok());
    assert!(at(1, 1000).validate().is_ok());
    for bad in [at(0, 100), at(1, 0), at(1, 1001)] {
        let error = bad.validate().unwrap_err();
        assert_eq!(error.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    }
}

#[test]
fn connector_and_direction_reject_unknown_words() {
    let connector = |word| {
        serde_json::from_value::<Filter>(json!({"column": "a", "operator": "=", "connector": word}))
    };
    assert_eq!(connector("or").unwrap().connector, Connector::Or);
    assert!(connector("xor").is_err());
    assert!(connector("AND").is_err());

    let sort = |word| serde_json::from_value::<Sort>(json!({"column": "a", "direction": word}));
    assert_eq!(sort("desc").unwrap().direction, Direction::Desc);
    assert!(sort("up").is_err());
}
