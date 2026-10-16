//! One plugin played in a process of its own, for the engine that started it: should the
//! plugin crash, only this process goes, and the engine plays on.
//!
//! `sonevo-plugin-host <link> <bundle> <plugin id> <sample rate> <max frames> <engine pid>`
//!
//! `sonevo-plugin-host --scan <bundle>` writes the plugins of a bundle, a line each: id,
//! name, vendor and features, separated by tabs. A bundle that crashes as it is read crashes
//! only this process.

use sonevo_clap::Bundle;
use sonevo_plugin_ipc::PluginEnd;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

/// How often the process looks up from waiting, to see whether the engine is still there.
const LOOK_UP: Duration = Duration::from_millis(100);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sonevo-plugin-host: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [scan, bundle] = args.as_slice() {
        if scan == "--scan" {
            return list(Path::new(bundle));
        }
    }
    let [link, bundle, id, rate, max_frames, engine] = args.as_slice() else {
        return Err(
            "usage: sonevo-plugin-host <link> <bundle> <plugin id> <sample rate> <max frames> <engine pid>"
                .into(),
        );
    };
    let (rate, max_frames, engine): (f64, u32, u32) = (rate.parse()?, max_frames.parse()?, engine.parse()?);
    let mut end = PluginEnd::open(link)?;
    let bundle = Bundle::open(Path::new(bundle))?;
    let mut instance = bundle.instantiate(id)?;
    instance.activate(rate, max_frames)?;

    // the plugin made, activated and dropped here, played on a thread of its own
    let engine = Engine::watch(engine)?;
    let played = std::thread::Builder::new().name("audio".into()).spawn(move || {
        raise_priority();
        while !end.stopping() && engine.alive() {
            end.serve(LOOK_UP, |asked| {
                if !instance.process(asked.notes, asked.left, asked.right) {
                    asked.left.fill(0.0);
                    asked.right.fill(0.0);
                }
            });
        }
        instance.stop();
        instance
    })?;
    let instance = played.join().map_err(|_| "the plugin failed while playing")?;
    drop(instance);
    Ok(())
}

fn list(bundle: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let clean = |text: &str| text.replace(['\t', '\n', '\r'], " ");
    for plugin in Bundle::open(bundle)?.plugins() {
        let features: Vec<String> = plugin.features.iter().map(|f| clean(f)).collect();
        println!(
            "{}\t{}\t{}\t{}",
            clean(&plugin.id),
            clean(&plugin.name),
            clean(&plugin.vendor),
            features.join(",")
        );
    }
    Ok(())
}

/// The engine's process, to end with it.
struct Engine(windows_sys::Win32::Foundation::HANDLE);

// The handle is only waited on.
unsafe impl Send for Engine {}

impl Engine {
    fn watch(pid: u32) -> std::io::Result<Engine> {
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};
        // SAFETY: a plain Win32 call
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Engine(handle))
    }

    fn alive(&self) -> bool {
        use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
        use windows_sys::Win32::System::Threading::WaitForSingleObject;
        // SAFETY: the handle is open
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_TIMEOUT }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: the handle is ours
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

/// The thread scheduled as Windows schedules audio ("Pro Audio"): played before anything
/// but the system's own.
fn raise_priority() {
    use windows_sys::Win32::System::Threading::AvSetMmThreadCharacteristicsW;
    let task: Vec<u16> = "Pro Audio".encode_utf16().chain([0]).collect();
    let mut index = 0;
    // SAFETY: a plain Win32 call; the handle stays with the thread until it ends
    let handle = unsafe { AvSetMmThreadCharacteristicsW(task.as_ptr(), &mut index) };
    if handle.is_null() {
        eprintln!("sonevo-plugin-host: the audio thread keeps a normal priority");
    }
}
