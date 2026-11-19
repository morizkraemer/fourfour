use std::borrow::Cow;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::bt::{BeatEngine, BeatMode, BeatResult, BeatWindows, MODEL_RATE};
use crate::cyankey;
use crate::decode::{decode_mono_measured, resample_to};
use crate::energy::{self, Energy};
use crate::fit::{self, FitParams, Window, WindowCheck, FPS};
use crate::keymap::camelot;
use crate::loudness::Loudness;
use crate::mp3::{self, DelayRule};
use crate::service;
use crate::waveform::{self, Waveforms};

#[derive(Clone)]
pub struct Config {
    pub threads: usize,
    pub fit: FitParams,
    pub mp3_rule: DelayRule,
    pub beats: bool,
    pub key: bool,
    pub waveform: bool,
    pub energy: bool,
    pub keep_logits: bool,
    pub loudness: bool,
    pub beat_windows: Option<BeatWindows>,
}

impl Default for Config {
    fn default() -> Self {
        Config { threads: 1, fit: FitParams::default(), mp3_rule: DelayRule::ModelLead, beats: true, key: true, waveform: true, energy: false, keep_logits: false, loudness: false, beat_windows: BeatWindows::from_env() }
    }
}

pub struct Analysis {
    pub duration_s: f64,
    pub truncated: bool,
    pub bpm: Option<f64>,
    pub first_beat_s: Option<f64>,
    pub beats_s: Vec<f64>,
    pub raw_beats_s: Vec<f64>,
    pub downbeats_s: Vec<f32>,
    pub key_class: Option<usize>,
    pub key_confidence: f64,
    pub waveforms: Option<Waveforms>,
    pub waveform_checksum: Option<u64>,
    pub detail_entries: usize,
    pub mp3_delay_s: f64,
    pub energy: Option<Energy>,
    pub logits: Option<(Vec<f32>, Vec<f32>)>,
    pub loudness: Option<Loudness>,
    pub beat_mode: BeatMode,
    pub window_checks: Vec<WindowCheck>,
    pub ms: Value,
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "analysis cancelled")
    }
}

impl std::error::Error for Cancelled {}

static NEVER_PAUSED: AtomicBool = AtomicBool::new(false);

pub struct Control<'a> {
    pub cancel: &'a AtomicBool,
    pub pause: &'a AtomicBool,
    /// Called once the waveform (and, when the pass detects it, the key class) is known, before the beat model ends.
    pub on_waveform: Option<&'a (dyn Fn(&Waveforms, Option<usize>) + Sync)>,
}

struct KeyPart {
    key: Option<cyankey::CyanKey>,
    ms: f64,
    resample_ms: f64,
}

impl<'a> Control<'a> {
    pub fn new(cancel: &'a AtomicBool) -> Self {
        Control { cancel, pause: &NEVER_PAUSED, on_waveform: None }
    }
}

pub fn analyze(engine: &BeatEngine, path: &Path, cfg: &Config) -> Result<Analysis> {
    analyze_cancellable(engine, path, cfg, &AtomicBool::new(false))
}

pub fn analyze_cancellable(engine: &BeatEngine, path: &Path, cfg: &Config, cancel: &AtomicBool) -> Result<Analysis> {
    analyze_controlled(engine, path, cfg, &Control::new(cancel))
}

