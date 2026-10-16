//! The plugins installed, scanned and each played by [`Remote`] in a process of its own, so that
//! a crash takes only its process down; the process is started again and the keys held pressed
//! again.

mod remote;

pub use remote::Remote;

use sonevo_dsp::{Param, Processor};
use sonevo_project::Id;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub bundle: PathBuf,
    pub id: String,
    pub name: String,
    pub vendor: String,
    /// Plays notes; otherwise it changes a sound, as an effect.
    pub instrument: bool,
}

/// `sonevo-plugin-host`, beside the program running or one folder up (tests run from
/// `target/debug/deps`).
pub fn host_program() -> std::io::Result<PathBuf> {
    let name = format!("sonevo-plugin-host{}", std::env::consts::EXE_SUFFIX);
    let here = std::env::current_exe()?;
    let found = here.ancestors().skip(1).take(2).map(|dir| dir.join(&name)).find(|p| p.is_file());
    found.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, format!("{name} is not beside Sonevo"))
    })
}

/// The folders CLAP plugins are installed in on this system.
pub fn standard_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Ok(paths) = std::env::var("CLAP_PATH") {
        folders.extend(std::env::split_paths(&paths));
    }
    if let Ok(common) = std::env::var("COMMONPROGRAMFILES") {
        folders.push(Path::new(&common).join("CLAP"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        folders.push(Path::new(&local).join("Programs").join("Common").join("CLAP"));
    }
    folders
}

/// The bundles under `folder`, in its subfolders too.
fn bundles(folder: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("clap")) && path.is_file() {
            found.push(path);
        } else if path.is_dir() {
            bundles(&path, found);
        }
    }
}

/// How long a bundle is given to say what it holds.
const SCAN_TIME: Duration = Duration::from_secs(10);

/// What the plugin host writes when asked `args`, given the time a scan is: nothing if it
/// fails, crashes or takes too long.
fn ask_host(host: &Path, args: &[&std::ffi::OsStr]) -> Option<String> {
    let mut child = Command::new(host)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < SCAN_TIME => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// What the bundle at `bundle` offers, asked in a process of its own: nothing if it fails.
pub fn scan_bundle(host: &Path, bundle: &Path) -> Vec<Found> {
    let Some(text) = ask_host(host, &["--scan".as_ref(), bundle.as_os_str()]) else { return Vec::new() };
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let (id, name, vendor) = (parts.next()?, parts.next()?, parts.next()?);
            let features = parts.next().unwrap_or_default();
            let instrument = features.split(',').any(|f| f == "instrument");
            Some(Found {
                bundle: bundle.to_owned(),
                id: id.to_owned(),
                name: name.to_owned(),
                vendor: vendor.to_owned(),
                instrument,
            })
        })
        .collect()
}

/// The settings of each plugin asked about, kept for as long as Sonevo runs: a plugin's are
/// asked for once.
static PARAMS: Mutex<Option<Described>> = Mutex::new(None);

/// The settings of plugins, by their bundle and their id.
type Described = HashMap<(String, String), &'static [Param]>;

/// The settings the plugin `id` of the bundle at `bundle` lets be moved, asked in a process
/// of its own: none if it fails.
pub fn params(bundle: &str, id: &str) -> &'static [Param] {
    let key = (bundle.to_owned(), id.to_owned());
    let mut known = PARAMS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(params) = known.get_or_insert_with(HashMap::new).get(&key) {
        return params;
    }
    let text = host_program()
        .ok()
        .and_then(|host| ask_host(&host, &["--params".as_ref(), bundle.as_ref(), id.as_ref()]));
    let params: Vec<Param> = text
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let id = parts.next()?;
            let name = parts.next()?;
            let (min, max, default) =
                (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
            // kept for as long as Sonevo runs, as the settings of built-in devices are
            let id: &'static str = Box::leak(id.to_owned().into_boxed_str());
            let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
            Some(Param::linear(id, name, min, max, default, ""))
        })
        .collect();
    let params: &'static [Param] = Box::leak(params.into_boxed_slice());
    known.get_or_insert_with(HashMap::new).insert(key, params);
    params
}

