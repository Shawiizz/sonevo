//! One plugin played in a process of its own, for the engine that started it: should the
//! plugin crash, only this process goes, and the engine plays on.
//!
//! `sonevo-plugin-host <link> <bundle> <plugin id> <sample rate> <max frames> <engine pid>
//! [<control> [<state file>]]`: the events named after `control` open the plugin's window
//! and save its state; the state file, if given, is loaded before the plugin plays.
//!
//! `sonevo-plugin-host --scan <bundle>` writes the plugins of a bundle, a line each: id,
//! name, vendor and features, separated by tabs. A bundle that crashes as it is read crashes
//! only this process.
//!
//! `sonevo-plugin-host --params <bundle> <plugin id>` writes the plugin's settings, a line
//! each: id, name, least, most, default, and `stepped` or not.

mod control;

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
    if let [params, bundle, id] = args.as_slice() {
        if params == "--params" {
            return describe(Path::new(bundle), id);
        }
    }
    let (played_args, control, state) = match args.len() {
        8 => (&args[..6], Some(args[6].as_str()), Some(Path::new(&args[7]))),
        7 => (&args[..6], Some(args[6].as_str()), None),
        _ => (&args[..], None, None),
    };
    let [link, bundle, id, rate, max_frames, engine] = played_args else {
        return Err("usage: sonevo-plugin-host <link> <bundle> <plugin id> <sample rate> <max frames> \
                    <engine pid> [<control> [<state file>]]"
            .into());
    };
    let (rate, max_frames, engine): (f64, u32, u32) = (rate.parse()?, max_frames.parse()?, engine.parse()?);
    let mut end = PluginEnd::open(link)?;
    let bundle = Bundle::open(Path::new(bundle))?;
    let title = bundle.plugins().into_iter().find(|p| p.id == *id).map_or_else(|| id.clone(), |p| p.name);
    let mut instance = bundle.instantiate(id)?;
    // the state saved with the song, given back before anything plays
    if let Some(bytes) = state.and_then(|path| std::fs::read(path).ok()) {
        if !instance.load_state(&bytes) {
            eprintln!("sonevo-plugin-host: the plugin would not take its state back");
        }
    }
    instance.activate(rate, max_frames)?;
    // its window and its state, kept on this thread while the instance plays on the other
    let mut control =
        control.and_then(|name| control::Control::new(name, instance.gui(), instance.keeper(), &title));

    // the plugin made, activated and dropped here, played on a thread of its own
    let engine = Engine::watch(engine)?;
    let played = std::thread::Builder::new().name("audio".into()).spawn(move || {
        raise_priority();
        while !end.stopping() && engine.alive() {
            end.serve(LOOK_UP, |asked| {
                if !instance.process(asked.notes, asked.changes, asked.left, asked.right) {
                    asked.left.fill(0.0);
                    asked.right.fill(0.0);
                }
            });
        }
        instance.stop();
        instance
    })?;
    while !played.is_finished() {
        match &mut control {
            Some(control) => control.serve(LOOK_UP),
            None => std::thread::sleep(LOOK_UP),
        }
    }
    // the window closed before the plugin goes
    drop(control);
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

fn describe(bundle: &Path, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let clean = |text: &str| text.replace(['\t', '\n', '\r'], " ");
    let instance = Bundle::open(bundle)?.instantiate(id)?;
    for p in instance.params() {
        let stepped = if p.stepped { "stepped" } else { "" };
        println!("{}\t{}\t{}\t{}\t{}\t{stepped}", p.id, clean(&p.name), p.min, p.max, p.default);
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
