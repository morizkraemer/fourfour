use std::borrow::Cow;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::bt::{BeatEngine, BeatResult};
use crate::cyankey;
use crate::decode::{decode_mono, resample_to};
use crate::fit::{self, FitParams, Window, FPS};
use crate::keymap::camelot;
use crate::mp3::{self, DelayRule};
use crate::waveform::{self, Waveforms};

#[derive(Clone)]
pub struct Config {
    pub threads: usize,
    pub fit: FitParams,
    pub mp3_rule: DelayRule,
    pub key: bool,
    pub waveform: bool,
    pub placeholder_waveform: bool,
    pub keep_logits: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { threads: 1, fit: FitParams::default(), mp3_rule: DelayRule::EncoderHeaderLame3100Early, key: true, waveform: true, placeholder_waveform: false, keep_logits: false }
    }
}

pub struct Analysis {
    pub duration_s: f64,
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
    pub logits: Option<(Vec<f32>, Vec<f32>)>,
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

pub fn analyze(engine: &BeatEngine, path: &Path, cfg: &Config) -> Result<Analysis> {
    analyze_cancellable(engine, path, cfg, &AtomicBool::new(false))
}

pub fn analyze_cancellable(engine: &BeatEngine, path: &Path, cfg: &Config, cancel: &AtomicBool) -> Result<Analysis> {
    let total = Instant::now();
    let t = Instant::now();
    let native = decode_mono(path)?;
    let decode_ms = ms(t);
    if native.samples.is_empty() {
        return Err(anyhow!("no audio decoded"));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(Cancelled.into());
    }
    let duration_s = native.samples.len() as f64 / native.rate as f64;
    let t = Instant::now();
    let mono44: Cow<[f32]> = if native.rate == 44100 {
        Cow::Borrowed(&native.samples)
    } else {
        Cow::Owned(resample_to(&native.samples, native.rate, 44100)?)
    };
    let resample44_ms = ms(t);
    let mono = &*mono44;

    let side_threads = if cfg.threads > 1 { 2 } else { 1 };
    let (beat_part, key_part, wf_part) = std::thread::scope(|s| {
        let key = cfg.key.then(|| {
            s.spawn(|| {
                let t = Instant::now();
                let r = cyankey::detect(mono, 44100, side_threads);
                (r, ms(t))
            })
        });
        let wf = cfg.waveform.then(|| {
            s.spawn(|| {
                let t = Instant::now();
                let w = if cfg.placeholder_waveform {
                    Ok(waveform::compute_placeholder(mono, 44100, side_threads))
                } else {
                    resample_to(&native.samples, native.rate, waveform::TARGET_SR).map(|a| waveform::compute(&a))
                };
                (w, ms(t))
            })
        });
        let beat = (|| -> Result<(BeatResult, Option<fit::Fit>, f64, f64, f64)> {
            let bt = engine.analyze(&native.samples, native.rate, cfg.threads)?;
            let t = Instant::now();
            let delay = mp3::timeline_shift_s(path, cfg.mp3_rule);
            let mp3_ms = ms(t);
            let t = Instant::now();
            let w = Window { offset: 0.0, len: bt.beat_logits.len() as f64 / FPS };
            let peaks: Vec<f64> = bt.beats.iter().map(|&b| b as f64).collect();
            let mut fp = cfg.fit.clone();
            fp.threads = cfg.threads;
            let fit = fit::fit_track(&peaks, &bt.beat_logits, &bt.down_logits, duration_s, &w, &fp);
            Ok((bt, fit, delay, ms(t), mp3_ms))
        })();
        (beat, key.map(|h| h.join().unwrap()), wf.map(|h| h.join().unwrap()))
    });
    let (bt, fit, delay, fit_ms, mp3_ms) = beat_part?;
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
    let (key_class, key_confidence, key_ms) = match &key_part {
        Some((Some(k), t)) => (Some(k.class), k.confidence, *t),
        Some((None, t)) => (None, 0.0, *t),
        None => (None, 0.0, 0.0),
    };
    let (waveforms, wf_ms) = match wf_part {
        Some((w, t)) => (Some(w?), t),
        None => (None, 0.0),
    };
    let waveform_checksum = waveforms.as_ref().map(waveform::checksum);
    let detail_entries = waveforms.as_ref().map_or(0, |w| w.detail.len());
    let ms_doc = json!({
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
    Ok(Analysis {
        duration_s,
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
        logits: cfg.keep_logits.then(|| (bt.beat_logits, bt.down_logits)),
        ms: ms_doc,
    })
}

impl Analysis {
    pub fn to_json(&self, path: &Path, id: Option<u64>) -> Value {
        json!({
            "id": id,
            "path": path,
            "duration_s": self.duration_s,
            "bpm": self.bpm,
            "first_beat_s": self.first_beat_s,
            "n_beats": self.beats_s.len(),
            "final_beats_s": self.beats_s,
            "downbeats_s": self.downbeats_s,
            "key": self.key_class.map(camelot),
            "key_confidence": self.key_confidence,
            "mp3_delay_ms": self.mp3_delay_s * 1000.0,
            "waveform_checksum": self.waveform_checksum,
            "detail_entries": self.detail_entries,
            "ms": self.ms,
        })
    }
}
