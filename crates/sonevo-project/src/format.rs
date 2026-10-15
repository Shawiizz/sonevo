//! The folder a song is saved as:
//!
//! ```text
//! My song.sonevo/
//!   song.json             everything but the patterns' notes
//!   patterns/<id>.json    a pattern's notes, one note a line
//!   samples/<hash>.<ext>  the samples, named by what is in them
//! ```
//!
//! Written the same way every time, the files of a song changed a little differ a little.

use crate::id::Id;
use crate::song::{Note, Song};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

/// The version of the format this Sonevo writes, and the newest it reads.
pub const FORMAT: u32 = 1;
pub const SONG_FILE: &str = "song.json";
const PATTERNS: &str = "patterns";
const SAMPLES: &str = "samples";

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("{}: {source}", path.display())]
    Json { path: PathBuf, source: serde_json::Error },
    #[error("This song was saved by a newer Sonevo (format {0}): update Sonevo to open it.")]
    Newer(u32),
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> FormatError + '_ {
    move |source| FormatError::Io { path: path.to_owned(), source }
}

fn json_error(path: &Path) -> impl FnOnce(serde_json::Error) -> FormatError + '_ {
    move |source| FormatError::Json { path: path.to_owned(), source }
}

#[derive(Serialize, Deserialize)]
struct SongFile {
    format: u32,
    song: Song,
}

#[derive(Deserialize)]
struct PatternFile {
    notes: BTreeMap<Id, Vec<Note>>,
}

/// Saves `song` into the folder `dir`, made if it is not there; the files of patterns no
/// longer in the song go.
pub fn save(dir: &Path, song: &Song) -> Result<(), FormatError> {
    let patterns_dir = dir.join(PATTERNS);
    std::fs::create_dir_all(&patterns_dir).map_err(io_error(&patterns_dir))?;
    let file = SongFile { format: FORMAT, song: song.clone() };
    let song_path = dir.join(SONG_FILE);
    let text = serde_json::to_string_pretty(&file).map_err(json_error(&song_path))?;
    write(&song_path, &(text + "\n"))?;
    for pattern in &song.patterns {
        write(&patterns_dir.join(format!("{}.json", pattern.id)), &pattern_text(&pattern.notes))?;
    }
    // what is left of patterns deleted
    let kept: Vec<String> = song.patterns.iter().map(|p| format!("{}.json", p.id)).collect();
    for entry in std::fs::read_dir(&patterns_dir).map_err(io_error(&patterns_dir))?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".json") && !kept.contains(&name) {
            std::fs::remove_file(entry.path()).map_err(io_error(&entry.path()))?;
        }
    }
    Ok(())
}

/// A pattern's notes, a channel's after another, one note a line: a note added is a line added.
fn pattern_text(notes: &BTreeMap<Id, Vec<Note>>) -> String {
    let mut text = String::from("{\n  \"notes\": {");
    for (i, (channel, notes)) in notes.iter().filter(|(_, n)| !n.is_empty()).enumerate() {
        text.push_str(if i == 0 { "\n" } else { ",\n" });
        text.push_str(&format!("    \"{channel}\": ["));
        for (j, note) in notes.iter().enumerate() {
            text.push_str(if j == 0 { "\n" } else { ",\n" });
            // a note is numbers only: it always serializes
            text.push_str("      ");
            text.push_str(&serde_json::to_string(note).unwrap_or_default());
        }
        text.push_str("\n    ]");
    }
    text.push_str(if notes.values().any(|n| !n.is_empty()) { "\n  }\n}\n" } else { "}\n}\n" });
    text
}

/// Writes `text` over `path` whole or not at all: a crash midway leaves the file as it was.
fn write(path: &Path, text: &str) -> Result<(), FormatError> {
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    let aside = path.with_extension("json.saving");
    std::fs::write(&aside, text).map_err(io_error(&aside))?;
    std::fs::rename(&aside, path).map_err(io_error(path))
}

/// Opens the song saved in `dir`, mended where it points nowhere: what was mended along.
pub fn load(dir: &Path) -> Result<(Song, Vec<String>), FormatError> {
    let song_path = dir.join(SONG_FILE);
    let text = std::fs::read_to_string(&song_path).map_err(io_error(&song_path))?;
    // the format first, alone: a newer one may not parse as this one does
    #[derive(Deserialize)]
    struct Version {
        format: u32,
    }
    let version: Version = serde_json::from_str(&text).map_err(json_error(&song_path))?;
    if version.format > FORMAT {
        return Err(FormatError::Newer(version.format));
    }
    let SongFile { mut song, .. } = serde_json::from_str(&text).map_err(json_error(&song_path))?;
    let mut mended = Vec::new();
    for pattern in &mut song.patterns {
        let path = dir.join(PATTERNS).join(format!("{}.json", pattern.id));
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                pattern.notes = serde_json::from_str::<PatternFile>(&text).map_err(json_error(&path))?.notes
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                mended.push(format!("{} had no file of notes: it is empty", pattern.name));
            }
            Err(source) => return Err(FormatError::Io { path, source }),
        }
    }
    mended.extend(song.mend());
    Ok((song, mended))
}

/// Copies the file at `source` into the song's folder `dir`, named by what is in it: the
/// name the song refers to it by. The same sound imported twice is kept once.
pub fn import_sample(dir: &Path, source: &Path) -> Result<String, FormatError> {
    let bytes = std::fs::read(source).map_err(io_error(source))?;
    let hash = blake3::hash(&bytes).to_hex();
    let extension =
        source.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "wav".into());
    let name = format!("{}.{extension}", &hash[..16]);
    let samples = dir.join(SAMPLES);
    std::fs::create_dir_all(&samples).map_err(io_error(&samples))?;
    let target = samples.join(&name);
    if !target.is_file() {
        std::fs::write(&target, &bytes).map_err(io_error(&target))?;
    }
    Ok(name)
}
