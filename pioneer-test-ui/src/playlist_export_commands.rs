use std::path::Path;

use pioneer_library::playlist_export::{ExportFormat, ExportReport};
use tauri::State;

use crate::analysis_run::SharedLibrary;
use crate::command_error::CommandError;

#[tauri::command(async)]
pub fn export_playlist(
    playlist_id: i64,
    format: ExportFormat,
    dest_path: String,
    library: State<'_, SharedLibrary>,
) -> Result<ExportReport, CommandError> {
    let document = library
        .lock()
        .map_err(|e| e.to_string())?
        .export_document(playlist_id)
        .map_err(CommandError::from_anyhow_chain)?;
    document
        .write(format, Path::new(&dest_path))
        .map_err(|e| CommandError::from_anyhow_chain(e).with_path(&dest_path))
}

#[tauri::command(async)]
pub fn playlist_export_file_name(
    playlist_id: i64,
    format: ExportFormat,
    library: State<'_, SharedLibrary>,
) -> Result<String, CommandError> {
    library
        .lock()
        .map_err(|e| e.to_string())?
        .export_file_name(playlist_id, format)
        .map_err(CommandError::from_anyhow_chain)
}

#[tauri::command(async)]
pub fn playlist_tracklist(playlist_id: i64, library: State<'_, SharedLibrary>) -> Result<String, CommandError> {
    let document = library
        .lock()
        .map_err(|e| e.to_string())?
        .export_document(playlist_id)
        .map_err(CommandError::from_anyhow_chain)?;
    document.render(ExportFormat::Text).map_err(CommandError::from_anyhow_chain)
}
