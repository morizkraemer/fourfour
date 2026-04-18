//! Sync diffing logic for incremental USB updates.
//!
//! Compares the caller's desired track list against the existing USB state
//! and produces a [`SyncPlan`] describing what needs to change.

use std::collections::HashMap;

use crate::models::{
    AnalysisResult, ExistingTrack, ExistingUsbState, Playlist, SyncAction, SyncEntry, SyncPlan,
    SyncReport, Track,
};

/// Compute a sync plan by diffing the caller's tracks against the existing USB state.
///
/// For each track, determines whether it needs to be added, replaced, updated,
/// or skipped. Tracks on the USB that are not in the caller's list are marked
/// for removal.
///
/// Playlist track IDs are remapped from the caller's ID space to stable USB IDs.
pub fn compute_sync_plan<'a>(
    tracks: &'a [Track],
    analyses: &'a [AnalysisResult],
    playlists: &[Playlist],
    existing: Option<&ExistingUsbState>,
) -> SyncPlan<'a> {
    // Build lookup from usb_path -> ExistingTrack for the current USB state.
    let mut existing_map: HashMap<&str, ExistingTrack> = match existing {
        Some(state) => state
            .tracks
            .iter()
            .map(|t| (t.usb_path.as_str(), t.clone()))
            .collect(),
        None => HashMap::new(),
    };

    let mut next_id: u32 = existing.map_or(1, |s| s.next_track_id);
    let mut entries: Vec<SyncEntry<'a>> = Vec::with_capacity(tracks.len());
    let mut id_remap: HashMap<u32, u32> = HashMap::with_capacity(tracks.len());

    for (track, analysis) in tracks.iter().zip(analyses.iter()) {
        let (action, usb_id) = if let Some(existing_track) = existing_map.remove(track.usb_path.as_str()) {
            if track.file_size != existing_track.file_size {
                (SyncAction::Replace, existing_track.id)
            } else if track_metadata_changed(track, &existing_track) {
                (SyncAction::Update, existing_track.id)
            } else {
                (SyncAction::Skip, existing_track.id)
            }
        } else {
            let id = next_id;
            next_id += 1;
            (SyncAction::Add, id)
        };

        id_remap.insert(track.id, usb_id);

        entries.push(SyncEntry {
            track,
            analysis,
            action,
            usb_id,
        });
    }

    // Remaining entries in the map are tracks on USB not in the caller's list.
    let removals: Vec<ExistingTrack> = existing_map.into_values().collect();

    // Remap playlist track_ids, filtering out any that aren't in the remap.
    let remapped_playlists: Vec<Playlist> = playlists
        .iter()
        .map(|pl| Playlist {
            id: pl.id,
            name: pl.name.clone(),
            track_ids: pl
                .track_ids
                .iter()
                .filter_map(|tid| id_remap.get(tid).copied())
                .collect(),
        })
        .collect();

    SyncPlan {
        entries,
        removals,
        id_remap,
        playlists: remapped_playlists,
    }
}

/// Check whether any metadata field differs between the caller's track and
/// the existing USB track.
fn track_metadata_changed(track: &Track, existing: &ExistingTrack) -> bool {
    track.title != existing.title
        || track.artist != existing.artist
        || track.album != existing.album
        || track.genre != existing.genre
        || track.label != existing.label
        || track.remixer != existing.remixer
        || track.comment != existing.comment
        || track.key != existing.key
        || track.year != existing.year
        || track.track_number != existing.track_number
        || track.disc_number != existing.disc_number
        || track.tempo != existing.tempo
        || track.sample_rate != existing.sample_rate
        || track.bitrate != existing.bitrate
        || (track.duration_secs.round() as i64 != existing.duration_secs.round() as i64)
        || (track.artwork.is_some() != existing.has_artwork)
}

