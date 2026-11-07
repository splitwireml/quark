//! Response types for sources, views and datasets.

use serde::Serialize;

use crate::relation::describe::DatasetInfo;
use crate::runtime::mount::ViewInfo;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceDetail {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub project_id: String,
    pub views: Vec<ViewInfo>,
}

/// An active source of the default project, as listed by `GET /nodes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatasetEntry {
    #[serde(flatten)]
    pub info: DatasetInfo,
    pub columns: Vec<String>,
}