pub fn analyze_controlled(engine: &BeatEngine, path: &Path, cfg: &Config, control: &Control) -> Result<Analysis> {
    let cancel = control.cancel;
    let total = Instant::now();
    let t = Instant::now();
    let native = decode_mono_measured(path, cfg.loudness)?;
    let decode_ms = ms(t);
    if native.samples.is_empty() {
        return Err(anyhow!("no audio decoded"));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(Cancelled.into());
    }
    let rate = native.rate;
    let truncated = native.truncated;
    let loudness = native.loudness;
    let loudness_ms = native.loudness_ms;
    let duration_s = native.samples.len() as f64 / rate as f64;
    let gapless_s = native.gapless.len() as f64 / rate as f64;
    let gapless_start_s = native.gapless.start as f64 / rate as f64;
    let native = Arc::new(native);
    let side_threads = if cfg.threads > 1 { 2 } else { 1 };
    let (beat_part, key_part, wf_part, energy_part) = std::thread::scope(|s| {
        let energy_thread = cfg.energy.then(|| {
            let native = native.clone();
            s.spawn(move || {
                let t = Instant::now();
                let energy = energy::analyze(native.gapless_samples(), rate);
                (energy, ms(t))
            })
        });
        let key = cfg.key.then(|| {
            let native = native.clone();
            s.spawn(move || -> Result<KeyPart> {
                let t = Instant::now();
                let gapless = native.gapless_samples();
                let mono44: Cow<[f32]> = match rate {
                    44100 => Cow::Borrowed(gapless),
                    rate => Cow::Owned(resample_to(gapless, rate, 44100)?),
                };
                let resample_ms = ms(t);
                let t = Instant::now();
                let key = cyankey::detect(&mono44, 44100, side_threads);
                Ok(KeyPart { key, ms: ms(t), resample_ms })
            })
        });
        let wf_audio = native.clone();
        let fast = s.spawn(move || -> (Option<Result<KeyPart>>, Option<(Waveforms, f64)>) {
            let wf = cfg.waveform.then(|| {
                let t = Instant::now();
                let w = waveform::compute(&wf_audio.samples, rate);
                (w, ms(t))
            });
            drop(wf_audio);
            let key = key.map(|h| h.join().unwrap());
            if let (Some((w, _)), Some(on_waveform)) = (&wf, control.on_waveform) {
                if !cancel.load(Ordering::Relaxed) {
                    let class = key.as_ref().and_then(|k| k.as_ref().ok()).and_then(|k| k.key.as_ref()).map(|k| k.class);
                    on_waveform(w, class);
                }
            }
            (key, wf)
        });
        let beat = (|| -> Result<(BeatResult, Option<fit::Fit>, f64, f64, f64, BeatMode, Vec<WindowCheck>)> {
            if !cfg.beats {
                let none = BeatResult {
                    beat_logits: Vec::new(),
                    down_logits: Vec::new(),
                    beats: Vec::new(),
                    downbeats: Vec::new(),
                    resample_ms: 0.0,
                    mel_ms: 0.0,
                    predict_ms: 0.0,
                    modeled: Vec::new(),
                };
                return Ok((none, None, 0.0, 0.0, 0.0, BeatMode::Full, Vec::new()));
            }
            let mut fp = cfg.fit.clone();
            fp.threads = cfg.threads;
            let fit_of = |bt: &BeatResult| {
                let w = Window { offset: 0.0, len: bt.beat_logits.len() as f64 / FPS };
                let peaks: Vec<f64> = bt.beats.iter().map(|&b| b as f64).collect();
                fit::fit_track(&peaks, &bt.beat_logits, &bt.down_logits, gapless_s, &w, &fp)
            };
            let resample = move || resample_to(native.gapless_samples(), rate, MODEL_RATE);
            let mut checks = Vec::new();
            let mut window_fit_ms = 0.0;
            let (bt, mode, accepted) = match &cfg.beat_windows {
                None => (engine.analyze_resampled(resample, cfg.threads, control)?, BeatMode::Full, None),
                Some(windows) => engine.analyze_windowed(resample, cfg.threads, control, windows, |bt| {
                    let t = Instant::now();
                    let fit = fit_of(bt);
                    if let Some(f) = &fit {
                        let peaks: Vec<f64> = bt.beats.iter().map(|&b| b as f64).collect();
                        checks = fit::check_windows(f, &peaks, &bt.beat_logits, &bt.modeled);
                    }
                    window_fit_ms = ms(t);
                    fit.filter(|_| fit::windows_agree(&checks))
                })?,
            };
            let t = Instant::now();
            let delay = mp3::timeline_shift_s(path, cfg.mp3_rule) + if cfg.mp3_rule == DelayRule::ModelLead { gapless_start_s } else { 0.0 };
            let mp3_ms = ms(t);
            let t = Instant::now();
            let fit = accepted.or_else(|| fit_of(&bt));
            Ok((bt, fit, delay, window_fit_ms + ms(t), mp3_ms, mode, checks))
        })();
        let (key, wf) = fast.join().unwrap();
        (beat, key, wf, energy_thread.map(|h| h.join().unwrap()))
    });
    let (bt, fit, delay, fit_ms, mp3_ms, beat_mode, window_checks) = beat_part?;
    if cancel.load(Ordering::Relaxed) {
        return Err(Cancelled.into());
    }

    let (beats_s, bpm, first_beat_s, raw_beats_s) = match &fit {
        Some(f) => {
            let b = fit::finalize_beats(&f.beats_s, delay);
            let first = b.first().copied();
            (b, Some(f.bpm), first, f.raw_beats_s.clone())
        }
        None => (Vec::new(), None, None, bt.beats.iter().map(|&b| b as f64).collect()),
    };
    let key_part = key_part.transpose()?;
    let resample44_ms = key_part.as_ref().map_or(0.0, |k| k.resample_ms);
    let (key_class, key_confidence, key_ms) = match &key_part {
        Some(KeyPart { key: Some(k), ms, .. }) => (Some(k.class), k.confidence, *ms),
        Some(KeyPart { key: None, ms, .. }) => (None, 0.0, *ms),
        None => (None, 0.0, 0.0),
    };
    let (waveforms, wf_ms) = match wf_part {
        Some((w, t)) => (Some(w), t),
        None => (None, 0.0),
    };
    let (energy, energy_ms) = match energy_part {
        Some((energy, energy_ms)) => (energy, Some(energy_ms)),
        None => (None, None),
    };
    let waveform_checksum = waveforms.as_ref().map(waveform::checksum);
    let detail_entries = waveforms.as_ref().map_or(0, |w| w.pwv7.len());
    let mut ms_doc = json!({
        "decode": decode_ms,
        "resample": bt.resample_ms + resample44_ms,
        "mel": bt.mel_ms,
        "predict": bt.predict_ms,
        "model": bt.resample_ms + bt.mel_ms + bt.predict_ms,
        "fit": fit_ms,
        "mp3": mp3_ms,
        "key": key_ms,
        "waveform": wf_ms,
        "total": ms(total),
    });
    if cfg.loudness {
        ms_doc["loudness"] = json!(loudness_ms);
    }
    if let Some(energy_ms) = energy_ms {
        ms_doc["energy"] = json!(energy_ms);
    }
    Ok(Analysis {
        duration_s,
        truncated,
        bpm,
        first_beat_s,
        beats_s,
        raw_beats_s,
        downbeats_s: bt.downbeats.clone(),
        key_class,
        key_confidence,
        waveforms,
        waveform_checksum,
        detail_entries,
        mp3_delay_s: delay,
        energy,
        logits: cfg.keep_logits.then(|| (bt.beat_logits, bt.down_logits)),
        loudness,
        beat_mode,
        window_checks,
        ms: ms_doc,
    })
}

