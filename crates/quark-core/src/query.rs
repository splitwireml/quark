//! Request types shared by the data endpoints, and column metadata for responses.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{ApiError, ApiResult};

const MAX_PAGE_SIZE: i64 = 1000;

/// A column's name and DuckDB type, as reported in response metadata.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Connector {
    #[default]
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub column: String,
    pub operator: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub connector: Connector,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sort {
    pub column: String,
    pub direction: Direction,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    100
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryRequest {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sorts: Vec<Sort>,
    #[serde(default)]
    pub dedupe_columns: Vec<String>,
}

impl Default for QueryRequest {
    fn default() -> Self {
        Self {
            page: default_page(),
            page_size: default_page_size(),
            filters: Vec::new(),
            sorts: Vec::new(),
            dedupe_columns: Vec::new(),
        }
    }
}

impl QueryRequest {
    /// Rejects `page < 1` and `page_size` outside `1..=1000` with a 422.
    pub fn validate(&self) -> ApiResult<()> {
        if self.page < 1 || !(1..=MAX_PAGE_SIZE).contains(&self.page_size) {
            return Err(ApiError::unprocessable("Invalid page or page size"));
        }
        Ok(())
    }
}

/// A [`QueryRequest`] plus the SQL text. The fields are repeated because serde
/// cannot combine `flatten` with `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SqlQueryRequest {
    pub sql: String,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sorts: Vec<Sort>,
    #[serde(default)]
    pub dedupe_columns: Vec<String>,
}

impl SqlQueryRequest {
    pub fn query(&self) -> QueryRequest {
        QueryRequest {
            page: self.page,
            page_size: self.page_size,
            filters: self.filters.clone(),
            sorts: self.sorts.clone(),
            dedupe_columns: self.dedupe_columns.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
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

        let filter: Filter =
            serde_json::from_value(json!({"column": "a", "operator": "="})).unwrap();
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
            serde_json::from_value::<Filter>(
                json!({"column": "a", "operator": "=", "connector": word}),
            )
        };
        assert_eq!(connector("or").unwrap().connector, Connector::Or);
        assert!(connector("xor").is_err());
        assert!(connector("AND").is_err());

        let sort = |word| serde_json::from_value::<Sort>(json!({"column": "a", "direction": word}));
        assert_eq!(sort("desc").unwrap().direction, Direction::Desc);
        assert!(sort("up").is_err());
    }
}