/// Build a summary report from a computed sync plan.
pub fn build_sync_report(plan: &SyncPlan) -> SyncReport {
    let mut added: u32 = 0;
    let mut updated: u32 = 0;
    let mut replaced: u32 = 0;
    let mut unchanged: u32 = 0;

    for entry in &plan.entries {
        match entry.action {
            SyncAction::Add => added += 1,
            SyncAction::Update => updated += 1,
            SyncAction::Replace => replaced += 1,
            SyncAction::Skip => unchanged += 1,
        }
    }

    SyncReport {
        tracks_added: added,
        tracks_updated: updated,
        tracks_replaced: replaced,
        tracks_removed: plan.removals.len() as u32,
        tracks_unchanged: unchanged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BeatGrid, WaveformPreview};
    use std::path::PathBuf;

    fn make_track(id: u32, usb_path: &str, title: &str, file_size: u64) -> Track {
        Track {
            source_path: PathBuf::from("/tmp/test.mp3"),
            usb_path: usb_path.to_string(),
            title: title.to_string(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            label: String::new(),
            remixer: String::new(),
            comment: String::new(),
            year: 0,
            disc_number: 0,
            track_number: 0,
            tempo: 12800,
            key: "1A".to_string(),
            duration_secs: 300.0,
            sample_rate: 44100,
            bitrate: 320,
            file_size,
            id,
            artwork: None,
        }
    }

    fn make_existing(id: u32, usb_path: &str, title: &str, file_size: u64) -> ExistingTrack {
        ExistingTrack {
            id,
            usb_path: usb_path.to_string(),
            title: title.to_string(),
            artist: String::new(),
            remixer: String::new(),
            album: String::new(),
            genre: String::new(),
            label: String::new(),
            key: "1A".to_string(),
            comment: String::new(),
            year: 0,
            track_number: 0,
            disc_number: 0,
            tempo: 12800,
            duration_secs: 300.0,
            sample_rate: 44100,
            bitrate: 320,
            file_size,
            has_artwork: false,
        }
    }

    fn make_analysis() -> AnalysisResult {
        AnalysisResult {
            beat_grid: BeatGrid { beats: vec![] },
            waveform: WaveformPreview { data: [0u8; 400] },
            bpm: 128.0,
            key: "1A".to_string(),
            cue_points: vec![],
        }
    }

    #[test]
    fn fresh_usb_all_adds() {
        let tracks = vec![
            make_track(1, "/Contents/A/t1.mp3", "Track 1", 1000),
            make_track(2, "/Contents/A/t2.mp3", "Track 2", 2000),
        ];
        let analyses = vec![make_analysis(), make_analysis()];
        let playlists = vec![];

        let plan = compute_sync_plan(&tracks, &analyses, &playlists, None);

        assert_eq!(plan.entries.len(), 2);
        assert_eq!(plan.entries[0].action, SyncAction::Add);
        assert_eq!(plan.entries[0].usb_id, 1);
        assert_eq!(plan.entries[1].action, SyncAction::Add);
        assert_eq!(plan.entries[1].usb_id, 2);
        assert!(plan.removals.is_empty());
    }

    #[test]
    fn skip_unchanged_tracks() {
        let tracks = vec![make_track(1, "/Contents/A/t1.mp3", "Track 1", 1000)];
        let analyses = vec![make_analysis()];
        let existing = ExistingUsbState {
            tracks: vec![make_existing(5, "/Contents/A/t1.mp3", "Track 1", 1000)],
            playlists: vec![],
            next_track_id: 6,
            next_playlist_id: 1,
        };

        let plan = compute_sync_plan(&tracks, &analyses, &[], Some(&existing));

        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].action, SyncAction::Skip);
        assert_eq!(plan.entries[0].usb_id, 5); // preserves existing ID
        assert!(plan.removals.is_empty());
    }

    #[test]
    fn replace_when_file_size_differs() {
        let tracks = vec![make_track(1, "/Contents/A/t1.mp3", "Track 1", 2000)];
        let analyses = vec![make_analysis()];
        let existing = ExistingUsbState {
            tracks: vec![make_existing(5, "/Contents/A/t1.mp3", "Track 1", 1000)],
            playlists: vec![],
            next_track_id: 6,
            next_playlist_id: 1,
        };

        let plan = compute_sync_plan(&tracks, &analyses, &[], Some(&existing));

        assert_eq!(plan.entries[0].action, SyncAction::Replace);
        assert_eq!(plan.entries[0].usb_id, 5);
    }

    #[test]
    fn update_when_metadata_changed() {
        let tracks = vec![make_track(1, "/Contents/A/t1.mp3", "New Title", 1000)];
        let analyses = vec![make_analysis()];
        let existing = ExistingUsbState {
            tracks: vec![make_existing(5, "/Contents/A/t1.mp3", "Old Title", 1000)],
            playlists: vec![],
            next_track_id: 6,
            next_playlist_id: 1,
        };

        let plan = compute_sync_plan(&tracks, &analyses, &[], Some(&existing));

        assert_eq!(plan.entries[0].action, SyncAction::Update);
        assert_eq!(plan.entries[0].usb_id, 5);
    }

    #[test]
    fn removals_for_missing_tracks() {
        let tracks = vec![make_track(1, "/Contents/A/t1.mp3", "Track 1", 1000)];
        let analyses = vec![make_analysis()];
        let existing = ExistingUsbState {
            tracks: vec![
                make_existing(5, "/Contents/A/t1.mp3", "Track 1", 1000),
                make_existing(6, "/Contents/A/t2.mp3", "Track 2", 2000),
            ],
            playlists: vec![],
            next_track_id: 7,
            next_playlist_id: 1,
        };

        let plan = compute_sync_plan(&tracks, &analyses, &[], Some(&existing));

        assert_eq!(plan.removals.len(), 1);
        assert_eq!(plan.removals[0].id, 6);
    }

    #[test]
    fn playlist_ids_remapped() {
        let tracks = vec![
            make_track(100, "/Contents/A/t1.mp3", "Track 1", 1000),
            make_track(200, "/Contents/A/t2.mp3", "Track 2", 2000),
        ];
        let analyses = vec![make_analysis(), make_analysis()];
        let playlists = vec![Playlist {
            id: 1,
            name: "My Playlist".to_string(),
            track_ids: vec![100, 200, 999], // 999 does not exist — should be filtered
        }];
        let existing = ExistingUsbState {
            tracks: vec![make_existing(5, "/Contents/A/t1.mp3", "Track 1", 1000)],
            playlists: vec![],
            next_track_id: 6,
            next_playlist_id: 2,
        };

        let plan = compute_sync_plan(&tracks, &analyses, &playlists, Some(&existing));

        assert_eq!(plan.playlists.len(), 1);
        // track 100 -> usb_id 5 (existing), track 200 -> usb_id 6 (new), 999 filtered out
        assert_eq!(plan.playlists[0].track_ids, vec![5, 6]);
    }

    #[test]
    fn build_report_counts() {
        let tracks = vec![
            make_track(1, "/Contents/A/t1.mp3", "Track 1", 1000),
            make_track(2, "/Contents/A/t2.mp3", "New Title", 2000),
            make_track(3, "/Contents/A/t3.mp3", "Track 3", 9999),
            make_track(4, "/Contents/A/t4.mp3", "Track 4", 4000),
        ];
        let analyses = vec![make_analysis(), make_analysis(), make_analysis(), make_analysis()];
        let existing = ExistingUsbState {
            tracks: vec![
                make_existing(1, "/Contents/A/t1.mp3", "Track 1", 1000), // Skip
                make_existing(2, "/Contents/A/t2.mp3", "Old Title", 2000), // Update
                make_existing(3, "/Contents/A/t3.mp3", "Track 3", 3000), // Replace (size differs)
                make_existing(5, "/Contents/A/gone.mp3", "Gone", 5000),  // Removal
            ],
            playlists: vec![],
            next_track_id: 6,
            next_playlist_id: 1,
        };

        let plan = compute_sync_plan(&tracks, &analyses, &[], Some(&existing));
        let report = build_sync_report(&plan);

        assert_eq!(report.tracks_added, 1);    // t4
        assert_eq!(report.tracks_unchanged, 1); // t1
        assert_eq!(report.tracks_updated, 1);   // t2
        assert_eq!(report.tracks_replaced, 1);  // t3
        assert_eq!(report.tracks_removed, 1);   // gone.mp3
    }
}
