//! The plugins installed: found by asking `sonevo-plugin-host` what each bundle holds, and
//! each played in a process of its own by [`Remote`], so that a plugin crashing takes only
//! its process down. The process is started again at once, and the keys held are pressed
//! again in it.

mod remote;

pub use remote::Remote;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// A plugin a bundle offers.
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

/// What the bundle at `bundle` offers, asked in a process of its own: nothing if it fails.
pub fn scan_bundle(host: &Path, bundle: &Path) -> Vec<Found> {
    let Ok(mut child) = Command::new(host)
        .arg("--scan")
        .arg(bundle)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < SCAN_TIME => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
        }
    }
    let Ok(output) = child.wait_with_output() else { return Vec::new() };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
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