/// What the events reaching the process of the plugin `device` plays are named after.
fn control(device: Id) -> String {
    format!("sonevo-plugin-{}-{device}", std::process::id())
}

/// The event `name`, opened to be set or waited on: none if no plugin's process made it.
#[cfg(windows)]
fn open_event(name: &str) -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use windows_sys::Win32::System::Threading::{
        OpenEventW, EVENT_MODIFY_STATE, SYNCHRONIZATION_SYNCHRONIZE,
    };
    let name: Vec<u16> = format!("Local\\{name}").encode_utf16().chain([0]).collect();
    // SAFETY: a plain Win32 call, the name living through it
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
    (!event.is_null()).then_some(event)
}

/// Opens the window of the plugin `device` plays, in the plugin's own process: false if it
/// is not playing, or has none.
#[cfg(windows)]
pub fn open_window(device: Id) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::SetEvent;
    use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};
    let Some(event) = open_event(&format!("{}-window", control(device))) else { return false };
    // SAFETY: plain Win32 calls on an event open
    unsafe {
        // Sonevo, in front, lets the plugin's process bring its window in front of it
        AllowSetForegroundWindow(ASFW_ANY);
        let set = SetEvent(event) != 0;
        CloseHandle(event);
        set
    }
}

/// How long a plugin is given to save its state.
const SAVE_TIME: u32 = 3000;

/// The state of the plugin `device` plays, as it saves it, asked of its process: none if it
/// is not playing, has none, or took too long.
#[cfg(windows)]
pub fn save_state(device: Id) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{SetEvent, WaitForSingleObject};
    let name = control(device);
    let save = open_event(&format!("{name}-save"))?;
    let Some(saved) = open_event(&format!("{name}-saved")) else {
        // SAFETY: the event is open
        unsafe { CloseHandle(save) };
        return None;
    };
    // SAFETY: plain Win32 calls on events open
    let done = unsafe {
        SetEvent(save);
        let done = WaitForSingleObject(saved, SAVE_TIME) == WAIT_OBJECT_0;
        CloseHandle(save);
        CloseHandle(saved);
        done
    };
    if !done {
        return None;
    }
    std::fs::read(std::env::temp_dir().join(format!("{name}.state"))).ok()
}

/// CLAP plugins, for the engine: each described by its process, played in another, its
/// state given back from the folder of the song open, as it was saved there.
pub struct Clap {
    folder: Arc<Mutex<Option<PathBuf>>>,
}

impl Clap {
    /// Plugins whose states are in `<folder>/plugins`, the folder followed as it changes.
    pub fn new(folder: Arc<Mutex<Option<PathBuf>>>) -> Clap {
        Clap { folder }
    }
}

/// Where the state of the plugin `device` plays is kept in the song's folder `folder`.
pub fn state_file(folder: &Path, device: Id) -> PathBuf {
    folder.join("plugins").join(format!("{device}.state"))
}

impl sonevo_engine::Plugins for Clap {
    fn params(&self, bundle: &str, id: &str) -> &'static [Param] {
        params(bundle, id)
    }

    fn start(
        &self,
        device: Id,
        bundle: &str,
        id: &str,
        sample_rate: f32,
    ) -> Result<Box<dyn Processor>, String> {
        let params = params(bundle, id);
        let folder = self.folder.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let state = folder.map(|f| state_file(&f, device)).filter(|f| f.is_file());
        Remote::start(Path::new(bundle), id, sample_rate, params, Some((&control(device), state.as_deref())))
            .map(|remote| Box::new(remote) as Box<dyn Processor>)
            .map_err(|e| e.to_string())
    }
}

/// Every plugin of every bundle under `folders`, by name.
pub fn scan(folders: &[PathBuf]) -> Vec<Found> {
    let Ok(host) = host_program() else { return Vec::new() };
    let mut paths = Vec::new();
    for folder in folders {
        bundles(folder, &mut paths);
    }
    paths.sort();
    paths.dedup();
    let mut found: Vec<Found> = paths.iter().flat_map(|bundle| scan_bundle(&host, bundle)).collect();
    found.sort_by_key(|f| f.name.to_lowercase());
    found
}