impl Analysis {
    pub fn to_json(&self, path: &Path, id: Option<u64>) -> Value {
        let mut doc = json!({
            "id": id,
            "path": path,
            "duration_s": self.duration_s,
            "truncated": self.truncated,
            "bpm": self.bpm,
            "first_beat_s": self.first_beat_s,
            "n_beats": self.beats_s.len(),
            "final_beats_s": self.beats_s,
            "downbeats_s": self.downbeats_s,
            "bar_phase": service::bar_phase(&self.beats_s, &self.downbeats_s, self.mp3_delay_s),
            "bar_phase_model_vote": service::bar_phase_by(service::BarOneRule::ModelVote, &self.beats_s, &self.downbeats_s, self.mp3_delay_s),
            "key": self.key_class.map(camelot),
            "key_confidence": self.key_confidence,
            "mp3_delay_ms": self.mp3_delay_s * 1000.0,
            "waveform_checksum": self.waveform_checksum,
            "detail_entries": self.detail_entries,
            "ms": self.ms,
        });
        if self.beat_mode != BeatMode::Full || !self.window_checks.is_empty() {
            doc["beat_mode"] = json!(self.beat_mode.name());
            doc["window_checks"] = json!(self
                .window_checks
                .iter()
                .map(|c| json!({ "from_s": c.from_s, "to_s": c.to_s, "expected": c.expected, "peaks": c.peaks, "inliers": c.inliers, "median_ms": c.median_ms }))
                .collect::<Vec<_>>());
        }
        if let Some(l) = &self.loudness {
            doc["loudness"] = json!({ "lufs": l.lufs, "peak_db": l.peak_db, "gain_db": l.gain_db });
        }
        if let Some(energy) = &self.energy {
            doc["energy"] = energy.to_json();
        }
        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analysis(loudness: Option<Loudness>) -> Analysis {
        Analysis {
            duration_s: 1.0,
            truncated: false,
            bpm: None,
            first_beat_s: None,
            beats_s: Vec::new(),
            raw_beats_s: Vec::new(),
            downbeats_s: Vec::new(),
            key_class: None,
            key_confidence: 0.0,
            waveforms: None,
            waveform_checksum: None,
            detail_entries: 0,
            mp3_delay_s: 0.0,
            energy: None,
            logits: None,
            loudness,
            beat_mode: BeatMode::Full,
            window_checks: Vec::new(),
            ms: Value::Null,
        }
    }

    #[test]
    fn loudness_is_off_by_default_and_absent_from_the_json_unless_measured() {
        assert!(!Config::default().loudness);
        let absent = analysis(None).to_json(Path::new("a.wav"), None);
        assert!(absent.get("loudness").is_none());
        let present = analysis(Some(Loudness { lufs: -9.5, peak_db: -0.3, gain_db: -4.5 })).to_json(Path::new("a.wav"), None);
        assert_eq!(present["loudness"], json!({ "lufs": -9.5, "peak_db": -0.3, "gain_db": -4.5 }));
    }
}
