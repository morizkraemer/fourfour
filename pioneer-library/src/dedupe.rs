
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hasher;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::duplicates::{DURATION_TOLERANCE_SECS, folded_tags_key};
use crate::{LocalLibrary, file_status};

const LOSSLESS_EXTENSIONS: [&str; 6] = ["flac", "wav", "wave", "aif", "aiff", "aifc"];
const READ_CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateConfidence {
    /// Every file in the group has the same bytes: copies of one file.
    ExactAudio,
    /// Same normalised artist and title with durations within [`DURATION_TOLERANCE_SECS`]; the audio was not compared.
    MetadataOnly,
}

/// What grouping needs to know about one library track.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateRow {
    pub id: i64,
    pub path: PathBuf,
    pub file_size: u64,
    pub artist: String,
    pub title: String,
    pub artist_key: String,
    pub title_key: String,
    pub duration_secs: f64,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub date_added: String,
    pub playlists: u32,
    pub cues: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateMember {
    pub id: i64,
    pub artist: String,
    pub title: String,
    pub path: String,
    pub format: String,
    pub lossless: bool,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub duration_secs: f64,
    pub file_size: u64,
    pub playlists: u32,
    pub cues: u32,
    pub date_added: String,
    pub file_present: bool,
}

/// One song with several entries. `members` is in keeper order: the proposed keeper first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub key: i64,
    pub confidence: DuplicateConfidence,
    pub keeper_id: i64,
    pub reason: String,
    pub members: Vec<DuplicateMember>,
}

pub fn is_lossless(path: &Path) -> bool {
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    LOSSLESS_EXTENSIONS.contains(&extension.as_str())
}

fn format_name(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default()
}

fn hash_file(path: &Path) -> Option<u64> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = std::hash::DefaultHasher::new();
    let mut buffer = vec![0u8; READ_CHUNK];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            return Some(hasher.finish());
        }
        hasher.write(&buffer[..read]);
    }
}

/// Whether both files can be read and hold the same bytes.
pub fn files_identical(a: &Path, b: &Path) -> bool {
    let (Ok(mut first), Ok(mut second)) = (std::fs::File::open(a), std::fs::File::open(b)) else { return false };
    let same_len = matches!((first.metadata(), second.metadata()), (Ok(x), Ok(y)) if x.len() == y.len());
    if !same_len {
        return false;
    }
    let (mut left, mut right) = (vec![0u8; READ_CHUNK], vec![0u8; READ_CHUNK]);
    loop {
        let Some(read) = read_full(&mut first, &mut left) else { return false };
        let Some(other) = read_full(&mut second, &mut right) else { return false };
        if read != other || left[..read] != right[..read] {
            return false;
        }
        if read == 0 {
            return true;
        }
    }
}

fn read_full(file: &mut std::fs::File, buffer: &mut [u8]) -> Option<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(_) => return None,
        }
    }
    Some(filled)
}

struct Components {
    parent: HashMap<i64, i64>,
}

impl Components {
    fn new(ids: impl Iterator<Item = i64>) -> Self {
        Self { parent: ids.map(|id| (id, id)).collect() }
    }

    fn root(&mut self, id: i64) -> i64 {
        let parent = self.parent[&id];
        if parent == id {
            return id;
        }
        let root = self.root(parent);
        self.parent.insert(id, root);
        root
    }

    fn join(&mut self, a: i64, b: i64) {
        let (a, b) = (self.root(a), self.root(b));
        if a != b {
            self.parent.insert(a.max(b), a.min(b));
        }
    }

    fn groups(&mut self) -> BTreeMap<i64, Vec<i64>> {
        let ids: Vec<i64> = self.parent.keys().copied().collect();
        let mut groups: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        for id in ids {
            groups.entry(self.root(id)).or_default().push(id);
        }
        groups
    }
}

fn exact_classes(rows: &[DuplicateRow]) -> HashMap<i64, i64> {
    let mut by_size: HashMap<u64, Vec<&DuplicateRow>> = HashMap::new();
    for row in rows.iter().filter(|r| r.file_size > 0) {
        by_size.entry(row.file_size).or_default().push(row);
    }
    let mut class_of = HashMap::new();
    for same_size in by_size.values().filter(|rows| rows.len() > 1) {
        let mut by_hash: HashMap<u64, Vec<i64>> = HashMap::new();
        for row in same_size {
            if let Some(hash) = hash_file(&row.path) {
                by_hash.entry(hash).or_default().push(row.id);
            }
        }
        for ids in by_hash.values().filter(|ids| ids.len() > 1) {
            let lowest = *ids.iter().min().expect("a class has members");
            class_of.extend(ids.iter().map(|id| (*id, lowest)));
        }
    }
    class_of
}

fn same_tags_clusters(rows: &[DuplicateRow]) -> Vec<Vec<i64>> {
    let mut by_tags: HashMap<(String, String), Vec<&DuplicateRow>> = HashMap::new();
    for row in rows {
        if let Some(key) = folded_tags_key(row.artist_key.clone(), row.title_key.clone()) {
            by_tags.entry(key).or_default().push(row);
        }
    }
    let mut clusters = Vec::new();
    for mut same_tags in by_tags.into_values().filter(|rows| rows.len() > 1) {
        same_tags.retain(|row| row.duration_secs > 0.0);
        same_tags.sort_by(|a, b| a.duration_secs.total_cmp(&b.duration_secs).then(a.id.cmp(&b.id)));
        let mut anchor = f64::NEG_INFINITY;
        for row in same_tags {
            if row.duration_secs - anchor > DURATION_TOLERANCE_SECS {
                anchor = row.duration_secs;
                clusters.push(Vec::new());
            }
            clusters.last_mut().expect("a cluster was opened").push(row.id);
        }
    }
    clusters.retain(|cluster| cluster.len() > 1);
    clusters
}

