//! The song kept as it is every little while, should Sonevo end without it being saved.
//!
//! A song never saved is kept in its draft folder, beside the sounds it plays. A song saved
//! somewhere is kept in a folder of the application's own, with the path of the song's folder
//! beside it: its sounds are there.

use serde::Serialize;
use sonevo_project::Song;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// The file naming the folder a song kept here was saved in.
const ORIGIN: &str = "origin.txt";

/// Where a song saved in `folder` is kept, under `root`.
pub fn place(root: &Path, folder: &Path) -> PathBuf {
    // FNV-1a: the same folder, the same place
    let hash = folder
        .to_string_lossy()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    root.join(format!("{hash:016x}.sonevo"))
}

/// Keeps `song` in `at`, with `origin` the folder it is saved in, if it is.
pub fn keep(at: &Path, song: &Song, origin: Option<&Path>) -> Result<(), String> {
    sonevo_project::save(at, song).map_err(|e| e.to_string())?;
    if let Some(origin) = origin {
        std::fs::write(at.join(ORIGIN), origin.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Forgets the song kept in `at`, its sounds left where they are.
pub fn forget(at: &Path) {
    let _ = std::fs::remove_file(at.join(sonevo_project::SONG_FILE));
    let _ = std::fs::remove_file(at.join(ORIGIN));
    let _ = std::fs::remove_dir_all(at.join("patterns"));
}

/// A song kept and never saved after.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    pub path: PathBuf,
    pub name: String,
    /// The folder it is saved in, if it is.
    pub origin: Option<PathBuf>,
    /// Seconds since 1970.
    pub time: u64,
}

/// Every song kept in the folders `roots`, the newest first.
pub fn kept(roots: &[&Path]) -> Vec<Kept> {
    let mut found: Vec<Kept> = roots
        .iter()
        .filter_map(|root| std::fs::read_dir(root).ok())
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let file = path.join(sonevo_project::SONG_FILE);
            let time = file.metadata().ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
            let (song, _) = sonevo_project::load(&path).ok()?;
            let origin = std::fs::read_to_string(path.join(ORIGIN)).ok().map(PathBuf::from);
            Some(Kept { path, name: song.name, origin, time })
        })
        .collect();
    found.sort_by(|a, b| b.time.cmp(&a.time));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_kept_are_found_with_where_they_come_from_and_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let (drafts, saved) = (dir.path().join("drafts"), dir.path().join("autosave"));
        let draft = drafts.join("a.sonevo");
        keep(&draft, &Song::new("Draft"), None).unwrap();
        let origin = dir.path().join("My song.sonevo");
        let at = place(&saved, &origin);
        assert_eq!(at, place(&saved, &origin));
        keep(&at, &Song::new("Saved"), Some(&origin)).unwrap();
        std::fs::create_dir_all(drafts.join("empty.sonevo")).unwrap();

        let found = kept(&[&drafts, &saved]);
        assert_eq!(found.len(), 2, "{found:?}");
        let saved_one = found.iter().find(|k| k.name == "Saved").unwrap();
        assert_eq!(saved_one.origin.as_deref(), Some(origin.as_path()));
        assert!(found.iter().any(|k| k.name == "Draft" && k.origin.is_none()));

        forget(&draft);
        forget(&at);
        assert!(kept(&[&drafts, &saved]).is_empty());
    }
}
