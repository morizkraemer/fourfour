//! Analysis runs beside playback and the web view. Its work goes on a thread of its own whose priority is lowered
//! first (QoS class utility on macOS, nice 10 on Linux); the threads the analysis spawns from there inherit it, so
//! when cores are short the web view's frames come first, and an idle machine still gives the analysis every core.

pub fn run_below_ui<T: Send>(work: impl FnOnce() -> T + Send) -> Result<T, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("analysis".into())
            .spawn_scoped(scope, || {
                if let Err(err) = lower_current_thread() {
                    eprintln!("analysis runs at normal priority: {err}");
                }
                work()
            })
            .map_err(|e| format!("could not start the analysis thread: {e}"))?
            .join()
            .map_err(|_| "the analysis thread panicked".to_string())
    })
}

#[cfg(target_os = "macos")]
fn lower_current_thread() -> Result<(), String> {
    const QOS_CLASS_UTILITY: u32 = 0x11;
    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
    }
    match unsafe { pthread_set_qos_class_self_np(QOS_CLASS_UTILITY, 0) } {
        0 => Ok(()),
        code => Err(format!("pthread_set_qos_class_self_np failed with {code}")),
    }
}

#[cfg(target_os = "linux")]
const ANALYSIS_NICE: i32 = 10;

#[cfg(target_os = "linux")]
fn lower_current_thread() -> Result<(), String> {
    if unsafe { linux::setpriority(linux::PRIO_PROCESS, linux::gettid() as u32, ANALYSIS_NICE) } == 0 {
        Ok(())
    } else {
        Err(format!("setpriority failed: {}", std::io::Error::last_os_error()))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn lower_current_thread() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "linux")]
mod linux {
    pub const PRIO_PROCESS: i32 = 0;
    unsafe extern "C" {
        pub fn setpriority(which: i32, who: u32, prio: i32) -> i32;
        pub fn getpriority(which: i32, who: u32) -> i32;
        pub fn gettid() -> i32;
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn own_nice() -> i32 {
        unsafe { linux::getpriority(linux::PRIO_PROCESS, linux::gettid() as u32) }
    }

    #[test]
    fn work_and_the_threads_it_spawns_run_at_the_analysis_nice_level() {
        let caller = own_nice();
        let (inside, spawned) = run_below_ui(|| (own_nice(), std::thread::spawn(own_nice).join().unwrap())).unwrap();
        assert_eq!(inside, ANALYSIS_NICE.max(caller));
        assert_eq!(spawned, inside);
        assert_eq!(own_nice(), caller);
    }

    #[test]
    fn a_panic_in_the_work_comes_back_as_an_error() {
        assert_eq!(run_below_ui(|| panic!("boom")).map(|()| ()), Err("the analysis thread panicked".to_string()));
    }
}
