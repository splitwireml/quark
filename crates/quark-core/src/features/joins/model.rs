//! Join preview request and response types.

use serde::{Deserialize, Serialize};

/// One side of a join: a dataset id or a SQL text on a node, but never both.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinReference {
    pub node_id: String,
    #[serde(default)]
    pub dataset: Option<String>,
    #[serde(default)]
    pub sql: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinWorkspaceRequest {
    pub left: JoinReference,
    pub right: JoinReference,
    pub left_keys: Vec<String>,
    pub right_keys: Vec<String>,
}

/// How a side is named in the response: a dataset by schema and name, a SQL view by name and text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Identity {
    Dataset { schema: String, name: String },
    View { name: String, sql: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Relationship {
    Cartesian,
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JoinPreview {
    pub node_id: String,
    pub left: Identity,
    pub right: Identity,
    pub left_rows: u64,
    pub right_rows: u64,
    pub output_rows: u64,
    pub relationship: Relationship,
    pub cartesian_risk: bool,
}
