use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const STAGE_DIR: &str = ".fourfour-staging";
pub const PREVIOUS_DIR: &str = ".fourfour-previous";
pub const JOURNAL_FILE: &str = ".fourfour-publish.json";
const JOURNAL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Journal {
    version: u32,
    moves: Vec<String>,
    deletes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    WriteJournal,
    Move(String),
    Delete(String),
    RemoveJournal,
    CleanUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    NothingToDo,
    DiscardedUnfinishedStaging,
    FinishedInterruptedPublish,
}

pub fn stage_dir(root: &Path) -> PathBuf {
    root.join(STAGE_DIR)
}

fn previous_dir(root: &Path) -> PathBuf {
    root.join(PREVIOUS_DIR)
}

fn journal_path(root: &Path) -> PathBuf {
    root.join(JOURNAL_FILE)
}

fn sync_file(path: &Path) {
    if let Ok(file) = std::fs::File::open(path) {
        let _ = file.sync_all();
    }
}

fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        sync_file(parent);
    }
}

fn remove_tree(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("Cannot remove {}", path.display())),
    }
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) -> Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot list {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, base, out)?;
        } else {
            out.push(path.strip_prefix(base).unwrap_or(&path).to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

pub fn staged_files(root: &Path) -> Result<Vec<String>> {
    let stage = stage_dir(root);
    let mut files = Vec::new();
    if stage.is_dir() {
        walk(&stage, &stage, &mut files)?;
    }
    Ok(files)
}

pub fn pending(root: &Path) -> bool {
    journal_path(root).exists() || stage_dir(root).exists() || previous_dir(root).exists()
}

pub fn begin(root: &Path) -> Result<PathBuf> {
    anyhow::ensure!(
        !pending(root),
        "An earlier sync of {} was interrupted. It has to be repaired before a new one starts.",
        root.display()
    );
    let stage = stage_dir(root);
    std::fs::create_dir_all(&stage).with_context(|| format!("Cannot create {}", stage.display()))?;
    Ok(stage)
}

pub fn abandon(root: &Path) -> Result<()> {
    anyhow::ensure!(!journal_path(root).exists(), "The sync of {} is already being published", root.display());
    remove_tree(&stage_dir(root))
}

fn set_aside(root: &Path, relative: &str) -> Result<()> {
    let target = root.join(relative);
    if !target.exists() {
        return Ok(());
    }
    let kept = previous_dir(root).join(relative);
    if kept.exists() {
        return std::fs::remove_file(&target).with_context(|| format!("Cannot remove {}", target.display()));
    }
    if let Some(parent) = kept.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("Cannot create {}", parent.display()))?;
    }
    std::fs::rename(&target, &kept).with_context(|| format!("Cannot move {} aside", target.display()))
}

fn remove_empty_parents(root: &Path, relative: &str) {
    let mut dir = root.join(relative);
    while dir.pop() && dir != root && dir.starts_with(root) {
        if std::fs::remove_dir(&dir).is_err() {
            break;
        }
    }
}

fn roll_forward(root: &Path, journal: &Journal, checkpoint: &mut dyn FnMut(&Step) -> Result<()>) -> Result<()> {
    let stage = stage_dir(root);
    for relative in &journal.moves {
        checkpoint(&Step::Move(relative.clone()))?;
        let staged = stage.join(relative);
        if !staged.exists() {
            continue;
        }
        set_aside(root, relative)?;
        let target = root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("Cannot create {}", parent.display()))?;
        }
        std::fs::rename(&staged, &target).with_context(|| format!("Cannot put {} in place", target.display()))?;
        sync_parent(&target);
    }
    for relative in &journal.deletes {
        checkpoint(&Step::Delete(relative.clone()))?;
        set_aside(root, relative)?;
    }
    sync_file(root);
    checkpoint(&Step::RemoveJournal)?;
    let journal_file = journal_path(root);
    std::fs::remove_file(&journal_file).with_context(|| format!("Cannot remove {}", journal_file.display()))?;
    sync_file(root);
    checkpoint(&Step::CleanUp)?;
    clean_up(root, &journal.deletes)
}

fn clean_up(root: &Path, deleted: &[String]) -> Result<()> {
    remove_tree(&previous_dir(root))?;
    remove_tree(&stage_dir(root))?;
    for relative in deleted {
        remove_empty_parents(root, relative);
    }
    Ok(())
}

pub fn commit(root: &Path, deletes: Vec<String>, checkpoint: &mut dyn FnMut(&Step) -> Result<()>) -> Result<()> {
    let moves = staged_files(root)?;
    for relative in &moves {
        sync_file(&stage_dir(root).join(relative));
    }
    let deletes: Vec<String> = deletes.into_iter().filter(|relative| !moves.contains(relative)).collect();
    let journal = Journal { version: JOURNAL_VERSION, moves, deletes };

    checkpoint(&Step::WriteJournal)?;
    let journal_file = journal_path(root);
    let unfinished = journal_file.with_extension("json.tmp");
    std::fs::write(&unfinished, serde_json::to_vec_pretty(&journal)?)
        .with_context(|| format!("Cannot write {}", unfinished.display()))?;
    sync_file(&unfinished);
    std::fs::rename(&unfinished, &journal_file).with_context(|| format!("Cannot write {}", journal_file.display()))?;
    sync_file(root);

    roll_forward(root, &journal, checkpoint)
}

pub fn recover(root: &Path) -> Result<Recovery> {
    let journal_file = journal_path(root);
    let _ = std::fs::remove_file(journal_file.with_extension("json.tmp"));
    if journal_file.exists() {
        let bytes = std::fs::read(&journal_file).with_context(|| format!("Cannot read {}", journal_file.display()))?;
        let journal: Journal = serde_json::from_slice(&bytes)
            .with_context(|| format!("{} is damaged; the interrupted sync cannot be finished", journal_file.display()))?;
        anyhow::ensure!(
            journal.version == JOURNAL_VERSION,
            "{} was written by a newer fourfour (version {})",
            journal_file.display(),
            journal.version
        );
        roll_forward(root, &journal, &mut |_| Ok(()))?;
        return Ok(Recovery::FinishedInterruptedPublish);
    }
    if stage_dir(root).exists() || previous_dir(root).exists() {
        clean_up(root, &[])?;
        return Ok(Recovery::DiscardedUnfinishedStaging);
    }
    Ok(Recovery::NothingToDo)
}