fn member_of(row: &DuplicateRow) -> DuplicateMember {
    DuplicateMember {
        id: row.id,
        artist: row.artist.clone(),
        title: row.title.clone(),
        path: row.path.to_string_lossy().into_owned(),
        format: format_name(&row.path),
        lossless: is_lossless(&row.path),
        bitrate: row.bitrate,
        sample_rate: row.sample_rate,
        duration_secs: row.duration_secs,
        file_size: row.file_size,
        playlists: row.playlists,
        cues: row.cues,
        date_added: row.date_added.clone(),
        file_present: file_status(&row.path) == crate::FileStatus::Ok,
    }
}

fn keeper_rank(member: &DuplicateMember) -> impl Ord + use<> {
    (
        !member.file_present,
        !member.lossless,
        Reverse(member.bitrate),
        Reverse(member.playlists),
        Reverse(member.cues),
        member.date_added.is_empty(),
        member.date_added.clone(),
        member.id,
    )
}

fn keeper_reason(keeper: &DuplicateMember, runner_up: &DuplicateMember) -> String {
    if keeper.file_present && !runner_up.file_present {
        "Its file is there; the other copy's file is missing".into()
    } else if keeper.lossless && !runner_up.lossless {
        format!("Lossless ({}) over lossy ({})", keeper.format, runner_up.format)
    } else if keeper.bitrate != runner_up.bitrate {
        format!("Higher bitrate ({} vs {} kbps)", keeper.bitrate, runner_up.bitrate)
    } else if keeper.playlists != runner_up.playlists {
        format!("In more playlists ({} vs {})", keeper.playlists, runner_up.playlists)
    } else if keeper.cues != runner_up.cues {
        format!("More cues ({} vs {})", keeper.cues, runner_up.cues)
    } else if keeper.date_added != runner_up.date_added {
        "Added to the library first".into()
    } else {
        "Same quality everywhere; the first entry".into()
    }
}

/// Groups `rows` into duplicate groups, ordered by their lowest track id. Files with the same size are compared by
/// content, so this reads them.
pub fn find_duplicate_groups(rows: &[DuplicateRow]) -> Vec<DuplicateGroup> {
    let exact = exact_classes(rows);
    let mut components = Components::new(rows.iter().map(|r| r.id));
    for (id, class) in &exact {
        components.join(*id, *class);
    }
    for cluster in same_tags_clusters(rows) {
        for id in &cluster[1..] {
            components.join(cluster[0], *id);
        }
    }
    let by_id: HashMap<i64, &DuplicateRow> = rows.iter().map(|r| (r.id, r)).collect();
    components
        .groups()
        .into_values()
        .filter(|ids| ids.len() > 1)
        .map(|ids| {
            let classes: HashSet<Option<i64>> = ids.iter().map(|id| exact.get(id).copied()).collect();
            let all_one_exact_class = classes.len() == 1 && !classes.contains(&None);
            let mut members: Vec<DuplicateMember> = ids.iter().map(|id| member_of(by_id[id])).collect();
            members.sort_by_cached_key(keeper_rank);
            DuplicateGroup {
                key: ids.iter().copied().min().expect("a group has members"),
                confidence: if all_one_exact_class { DuplicateConfidence::ExactAudio } else { DuplicateConfidence::MetadataOnly },
                keeper_id: members[0].id,
                reason: keeper_reason(&members[0], &members[1]),
                members,
            }
        })
        .collect()
}

impl LocalLibrary {
    /// The tracks that could be part of a duplicate group (they share a file size or normalised artist and title with
    /// another track), with their playlist and cue counts. Cheap: no audio file is read.
    pub fn duplicate_rows(&self) -> Result<Vec<DuplicateRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.source_path, t.file_size, t.artist, t.title, t.artist_key, t.title_key, t.duration_secs,
                    t.bitrate, t.sample_rate, t.date_added,
                    (SELECT COUNT(*) FROM playlist_tracks p WHERE p.track_id = t.id)
             FROM tracks t ORDER BY t.id",
        )?;
        let mut rows: Vec<DuplicateRow> = stmt
            .query_map([], |row| {
                Ok(DuplicateRow {
                    id: row.get(0)?,
                    path: PathBuf::from(row.get::<_, String>(1)?),
                    file_size: row.get::<_, i64>(2)? as u64,
                    artist: row.get(3)?,
                    title: row.get(4)?,
                    artist_key: row.get(5)?,
                    title_key: row.get(6)?,
                    duration_secs: row.get(7)?,
                    bitrate: row.get(8)?,
                    sample_rate: row.get(9)?,
                    date_added: row.get(10)?,
                    playlists: row.get(11)?,
                    cues: 0,
                })
            })?
            .collect::<Result<_, _>>()?;
        let mut sizes: HashMap<u64, u32> = HashMap::new();
        let mut tags: HashMap<(String, String), u32> = HashMap::new();
        for row in &rows {
            if row.file_size > 0 {
                *sizes.entry(row.file_size).or_default() += 1;
            }
            if let Some(key) = folded_tags_key(row.artist_key.clone(), row.title_key.clone()) {
                *tags.entry(key).or_default() += 1;
            }
        }
        rows.retain(|row| {
            sizes.get(&row.file_size).is_some_and(|n| *n > 1 && row.file_size > 0)
                || folded_tags_key(row.artist_key.clone(), row.title_key.clone()).is_some_and(|key| tags[&key] > 1)
        });
        for row in &mut rows {
            row.cues = self.chosen_cues(row.id).map_or(0, |cues| cues.len() as u32);
        }
        Ok(rows)
    }
}
