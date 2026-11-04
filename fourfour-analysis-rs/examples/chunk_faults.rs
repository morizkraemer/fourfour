use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use rten::{ModelOptions, RunOptions, ThreadPool, ValueOrView, ValueView};

const BIG: usize = 128 * 1024;
const HUGE: usize = 32 << 20;

static BIG_ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BIG_BYTES: AtomicUsize = AtomicUsize::new(0);
static HUGE_ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BIG_FREES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() >= BIG {
            BIG_ALLOCS.fetch_add(1, Ordering::Relaxed);
            BIG_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            if layout.size() >= HUGE {
                HUGE_ALLOCS.fetch_add(1, Ordering::Relaxed);
            }
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() >= BIG {
            BIG_FREES.fetch_add(1, Ordering::Relaxed);
        }
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size >= BIG {
            BIG_ALLOCS.fetch_add(1, Ordering::Relaxed);
            BIG_BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn thread_stat() -> (u64, f64, f64) {
    let text = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = text.rsplit(')').next().unwrap_or("").split_whitespace().collect();
    let n = |i: usize| fields.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    (n(7) as u64, n(11) * 10.0, n(12) * 10.0)
}

fn main() -> anyhow::Result<()> {
    if std::env::var_os("CHUNK_HEAP").is_some() {
        fourfour_analysis::heap::keep_freed_memory();
    }
    let dir = std::env::var_os("FOURFOUR_MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap()).join(".cache/fourfour-models"));
    let frames: usize = std::env::var("CHUNK_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(1500);
    let rounds: usize = std::env::var("CHUNK_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
    let mut options = ModelOptions::with_all_ops();
    options.prepack_weights(false);
    let model = options.load_file(dir.join("beat_this.onnx"))?;
    let input = model.find_node("spectrogram").unwrap();
    let beat = model.find_node("beat").or_else(|| model.find_node("beat_logits")).unwrap();
    let down = model.find_node("downbeat").or_else(|| model.find_node("downbeat_logits")).unwrap();
    let mut seed = 0x2545_f491_u32;
    let data: Vec<f32> = (0..frames * 128)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 1000) as f32 / 250.0
        })
        .collect();
    let run_options = RunOptions::default().with_thread_pool(Some(Arc::new(ThreadPool::with_num_threads(1))));
    let mut hash = 0u64;
    for round in 0..rounds {
        let (f0, u0, s0) = thread_stat();
        let (a0, b0, h0, fr0) = (
            BIG_ALLOCS.load(Ordering::Relaxed),
            BIG_BYTES.load(Ordering::Relaxed),
            HUGE_ALLOCS.load(Ordering::Relaxed),
            BIG_FREES.load(Ordering::Relaxed),
        );
        let t = Instant::now();
        let view = ValueView::from_shape(&[1, frames, 128][..], data.as_slice())?;
        let outs = model.run(vec![(input, ValueOrView::from(view))], &[beat, down], Some(run_options.clone()))?;
        for v in outs {
            for x in v.into_tensor::<f32>().unwrap().into_data() {
                hash = (hash ^ x.to_bits() as u64).wrapping_mul(0x100000001b3);
            }
        }
        let wall = t.elapsed().as_secs_f64() * 1000.0;
        let (f1, u1, s1) = thread_stat();
        println!(
            "round {round}: faults {} user {:.0} ms sys {:.0} ms wall {wall:.0} ms; allocs>=128K {} ({} MB, {} >=32MB) frees {}",
            f1 - f0,
            u1 - u0,
            s1 - s0,
            BIG_ALLOCS.load(Ordering::Relaxed) - a0,
            (BIG_BYTES.load(Ordering::Relaxed) - b0) >> 20,
            HUGE_ALLOCS.load(Ordering::Relaxed) - h0,
            BIG_FREES.load(Ordering::Relaxed) - fr0
        );
    }
    println!("hash {hash:016x}");
    Ok(())
}
