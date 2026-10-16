use sonevo_versions::{extract, list, record, restore, VersionError, DIR};
use std::fs;
use std::path::Path;

fn song(dir: &Path, text: &str) {
    fs::create_dir_all(dir.join("patterns")).unwrap();
    fs::write(dir.join("song.json"), text).unwrap();
    fs::write(dir.join("patterns").join("a.json"), format!("notes of {text}")).unwrap();
}

#[test]
fn a_song_without_versions_has_none() {
    let dir = tempfile::tempdir().unwrap();
    assert!(list(dir.path()).unwrap().is_empty());
    assert!(!dir.path().join(DIR).exists());
}

#[test]
fn versions_are_listed_newest_first_and_only_when_something_changed() {
    let dir = tempfile::tempdir().unwrap();
    song(dir.path(), "one");
    let first = record(dir.path(), "First idea", false).unwrap().unwrap();
    assert!(record(dir.path(), "Same", true).unwrap().is_none());
    song(dir.path(), "two");
    let second = record(dir.path(), "Saved", true).unwrap().unwrap();
    let versions = list(dir.path()).unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0], second);
    assert_eq!(versions[1], first);
    assert!(versions[0].automatic && !versions[1].automatic);
    assert_eq!(versions[1].name, "First idea");
    assert!(versions[0].time > 1_700_000_000);
}

#[test]
fn going_back_brings_every_file_back_and_can_be_undone() {
    let dir = tempfile::tempdir().unwrap();
    song(dir.path(), "one");
    let first = record(dir.path(), "One", false).unwrap().unwrap();
    song(dir.path(), "two");
    fs::write(dir.path().join("patterns").join("b.json"), "new pattern").unwrap();
    fs::create_dir_all(dir.path().join("samples")).unwrap();
    fs::write(dir.path().join("samples").join("x.wav"), [1u8, 2, 3]).unwrap();
    restore(dir.path(), &first.id).unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("song.json")).unwrap(), "one");
    assert!(!dir.path().join("patterns").join("b.json").exists());
    assert!(!dir.path().join("samples").exists());
    assert!(dir.path().join(DIR).is_dir());

    // the state left behind was kept: going back to it
    let versions = list(dir.path()).unwrap();
    assert_eq!(versions[0].name, "Before going back");
    restore(dir.path(), &versions[0].id).unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("song.json")).unwrap(), "two");
    assert_eq!(fs::read(dir.path().join("samples").join("x.wav")).unwrap(), [1, 2, 3]);
}

#[test]
fn a_version_is_written_out_beside_the_song() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    song(dir.path(), "one");
    let first = record(dir.path(), "One", false).unwrap().unwrap();
    song(dir.path(), "two");
    extract(dir.path(), &first.id, &out.path().join("A")).unwrap();
    assert_eq!(fs::read_to_string(out.path().join("A").join("song.json")).unwrap(), "one");
    assert_eq!(fs::read_to_string(dir.path().join("song.json")).unwrap(), "two");
    assert!(matches!(restore(dir.path(), "0123"), Err(VersionError::Missing(_))));
}
