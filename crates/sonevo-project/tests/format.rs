//! A song saved and opened again.

use sonevo_project::{
    import_sample, load, save, Clip, ClipContent, FormatError, Id, Note, Point, Song, Target, PPQ, SONG_FILE,
};

fn with_notes() -> Song {
    let mut song = Song::new("Test");
    let kick = song.channels[0].id;
    let hat = song.channels[2].id;
    let pattern = &mut song.patterns[0];
    pattern.notes.insert(
        kick,
        (0..4).map(|beat| Note { start: beat * PPQ, length: PPQ / 4, key: 60, velocity: 0.8 }).collect(),
    );
    pattern.notes.insert(hat, vec![Note { start: PPQ / 2, length: PPQ / 4, key: 60, velocity: 0.5 }]);
    let track = song.tracks[0].id;
    let pattern = song.patterns[0].id;
    song.clips.push(Clip {
        id: Id::new(),
        track,
        start: 0,
        length: 4 * PPQ,
        offset: 0,
        content: ClipContent::Pattern { pattern },
    });
    song.clips.push(Clip {
        id: Id::new(),
        track: song.tracks[1].id,
        start: 0,
        length: 8 * PPQ,
        offset: 0,
        content: ClipContent::Automation {
            target: Target::ChannelVolume { channel: kick },
            points: vec![
                Point { at: 0, value: 0.0, curve: 0.0 },
                Point { at: 8 * PPQ, value: 1.0, curve: 0.5 },
            ],
        },
    });
    song
}

#[test]
fn a_song_saved_opens_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let song = with_notes();
    save(dir.path(), &song).unwrap();
    let (opened, mended) = load(dir.path()).unwrap();
    assert!(mended.is_empty(), "{mended:?}");
    assert_eq!(opened, song);
}

#[test]
fn a_note_added_changes_one_line_of_one_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut song = with_notes();
    save(dir.path(), &song).unwrap();
    let song_before = std::fs::read_to_string(dir.path().join(SONG_FILE)).unwrap();
    let pattern_file = dir.path().join("patterns").join(format!("{}.json", song.patterns[0].id));
    let pattern_before = std::fs::read_to_string(&pattern_file).unwrap();

    let hat = song.channels[2].id;
    let notes = song.patterns[0].notes.get_mut(&hat).unwrap();
    notes.push(Note { start: PPQ * 3 / 2, length: PPQ / 4, key: 60, velocity: 0.5 });
    save(dir.path(), &song).unwrap();

    assert_eq!(std::fs::read_to_string(dir.path().join(SONG_FILE)).unwrap(), song_before);
    let pattern_after = std::fs::read_to_string(&pattern_file).unwrap();
    let before: Vec<&str> = pattern_before.lines().collect();
    let after: Vec<&str> = pattern_after.lines().collect();
    assert_eq!(after.len(), before.len() + 1);
    let added: Vec<&&str> = after.iter().filter(|line| !before.contains(line)).collect();
    // the line before the new note gains a comma, and the note comes in a line of its own
    assert_eq!(added.len(), 2, "{pattern_after}");
}

#[test]
fn a_pattern_deleted_takes_its_file_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut song = with_notes();
    save(dir.path(), &song).unwrap();
    let gone = song.patterns.remove(0);
    song.clips.clear();
    save(dir.path(), &song).unwrap();
    assert!(!dir.path().join("patterns").join(format!("{}.json", gone.id)).exists());
}

#[test]
fn what_points_nowhere_is_mended_on_opening() {
    let dir = tempfile::tempdir().unwrap();
    let mut song = with_notes();
    // a channel gone, its notes and its route left behind; a clip on a track gone
    song.channels.remove(0);
    song.channels[0].insert = Some(Id::new());
    song.clips[0].track = Id::new();
    save(dir.path(), &song).unwrap();
    let (opened, mended) = load(dir.path()).unwrap();
    assert_eq!(mended.len(), 3, "{mended:?}");
    assert!(opened.patterns[0].notes.len() == 1);
    assert_eq!(opened.clips.len(), 1);
    assert_eq!(opened.channels[0].insert, None);
}

#[test]
fn a_song_from_a_newer_sonevo_is_not_opened() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &Song::new("Test")).unwrap();
    let path = dir.path().join(SONG_FILE);
    let text = std::fs::read_to_string(&path).unwrap().replacen("\"format\": 1", "\"format\": 99", 1);
    std::fs::write(&path, text).unwrap();
    assert!(matches!(load(dir.path()), Err(FormatError::Newer(99))));
}

#[test]
fn a_sample_imported_twice_is_kept_once_by_what_is_in_it() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("Kick 01.WAV");
    std::fs::write(&source, b"RIFF....WAVE").unwrap();
    let song_dir = dir.path().join("Song.sonevo");
    let first = import_sample(&song_dir, &source).unwrap();
    let again = import_sample(&song_dir, &source).unwrap();
    assert_eq!(first, again);
    assert!(first.ends_with(".wav") && first.len() == 20);
    assert_eq!(std::fs::read_dir(song_dir.join("samples")).unwrap().count(), 1);
}
