//! The versions of a song: the song's folder as it was each time a version was made, kept in
//! a git repository of its own inside the folder (`.versions`), written by gitoxide. Nothing
//! needs git installed, and the folder stays an ordinary folder: the user's own git, if they
//! have one, sees the repository as one more folder to ignore.

use gix::bstr::{BString, ByteSlice};
use gix::objs::tree::{Entry, EntryKind};
use gix::refs::transaction::PreviousValue;
use gix::ObjectId;
use std::path::{Path, PathBuf};

/// The repository's folder, inside the song's.
pub const DIR: &str = ".versions";
const BRANCH: &str = "refs/heads/versions";
const AUTOMATIC: &str = "Automatic: yes";

#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("the versions could not be read or written: {0}")]
    Git(String),
    #[error("no version {0}")]
    Missing(String),
}

fn git(e: impl std::fmt::Display) -> VersionError {
    VersionError::Git(e.to_string())
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> VersionError + '_ {
    move |e| VersionError::Io(path.to_owned(), e)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// Its commit, in hexadecimal.
    pub id: String,
    pub name: String,
    /// Made by Sonevo itself, as the song was saved; not named by the user.
    pub automatic: bool,
    /// Seconds since 1970.
    pub time: i64,
}

fn repository(song: &Path, create: bool) -> Result<Option<gix::Repository>, VersionError> {
    let dir = song.join(DIR);
    if dir.is_dir() {
        return gix::open(&dir).map(Some).map_err(git);
    }
    if !create {
        return Ok(None);
    }
    let repo = gix::init_bare(&dir).map_err(git)?;
    // git itself, if ever pointed at it, finds the versions where it looks first
    let head = dir.join("HEAD");
    std::fs::write(&head, format!("ref: {BRANCH}\n")).map_err(io(&head))?;
    Ok(Some(repo))
}

/// Whether a file of the song's folder is part of a version: not the repository, not what
/// is hidden.
fn kept(name: &str) -> bool {
    !name.starts_with('.')
}

/// The tree of the folder `dir` as it is, written into `repo`.
fn write_tree(repo: &gix::Repository, dir: &Path) -> Result<ObjectId, VersionError> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(io(dir))? {
        let entry = entry.map_err(io(dir))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !kept(&name) {
            continue;
        }
        let path = entry.path();
        let (mode, oid) = if path.is_dir() {
            (EntryKind::Tree, write_tree(repo, &path)?)
        } else {
            let bytes = std::fs::read(&path).map_err(io(&path))?;
            (EntryKind::Blob, repo.write_blob(bytes).map_err(git)?.detach())
        };
        entries.push(Entry { mode: mode.into(), filename: BString::from(name), oid });
    }
    entries.sort();
    Ok(repo.write_object(gix::objs::Tree { entries }).map_err(git)?.detach())
}

fn latest(repo: &gix::Repository) -> Result<Option<ObjectId>, VersionError> {
    let reference = repo.try_find_reference(BRANCH).map_err(git)?;
    reference.map(|mut r| r.peel_to_id().map(|id| id.detach())).transpose().map_err(git)
}

/// Makes a version of the song's folder as it is now, named `name`; none if nothing changed
/// since the last one.
pub fn record(song: &Path, name: &str, automatic: bool) -> Result<Option<Version>, VersionError> {
    let repo = repository(song, true)?.expect("made");
    let tree = write_tree(&repo, song)?;
    let parent = latest(&repo)?;
    if let Some(parent) = parent {
        if repo.find_commit(parent).map_err(git)?.tree_id().map_err(git)? == tree {
            return Ok(None);
        }
    }
    let name = name.trim();
    let name = if name.is_empty() { "Version" } else { name };
    let message = if automatic { format!("{name}\n\n{AUTOMATIC}\n") } else { format!("{name}\n") };
    let time = gix::date::Time::now_local_or_utc();
    let signature = gix::actor::Signature { name: "Sonevo".into(), email: "sonevo@localhost".into(), time };
    let commit = gix::objs::Commit {
        tree,
        parents: parent.into_iter().collect(),
        author: signature.clone(),
        committer: signature,
        encoding: None,
        message: message.into(),
        extra_headers: Vec::new(),
    };
    let id = repo.write_object(&commit).map_err(git)?.detach();
    repo.reference(BRANCH, id, PreviousValue::Any, "version").map_err(git)?;
    Ok(Some(Version { id: id.to_string(), name: name.to_owned(), automatic, time: time.seconds }))
}

