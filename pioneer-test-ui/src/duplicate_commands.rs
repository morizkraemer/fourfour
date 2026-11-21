use pioneer_library::{ArchivedMerge, DuplicateGroup, MergeReport, MergeRequest, RestoreOutcome, find_duplicate_groups};
use tauri::State;

use crate::analysis_run::SharedLibrary;
use crate::command_error::CommandError;

fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, CommandError> + Send + 'static) -> impl std::future::Future<Output = Result<T, CommandError>> {
    async move {
        tauri::async_runtime::spawn_blocking(work)
            .await
            .map_err(|e| CommandError::from(e.to_string()))?
    }
}

fn lock(library: &SharedLibrary) -> Result<std::sync::MutexGuard<'_, pioneer_library::LocalLibrary>, CommandError> {
    library.lock().map_err(|e| CommandError::from(e.to_string()))
}

pub fn list_groups(library: &SharedLibrary) -> Result<Vec<DuplicateGroup>, CommandError> {
    let rows = lock(library)?.duplicate_rows().map_err(CommandError::from_anyhow)?;
    Ok(find_duplicate_groups(&rows))
}

#[tauri::command]
pub async fn list_duplicate_groups(library: State<'_, SharedLibrary>) -> Result<Vec<DuplicateGroup>, CommandError> {
    let library = library.inner().clone();
    blocking(move || list_groups(&library)).await
}

#[tauri::command]
pub async fn apply_duplicate_merges(requests: Vec<MergeRequest>, library: State<'_, SharedLibrary>) -> Result<MergeReport, CommandError> {
    let library = library.inner().clone();
    blocking(move || Ok(lock(&library)?.apply_duplicate_merges(&requests))).await
}

#[tauri::command]
pub async fn list_duplicate_archive(library: State<'_, SharedLibrary>) -> Result<Vec<ArchivedMerge>, CommandError> {
    let library = library.inner().clone();
    blocking(move || lock(&library)?.list_archive().map_err(CommandError::from_anyhow)).await
}

#[tauri::command]
pub async fn restore_duplicate_merge(merge_id: i64, library: State<'_, SharedLibrary>) -> Result<RestoreOutcome, CommandError> {
    let library = library.inner().clone();
    blocking(move || lock(&library)?.restore_merge(merge_id).map_err(CommandError::from_anyhow_chain)).await
}
