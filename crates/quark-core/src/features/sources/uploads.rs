//! Saving an uploaded file and registering it as a source, removing the file on every failure.

use std::path::{Path, PathBuf};

use serde_json::Value;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::service::{
    public_source, source_record, supported_extension, upload_source, uploaded_name,
};
use crate::error::{ApiError, ApiResult};
use crate::relation::describe::datasets;
use crate::runtime::AppState;
use crate::runtime::files::remove_with_retry;
use crate::runtime::registry::DEFAULT_PROJECT_ID;
use crate::runtime::state::cannot_open;

/// A file written to the uploads folder that is not yet in the catalog.
pub(super) struct SavedUpload {
    pub(super) node_id: String,
    pub(super) name: String,
    pub(super) path: PathBuf,
}

/// Where an upload's bytes come from, one chunk at a time.
pub(super) trait ChunkSource: Send {
    type Chunk: AsRef<[u8]>;

    /// The next chunk, or `None` once the body is complete.
    fn next_chunk(&mut self) -> impl Future<Output = ApiResult<Option<Self::Chunk>>> + Send;
}

/// Streams the chunks from `chunks` into a new upload named after `client_name`,
/// deleting the file if writing fails.
pub(super) async fn store(
    state: &AppState,
    client_name: &str,
    chunks: &mut impl ChunkSource,
) -> ApiResult<SavedUpload> {
    let name = uploaded_name(client_name).to_owned();
    let ext = supported_extension(&name)?;
    let node_id = Uuid::new_v4().simple().to_string();
    let path = state.upload_path(&node_id, &ext);
    if let Err(error) = write_chunks(chunks, &path).await {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(error);
    }
    Ok(SavedUpload {
        node_id,
        name,
        path,
    })
}

/// Adds the saved upload at `path` to the catalog, or deletes the file when that fails.
pub fn register_upload(
    state: &AppState,
    project_id: Option<&str>,
    node_id: &str,
    original_name: &str,
    path: &Path,
) -> ApiResult<Value> {
    let result = add_upload(state, project_id, node_id, original_name, path);
    if result.is_err() {
        remove_with_retry(path);
    }
    result
}

fn add_upload(
    state: &AppState,
    project_id: Option<&str>,
    node_id: &str,
    original_name: &str,
    path: &Path,
) -> ApiResult<Value> {
    state
        .catalog()
        .project(project_id.unwrap_or(DEFAULT_PROJECT_ID))?;
    let source = upload_source(project_id, node_id, original_name, path);
    let engine = state.open_node_engine(&source)?;
    datasets(&engine.primary().conn).map_err(cannot_open)?;
    let public = public_source(&source);
    let record = source_record(&source);
    state.register_source(source, record, engine)?;
    Ok(public)
}

/// Writes each chunk into a new file as it arrives, so memory stays flat.
async fn write_chunks(chunks: &mut impl ChunkSource, path: &Path) -> ApiResult<()> {
    let mut file = File::create_new(path).await.map_err(unsaved)?;
    while let Some(chunk) = chunks.next_chunk().await? {
        file.write_all(chunk.as_ref()).await.map_err(unsaved)?;
    }
    file.flush().await.map_err(unsaved)
}

fn unsaved(error: std::io::Error) -> ApiError {
    tracing::error!(%error, "could not save an upload");
    ApiError::internal("Internal error")
}

#[cfg(test)]
mod tests;
