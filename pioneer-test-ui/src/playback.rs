//! Desktop audio preview playback for the player strip (rodio).

use std::io::BufReader;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
pub struct PlaybackTick {
    pub position_ms: u64,
    pub duration_ms: u64,
    pub playing: bool,
}

struct PlaybackState {
    sink: Option<Sink>,
    path: Option<String>,
    duration_ms: u64,
    position_ms: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    tick_stop: Arc<AtomicBool>,
}

pub struct PlaybackEngine {
    handle: OutputStreamHandle,
    state: Mutex<PlaybackState>,
}

impl PlaybackEngine {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default().map_err(|e| e.to_string())?;
        // OutputStream is !Send; leak it so the handle stays valid for the process lifetime.
        let _leaked: &'static OutputStream = Box::leak(Box::new(stream));
        Ok(Self {
            handle,
            state: Mutex::new(PlaybackState {
                sink: None,
                path: None,
                duration_ms: 0,
                position_ms: Arc::new(AtomicU64::new(0)),
                playing: Arc::new(AtomicBool::new(false)),
                tick_stop: Arc::new(AtomicBool::new(true)),
            }),
        })
    }

    pub fn play(&self, app: &AppHandle, path: &str, position_ms: u64) -> Result<(), String> {
        let mut g = self.state.lock().map_err(|e| e.to_string())?;
        stop_sink(&mut g);

        let file = std::fs::File::open(path).map_err(|e| format!("Open {}: {e}", path))?;
        let source = Decoder::new(BufReader::new(file)).map_err(|e| e.to_string())?;
        let duration = source.total_duration().unwrap_or(Duration::ZERO);
        let duration_ms = duration.as_millis() as u64;

        let skip = Duration::from_millis(position_ms.min(duration_ms));
        let source = source.skip_duration(skip);

        let sink = Sink::try_new(&self.handle).map_err(|e| e.to_string())?;
        sink.append(source);
        sink.play();

        g.position_ms.store(position_ms.min(duration_ms), Ordering::SeqCst);
        g.duration_ms = duration_ms;
        g.path = Some(path.to_string());
        g.playing.store(true, Ordering::SeqCst);
        g.sink = Some(sink);

        g.tick_stop.store(false, Ordering::SeqCst);
        spawn_tick_thread(
            app.clone(),
            g.position_ms.clone(),
            g.playing.clone(),
            g.tick_stop.clone(),
            duration_ms,
        );

        Ok(())
    }

    pub fn pause(&self) -> Result<(), String> {
        let g = self.state.lock().map_err(|e| e.to_string())?;
        if let Some(sink) = g.sink.as_ref() {
            sink.pause();
        }
        g.playing.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub fn resume(&self) -> Result<(), String> {
        let g = self.state.lock().map_err(|e| e.to_string())?;
        if let Some(sink) = g.sink.as_ref() {
            sink.play();
            g.playing.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn seek(&self, app: &AppHandle, position_ms: u64) -> Result<(), String> {
        let path = {
            let g = self.state.lock().map_err(|e| e.to_string())?;
            g.path.clone()
        };
        let Some(path) = path else {
            return Ok(());
        };
        self.play(app, &path, position_ms)
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut g = self.state.lock().map_err(|e| e.to_string())?;
        stop_sink(&mut g);
        g.path = None;
        g.duration_ms = 0;
        g.position_ms.store(0, Ordering::SeqCst);
        g.playing.store(false, Ordering::SeqCst);
        Ok(())
    }
}

fn stop_sink(g: &mut PlaybackState) {
    if let Some(sink) = g.sink.take() {
        sink.stop();
    }
    g.tick_stop.store(true, Ordering::SeqCst);
}

fn spawn_tick_thread(
    app: AppHandle,
    position_ms: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    tick_stop: Arc<AtomicBool>,
    duration_ms: u64,
) {
    thread::spawn(move || {
        let mut last = std::time::Instant::now();
        while !tick_stop.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(50));
            if playing.load(Ordering::SeqCst) {
                let elapsed = last.elapsed().as_millis() as u64;
                last = std::time::Instant::now();
                let next = (position_ms.load(Ordering::SeqCst) + elapsed).min(duration_ms);
                position_ms.store(next, Ordering::SeqCst);
                if next >= duration_ms {
                    playing.store(false, Ordering::SeqCst);
                }
            } else {
                last = std::time::Instant::now();
            }
            let tick = PlaybackTick {
                position_ms: position_ms.load(Ordering::SeqCst),
                duration_ms,
                playing: playing.load(Ordering::SeqCst),
            };
            let _ = app.emit("playback-tick", tick);
        }
    });
}

#[tauri::command]
pub fn playback_play(
    path: String,
    position_ms: u64,
    app: AppHandle,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.play(&app, &path, position_ms)
}

#[tauri::command]
pub fn playback_pause(engine: tauri::State<'_, Arc<PlaybackEngine>>) -> Result<(), String> {
    engine.pause()
}

#[tauri::command]
pub fn playback_resume(engine: tauri::State<'_, Arc<PlaybackEngine>>) -> Result<(), String> {
    engine.resume()
}

#[tauri::command]
pub fn playback_seek(
    position_ms: u64,
    app: AppHandle,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.seek(&app, position_ms)
}

#[tauri::command]
pub fn playback_stop(engine: tauri::State<'_, Arc<PlaybackEngine>>) -> Result<(), String> {
    engine.stop()
}
