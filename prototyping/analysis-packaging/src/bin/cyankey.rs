use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use ffcore::cyankey;
use ffcore::keymap::{class_from_camelot, is_minor, tonic_of};

struct Entry {
    id: usize,
    file: String,
    truth: usize,
}

fn load_manifest(path: &Path) -> Vec<Entry> {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect("read manifest")).unwrap();
    json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| Entry {
            id: e["id"].as_str().unwrap().parse().unwrap(),
            file: e["path"].as_str().unwrap().rsplit('/').next().unwrap().to_string(),
            truth: class_from_camelot(e["ground_truth"]["key"].as_str().unwrap()).expect("camelot"),
        })
        .collect()
}

#[derive(Default)]
struct Tally {
    n: usize,
    exact: usize,
    relative: usize,
    parallel: usize,
    fifth: usize,
}

impl Tally {
    fn add(&mut self, pred: usize, truth: usize) {
        self.n += 1;
        let (pt, tt) = (tonic_of(pred), tonic_of(truth));
        let (pm, tm) = (is_minor(pred), is_minor(truth));
        if pred == truth {
            self.exact += 1;
        } else if pm != tm && pt == tt {
            self.parallel += 1;
        } else if pm != tm && ((!pm && pt == (tt + 3) % 12) || (pm && pt == (tt + 9) % 12)) {
            self.relative += 1;
        } else if pm == tm && ((pt + 12 - tt) % 12 == 7 || (tt + 12 - pt) % 12 == 7) {
            self.fifth += 1;
        }
    }

    fn report(&self, label: &str) {
        let p = |x: usize| 100.0 * x as f64 / self.n.max(1) as f64;
        println!(
            "{label} n={} exact={} ({:.1}%) relative={:.1}% parallel={:.1}% fifth={:.1}% exact+rel+fifth={:.1}%",
            self.n,
            self.exact,
            p(self.exact),
            p(self.relative),
            p(self.parallel),
            p(self.fifth),
            p(self.exact + self.relative + self.fifth)
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let manifest = PathBuf::from(&args[0]);
    let audio_dir = PathBuf::from(&args[1]);
    let threads: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(4);
    let entries = load_manifest(&manifest);
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<(usize, f64, f64, f64)>>> = entries.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= entries.len() {
                    break;
                }
                let audio = beat_this::load_audio(&audio_dir.join(&entries[i].file), 44100).expect("decode");
                let t = Instant::now();
                let r = cyankey::detect(&audio.samples, 44100, 1);
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                if let Some(r) = r {
                    *results[i].lock().unwrap() = Some((r.class, r.confidence, r.tuning_cents, ms));
                }
            });
        }
    });
    let (mut all, mut test, mut train) = (Tally::default(), Tally::default(), Tally::default());
    let mut times = Vec::new();
    let mut lines = Vec::new();
    for (e, r) in entries.iter().zip(&results) {
        let Some((class, conf, tune, ms)) = *r.lock().unwrap() else { continue };
        times.push(ms);
        all.add(class, e.truth);
        if e.id % 3 == 0 { test.add(class, e.truth) } else { train.add(class, e.truth) }
        lines.push(format!("{} {} {} {:.3} {:.1}", e.id, e.truth, class, conf, tune));
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    all.report("all");
    train.report("dev (id % 3 != 0)");
    test.report("test (id % 3 == 0)");
    println!("detect ms median {:.0} (decode excluded, single thread)", times[times.len() / 2]);
    if let Some(out) = args.get(3) {
        std::fs::write(out, lines.join("\n")).unwrap();
    }
}
