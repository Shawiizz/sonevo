use sonevo_project::{
    builtin, Clip, ClipContent, Device, Edit, EditError, History, Id, Insert, Note, Song, Track, PPQ,
};

fn note(start: i64, key: u8) -> Note {
    Note { start, length: PPQ, key, velocity: 0.8 }
}

#[test]
fn edits_read_from_the_interface() {
    let id = Id::new();
    let text = format!(r#"{{"type":"setParam","device":"{id}","param":"cutoff","value":1200}}"#);
    let edit: Edit = serde_json::from_str(&text).unwrap();
    assert_eq!(edit, Edit::SetParam { device: id, param: "cutoff".into(), value: 1200.0 });
    let text =
        r#"{"type":"setSong","name":"A","tempo":140,"signature":{"beats":3,"unit":4},"loopRegion":null}"#;
    assert!(matches!(serde_json::from_str::<Edit>(text).unwrap(), Edit::SetSong { tempo: 140.0, .. }));
}

#[test]
fn a_channel_put_back_replaces_moves_or_adds() {
    let mut song = Song::new("A");
    let mut kick = song.channels[0].clone();
    kick.name = "Big kick".into();
    song.apply(Edit::PutChannel { channel: kick.clone(), index: None }).unwrap();
    assert_eq!(song.channels[0].name, "Big kick");
    song.apply(Edit::PutChannel { channel: kick.clone(), index: Some(3) }).unwrap();
    assert_eq!(song.channels[3].id, kick.id);
    let mut synth = kick;
    synth.id = Id::new();
    synth.instrument = Device::builtin(builtin::SYNTH);
    song.apply(Edit::PutChannel { channel: synth, index: Some(0) }).unwrap();
    assert_eq!(song.channels.len(), 5);
    assert_eq!(song.channels[0].instrument.builtin_name(), Some(builtin::SYNTH));
}

#[test]
fn notes_set_and_taken_with_their_channel() {
    let mut song = Song::new("A");
    let (pattern, channel) = (song.patterns[0].id, song.channels[0].id);
    let notes = vec![note(PPQ, 64), note(0, 60)];
    song.apply(Edit::SetNotes { pattern, channel, notes }).unwrap();
    // mended into time order
    assert_eq!(song.patterns[0].notes_of(channel)[0].key, 60);
    song.apply(Edit::RemoveChannel { id: channel }).unwrap();
    assert!(song.patterns[0].notes.is_empty());
    let gone = Edit::SetNotes { pattern, channel, notes: vec![note(0, 60)] };
    assert_eq!(song.apply(gone), Err(EditError::Missing(channel)));
}

#[test]
fn a_pattern_renamed_keeps_its_notes() {
    let mut song = Song::new("A");
    let (pattern, channel) = (song.patterns[0].id, song.channels[0].id);
    song.apply(Edit::SetNotes { pattern, channel, notes: vec![note(0, 36)] }).unwrap();
    let mut renamed = song.patterns[0].clone();
    renamed.notes.clear();
    renamed.name = "Verse".into();
    song.apply(Edit::PutPattern { pattern: renamed, index: None }).unwrap();
    assert_eq!(song.patterns[0].name, "Verse");
    assert_eq!(song.patterns[0].notes_of(channel).len(), 1);
    assert_eq!(song.apply(Edit::RemovePattern { id: pattern }), Err(EditError::Last("pattern")));
}

#[test]
fn a_track_taken_out_takes_its_clips() {
    let mut song = Song::new("A");
    let track = song.tracks[0].id;
    let pattern = song.patterns[0].id;
    let clip = Clip {
        id: Id::new(),
        track,
        start: -50,
        length: PPQ * 4,
        offset: 0,
        content: ClipContent::Pattern { pattern },
    };
    song.apply(Edit::PutClip { clip }).unwrap();
    assert_eq!(song.clips[0].start, 0);
    song.apply(Edit::RemoveTrack { id: track }).unwrap();
    assert!(song.clips.is_empty());
    song.apply(Edit::PutTrack {
        track: Track { id: Id::new(), name: "New".into(), mute: false },
        index: Some(0),
    })
    .unwrap();
    assert_eq!(song.tracks[0].name, "New");
}

#[test]
fn a_mixer_track_taken_out_sends_its_channels_to_the_master() {
    let mut song = Song::new("A");
    let insert = Insert::new("Drums", "#ffffff");
    let id = insert.id;
    song.apply(Edit::PutInsert { insert, index: None }).unwrap();
    let mut kick = song.channels[0].clone();
    kick.insert = Some(id);
    song.apply(Edit::PutChannel { channel: kick, index: None }).unwrap();
    assert_eq!(song.channels[0].insert, Some(id));
    song.apply(Edit::RemoveInsert { id }).unwrap();
    assert_eq!(song.channels[0].insert, None);
}

#[test]
fn params_are_set_on_instruments_and_effects() {
    let mut song = Song::new("A");
    song.master.effects.push(Device::builtin(builtin::LIMITER));
    let limiter = song.master.effects[0].id;
    let kick = song.channels[0].instrument.id;
    song.apply(Edit::SetParam { device: limiter, param: "ceiling".into(), value: -1.0 }).unwrap();
    song.apply(Edit::SetParam { device: kick, param: "tune".into(), value: 3.0 }).unwrap();
    assert_eq!(song.master.effects[0].params["ceiling"], -1.0);
    assert_eq!(song.channels[0].instrument.params["tune"], 3.0);
    let nothing = Id::new();
    let edit = Edit::SetParam { device: nothing, param: "x".into(), value: 0.0 };
    assert_eq!(song.apply(edit), Err(EditError::Missing(nothing)));
}

#[test]
fn arrangements_are_kept_switched_and_taken_out() {
    let mut song = Song::new("A");
    let track = song.tracks[0].id;
    let pattern = song.patterns[0].id;
    let clip = |start| Clip {
        id: Id::new(),
        track,
        start,
        length: PPQ * 4,
        offset: 0,
        content: ClipContent::Pattern { pattern },
    };
    song.apply(Edit::PutClip { clip: clip(0) }).unwrap();
    song.apply(Edit::KeepArrangementAs { name: "Longer".into() }).unwrap();
    // the copy played from now on changes alone
    song.apply(Edit::PutClip { clip: clip(PPQ * 4) }).unwrap();
    assert_eq!((song.arrangement.as_str(), song.clips.len()), ("Longer", 2));
    assert_eq!((song.arrangements[0].name.as_str(), song.arrangements[0].clips.len()), ("Arrangement 1", 1));
    song.apply(Edit::SwitchArrangement { index: 0 }).unwrap();
    assert_eq!((song.arrangement.as_str(), song.clips.len()), ("Arrangement 1", 1));
    assert_eq!(song.arrangements[0].name, "Longer");
    // a track taken out takes its clips out of every arrangement
    song.apply(Edit::PutTrack {
        track: Track { id: Id::new(), name: "Spare".into(), mute: false },
        index: None,
    })
    .unwrap();
    song.apply(Edit::RemoveTrack { id: track }).unwrap();
    assert!(song.clips.is_empty() && song.arrangements[0].clips.is_empty());
    assert_eq!(song.apply(Edit::SwitchArrangement { index: 3 }), Err(EditError::NoArrangement(3)));
    song.apply(Edit::RemoveArrangement { index: 0 }).unwrap();
    assert!(song.arrangements.is_empty());
}

#[test]
fn undo_and_redo_go_back_and_forth() {
    let mut song = Song::new("A");
    let mut history = History::default();
    let first = song.clone();
    let track = Track { id: Id::new(), name: "Extra".into(), mute: false };
    history.apply(&mut song, Edit::PutTrack { track, index: None }).unwrap();
    assert_eq!(song.tracks.len(), 9);
    assert!(history.undo(&mut song));
    assert_eq!(song, first);
    assert!(!history.undo(&mut song));
    assert!(history.redo(&mut song));
    assert_eq!(song.tracks.len(), 9);
    assert!(!history.can_redo());
}

#[test]
fn a_knob_turned_is_undone_at_once() {
    let mut song = Song::new("A");
    let mut history = History::default();
    let kick = song.channels[0].instrument.id;
    for value in 0..20 {
        let edit = Edit::SetParam { device: kick, param: "tune".into(), value: f64::from(value) };
        history.apply(&mut song, edit).unwrap();
    }
    assert!(history.undo(&mut song));
    assert!(!song.channels[0].instrument.params.contains_key("tune"));
    assert!(!history.can_undo());
}

#[test]
fn a_batch_is_undone_as_one_and_fails_as_one() {
    let mut song = Song::new("A");
    let mut history = History::default();
    let tracks = (0..3).map(|i| Edit::PutTrack {
        track: Track { id: Id::new(), name: format!("{i}"), mute: false },
        index: None,
    });
    history.apply(&mut song, Edit::Batch { edits: tracks.collect() }).unwrap();
    assert_eq!(song.tracks.len(), 11);
    let before = song.clone();
    let failing = Edit::Batch {
        edits: vec![Edit::RemoveTrack { id: song.tracks[0].id }, Edit::RemoveChannel { id: Id::new() }],
    };
    assert!(history.apply(&mut song, failing).is_err());
    assert_eq!(song, before);
    history.undo(&mut song);
    assert_eq!(song.tracks.len(), 8);
}