/// Every version, the newest first.
pub fn list(song: &Path) -> Result<Vec<Version>, VersionError> {
    let Some(repo) = repository(song, false)? else { return Ok(Vec::new()) };
    let mut versions = Vec::new();
    let mut next = latest(&repo)?;
    while let Some(id) = next {
        let commit = repo.find_commit(id).map_err(git)?;
        let message = commit.message_raw_sloppy().to_str_lossy().into_owned();
        let name = message.lines().next().unwrap_or_default().to_owned();
        let automatic = message.lines().any(|l| l.trim() == AUTOMATIC);
        let time = commit.time().map_err(git)?.seconds;
        versions.push(Version { id: id.to_string(), name, automatic, time });
        next = commit.parent_ids().next().map(|p| p.detach());
    }
    Ok(versions)
}

fn find(repo: &gix::Repository, id: &str) -> Result<ObjectId, VersionError> {
    let oid = ObjectId::from_hex(id.as_bytes()).map_err(|_| VersionError::Missing(id.to_owned()))?;
    let commit = repo.find_commit(oid).map_err(|_| VersionError::Missing(id.to_owned()))?;
    commit.tree_id().map(|t| t.detach()).map_err(git)
}

/// Writes the tree `tree` into `dir`, taking out what it does not hold.
fn write_out(repo: &gix::Repository, tree: ObjectId, dir: &Path) -> Result<(), VersionError> {
    std::fs::create_dir_all(dir).map_err(io(dir))?;
    let tree = repo.find_tree(tree).map_err(git)?;
    let mut names = Vec::new();
    for entry in tree.iter() {
        let entry = entry.map_err(git)?;
        let name = entry.filename().to_str_lossy().into_owned();
        // a name from the repository never leaves the folder
        if name.contains(['/', '\\']) || name == ".." || !kept(&name) {
            continue;
        }
        let path = dir.join(&name);
        if entry.mode().is_tree() {
            if path.is_file() {
                std::fs::remove_file(&path).map_err(io(&path))?;
            }
            write_out(repo, entry.oid().to_owned(), &path)?;
        } else {
            if path.is_dir() {
                std::fs::remove_dir_all(&path).map_err(io(&path))?;
            }
            let blob = repo.find_blob(entry.oid()).map_err(git)?;
            if std::fs::read(&path).ok().as_deref() != Some(&blob.data[..]) {
                std::fs::write(&path, &blob.data).map_err(io(&path))?;
            }
        }
        names.push(name);
    }
    for entry in std::fs::read_dir(dir).map_err(io(dir))?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if kept(&name) && !names.contains(&name) {
            let path = entry.path();
            let removed =
                if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
            removed.map_err(io(&path))?;
        }
    }
    Ok(())
}

/// Brings the song's folder back to the version `id`, the folder as it was kept as a version
/// first, so that going back can be undone.
pub fn restore(song: &Path, id: &str) -> Result<(), VersionError> {
    let repo = repository(song, true)?.expect("made");
    let tree = find(&repo, id)?;
    record(song, "Before going back", true)?;
    write_out(&repo, tree, song)
}

/// Writes the version `id` into the folder `to`, as a song of its own: to be listened to
/// beside the one open.
pub fn extract(song: &Path, id: &str, to: &Path) -> Result<(), VersionError> {
    let Some(repo) = repository(song, false)? else { return Err(VersionError::Missing(id.to_owned())) };
    let tree = find(&repo, id)?;
    write_out(&repo, tree, to)
}
