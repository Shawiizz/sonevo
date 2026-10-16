//! The song open, its history, where it is saved; and the engine playing it out of the
//! sound card.

use crate::autosave;
use serde::Serialize;
use sonevo_audio::{Output, Settings};
use sonevo_engine::{Engine, Meter, Mode};
use sonevo_project::{Clip, ClipContent, Edit, History, Id, Note, Song, Ticks, Track, PPQ};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

/// The song as the interface shows it: its patterns' notes with it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View<'a> {
    pub song: &'a Song,
    pub notes: BTreeMap<Id, &'a BTreeMap<Id, Vec<Note>>>,
    /// Where it is saved; none for a song not saved yet.
    pub folder: Option<&'a Path>,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// The pattern playing over and over, if not the song.
    pub pattern_mode: Option<Id>,
    pub missing: &'a [String],
    /// The version played in place of the song, to compare by ear.
    pub comparing: Option<&'a str>,
}

/// What changes as the song plays, read many times a second.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub position: f64,
    pub playing: bool,
    pub master: f32,
    pub channels: BTreeMap<Id, f32>,
    pub inserts: BTreeMap<Id, f32>,
    pub audio: AudioStatus,
    /// While recording, the loudest the input was since the last look.
    pub recording: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub device: Option<String>,
    pub sample_rate: u32,
    pub latency_ms: f64,
    pub failed: bool,
    pub error: Option<String>,
}

pub struct Studio {
    pub song: Song,
    history: History,
    folder: Option<PathBuf>,
    /// Where a song not saved yet keeps its sounds.
    draft: PathBuf,
    drafts: PathBuf,
    /// Where songs saved somewhere are kept as they change.
    autosaves: PathBuf,
    dirty: bool,
    /// Changed since it was last kept.
    unkept: bool,
    mode: Mode,
    missing: Vec<String>,
    engine: Option<Engine>,
    output: Option<Output>,
    audio_error: Option<String>,
    comparing: Option<Compared>,
    recording: Option<Recording>,
    /// The folder plugins take their states back from: the song's.
    plugin_folder: Arc<std::sync::Mutex<Option<PathBuf>>>,
}

/// An input being recorded, from where playing was when it started.
struct Recording {
    input: sonevo_audio::Input,
    start: f64,
    left: Vec<f32>,
    right: Vec<f32>,
}

/// A version of the song, written out beside it to be played in its place.
struct Compared {
    id: String,
    song: Song,
    folder: PathBuf,
}

impl Studio {
    /// A studio keeping its drafts and its songs as they change under `data`.
    pub fn new(data: &Path, song: Song) -> Studio {
        let drafts = data.join("drafts");
        Studio {
            song,
            history: History::default(),
            folder: None,
            draft: drafts.join(format!("{}.sonevo", Id::new())),
            autosaves: data.join("autosave"),
            drafts,
            dirty: false,
            unkept: false,
            mode: Mode::Song,
            missing: Vec::new(),
            engine: None,
            output: None,
            audio_error: None,
            comparing: None,
            recording: None,
            plugin_folder: Arc::default(),
        }
    }

    /// The folder the song's sounds are in.
    pub fn working_folder(&self) -> &Path {
        self.folder.as_deref().unwrap_or(&self.draft)
    }

    pub fn view(&self) -> View<'_> {
        View {
            song: &self.song,
            notes: self.song.patterns.iter().map(|p| (p.id, &p.notes)).collect(),
            folder: self.folder.as_deref(),
            dirty: self.dirty,
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            pattern_mode: match self.mode {
                Mode::Pattern(id) => Some(id),
                Mode::Song | Mode::Stem(_) => None,
            },
            missing: &self.missing,
            comparing: self.comparing.as_ref().map(|c| c.id.as_str()),
        }
    }

    /// Hands the song as it now is to the engine.
    fn sync(&mut self) {
        if let Mode::Pattern(id) = self.mode {
            if self.song.pattern(id).is_none() {
                self.mode = Mode::Song;
            }
        }
        let folder = self.folder.clone().unwrap_or_else(|| self.draft.clone());
        *self.plugin_folder.lock().unwrap_or_else(|e| e.into_inner()) = Some(folder.clone());
        let (song, folder) = match &self.comparing {
            Some(c) => (&c.song, c.folder.clone()),
            None => (&self.song, folder),
        };
        if let Some(engine) = &mut self.engine {
            self.missing = engine.load(song, self.mode, Some(&folder));
        }
    }

    pub fn edit(&mut self, edit: Edit) -> Result<(), String> {
        // a change is heard on the song itself
        self.comparing = None;
        self.history.apply(&mut self.song, edit).map_err(|e| e.to_string())?;
        self.dirty = true;
        self.unkept = true;
        self.sync();
        Ok(())
    }

    pub fn undo(&mut self) {
        if self.history.undo(&mut self.song) {
            self.dirty = true;
            self.unkept = true;
            self.sync();
        }
    }

    pub fn redo(&mut self) {
        if self.history.redo(&mut self.song) {
            self.dirty = true;
            self.unkept = true;
            self.sync();
        }
    }

    /// Where the song is kept as it changes.
    fn kept_at(&self) -> PathBuf {
        match &self.folder {
            Some(folder) => autosave::place(&self.autosaves, folder),
            None => self.draft.clone(),
        }
    }

    /// Keeps the song as it is, if it changed since: should Sonevo end, it is not lost.
    pub fn autosave(&mut self) -> Result<(), String> {
        if !self.unkept || !self.dirty {
            return Ok(());
        }
        autosave::keep(&self.kept_at(), &self.song, self.folder.as_deref())?;
        self.unkept = false;
        Ok(())
    }

    /// Forgets the song as it was kept: its changes saved, or left on purpose.
    pub fn forget_autosave(&self) {
        autosave::forget(&self.kept_at());
    }

    /// The songs kept and never saved after, by an earlier time Sonevo ran.
    pub fn kept(&self) -> Vec<autosave::Kept> {
        autosave::kept(&[&self.drafts, &self.autosaves])
    }

    /// Opens a song as it was kept: in its own folder if it was saved, else as the draft it is.
    pub fn recover(&mut self, path: &Path) -> Result<(), String> {
        let found = self.kept().into_iter().find(|k| k.path == path).ok_or("This song is no longer kept")?;
        let (song, _) = sonevo_project::load(path).map_err(|e| e.to_string())?;
        self.replace(song, found.origin.clone());
        if found.origin.is_none() {
            self.draft = path.to_owned();
        }
        self.dirty = true;
        self.unkept = true;
        Ok(())
    }

    /// Another song in place of this one, its history forgotten.
    pub fn replace(&mut self, song: Song, folder: Option<PathBuf>) {
        if let Some(engine) = &mut self.engine {
            engine.stop();
            engine.seek(0);
            engine.panic();
        }
        self.song = song;
        self.folder = folder;
        self.comparing = None;
        self.draft = self.drafts.join(format!("{}.sonevo", Id::new()));
        self.history.clear();
        self.dirty = false;
        self.unkept = false;
        self.mode = Mode::Song;
        self.sync();
    }

    pub fn open(&mut self, folder: &Path) -> Result<Vec<String>, String> {
        let (song, mended) = sonevo_project::load(folder).map_err(|e| e.to_string())?;
        self.replace(song, Some(folder.to_owned()));
        Ok(mended)
    }

    /// The states of the plugins the song plays, saved into `folder` beside it.
    fn save_plugin_states(&self, folder: &Path) {
        let instruments = self.song.channels.iter().map(|c| &c.instrument);
        let effects = self.song.inserts.iter().chain([&self.song.master]).flat_map(|i| i.effects.iter());
        for device in instruments.chain(effects).filter(|d| d.builtin_name().is_none()) {
            if let Some(bytes) = sonevo_plugins::save_state(device.id) {
                let file = sonevo_plugins::state_file(folder, device.id);
                if let Some(dir) = file.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(file, bytes);
            }
        }
    }

    /// Saves where the song was saved last; false if it never was.
    pub fn save(&mut self) -> Result<bool, String> {
        let Some(folder) = self.folder.clone() else { return Ok(false) };
        sonevo_project::save(&folder, &self.song).map_err(|e| e.to_string())?;
        self.save_plugin_states(&folder);
        self.dirty = false;
        self.forget_autosave();
        // every save is a version, made without asking
        sonevo_versions::record(&folder, "Saved", true).map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub fn versions(&self) -> Result<Vec<sonevo_versions::Version>, String> {
        match &self.folder {
            Some(folder) => sonevo_versions::list(folder).map_err(|e| e.to_string()),
            None => Ok(Vec::new()),
        }
    }

    /// Saves, then keeps the song as it is as a version named `name`.
    pub fn make_version(&mut self, name: &str) -> Result<(), String> {
        let Some(folder) = self.folder.clone() else { return Err("Save the song first".into()) };
        sonevo_project::save(&folder, &self.song).map_err(|e| e.to_string())?;
        self.save_plugin_states(&folder);
        self.dirty = false;
        sonevo_versions::record(&folder, name, false).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Brings the song back to the version `id`, the song as it was kept as a version first.
    pub fn restore_version(&mut self, id: &str) -> Result<(), String> {
        let Some(folder) = self.folder.clone() else { return Err("Save the song first".into()) };
        sonevo_project::save(&folder, &self.song).map_err(|e| e.to_string())?;
        sonevo_versions::restore(&folder, id).map_err(|e| e.to_string())?;
        self.open(&folder).map(|_| ())
    }

    /// Plays the version `id` in place of the song, or the song again.
    pub fn compare(&mut self, id: Option<&str>) -> Result<(), String> {
        let Some(id) = id else {
            self.comparing = None;
            self.sync();
            return Ok(());
        };
        let Some(folder) = self.folder.clone() else { return Err("Save the song first".into()) };
        let out = self.autosaves.with_file_name("compare").join(id);
        sonevo_versions::extract(&folder, id, &out).map_err(|e| e.to_string())?;
        // the sounds are the song's, the same in every version
        let (song, _) = sonevo_project::load(&out).map_err(|e| e.to_string())?;
        self.comparing = Some(Compared { id: id.to_owned(), song, folder });
        self.sync();
        Ok(())
    }

    /// Saves into `folder`, the sounds the song plays taken along.
    pub fn save_as(&mut self, folder: &Path) -> Result<(), String> {
        let from = self.working_folder().join("samples");
        let to = folder.join("samples");
        if from.is_dir() && from != to {
            std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
            for name in used_samples(&self.song) {
                let (source, target) = (from.join(&name), to.join(&name));
                if source.is_file() && !target.exists() {
                    std::fs::copy(&source, &target).map_err(|e| e.to_string())?;
                }
            }
        }
        sonevo_project::save(folder, &self.song).map_err(|e| e.to_string())?;
        self.save_plugin_states(folder);
        self.forget_autosave();
        self.folder = Some(folder.to_owned());
        self.dirty = false;
        self.sync();
        Ok(())
    }

    /// Copies the sound at `source` into the song's folder: the name the song knows it by.
    pub fn import_sample(&mut self, source: &Path) -> Result<String, String> {
        let folder = self.working_folder().to_owned();
        sonevo_project::import_sample(&folder, source).map_err(|e| e.to_string())
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode != mode {
            self.mode = mode;
            if let Some(engine) = &mut self.engine {
                engine.seek(0);
            }
            self.sync();
        }
    }

    pub fn engine(&mut self) -> Option<&mut Engine> {
        self.engine.as_mut()
    }

    /// Opens the output `settings` names, the engine playing out of it.
    pub fn start_audio(&mut self, settings: &Settings) {
        self.output = None;
        self.engine = None;
        let (engines, engine_in) = mpsc::channel();
        let started = Output::start(settings, move |rate| {
            let (engine, renderer) = Engine::new(rate as f32);
            let _ = engines.send(engine);
            renderer
        });
        match started.map(|output| (output, engine_in.recv())) {
            Ok((output, Ok(mut engine))) => {
                engine.set_plugins(Arc::new(sonevo_plugins::Clap::new(self.plugin_folder.clone())));
                self.output = Some(output);
                self.engine = Some(engine);
                self.audio_error = None;
            }
            Ok((_, Err(_))) => self.audio_error = Some("the output started no engine".into()),
            Err(e) => self.audio_error = Some(e.to_string()),
        }
        self.sync();
    }

    pub fn audio(&self) -> AudioStatus {
        match &self.output {
            Some(o) => AudioStatus {
                device: Some(o.name.clone()),
                sample_rate: o.sample_rate,
                latency_ms: o.latency() * 1000.0,
                failed: o.failed(),
                error: None,
            },
            None => AudioStatus { error: self.audio_error.clone(), ..AudioStatus::default() },
        }
    }

    pub fn status(&mut self) -> Status {
        let audio = self.audio();
        let recording = self.recording.as_mut().map(|r| {
            r.input.take(&mut r.left, &mut r.right);
            r.input.peak()
        });
        let Some(engine) = &mut self.engine else {
            return Status {
                position: 0.0,
                playing: false,
                master: 0.0,
                channels: BTreeMap::new(),
                inserts: BTreeMap::new(),
                audio,
                recording,
            };
        };
        engine.collect();
        let channels =
            self.song.channels.iter().map(|c| (c.id, engine.meter(Meter::Channel(c.id)))).collect();
        let inserts = self.song.inserts.iter().map(|i| (i.id, engine.meter(Meter::Insert(i.id)))).collect();
        Status {
            position: engine.position(),
            playing: engine.playing(),
            master: engine.meter(Meter::Master),
            channels,
            inserts,
            audio,
            recording,
        }
    }
}

impl Studio {
    /// Records the input `input` of `driver` from where playing is, the song playing.
    pub fn record(&mut self, driver: sonevo_audio::Driver, input: Option<&str>) -> Result<(), String> {
        if self.recording.is_some() {
            return Ok(());
        }
        let Some(engine) = &mut self.engine else { return Err("No sound card is open".into()) };
        let input = sonevo_audio::Input::start(driver, input).map_err(|e| e.to_string())?;
        let start = engine.position();
        engine.play();
        self.recording = Some(Recording { input, start, left: Vec::new(), right: Vec::new() });
        Ok(())
    }

    /// Stops recording: what was heard placed on a new track, from where it started.
    pub fn stop_recording(&mut self) -> Result<(), String> {
        let Some(mut recording) = self.recording.take() else { return Ok(()) };
        if let Some(engine) = &mut self.engine {
            engine.pause();
        }
        recording.input.take(&mut recording.left, &mut recording.right);
        let rate = recording.input.sample_rate;
        drop(recording.input);
        if recording.left.is_empty() {
            return Ok(());
        }
        // written apart first, under the name the song will know it by
        let apart = std::env::temp_dir().join(format!("sonevo-{}", Id::new()));
        let file = apart.join("Recording.wav");
        std::fs::create_dir_all(&apart).map_err(|e| e.to_string())?;
        let written = sonevo_engine::write_wav(
            &file,
            rate,
            &recording.left,
            &recording.right,
            sonevo_engine::Depth::Pcm24,
        )
        .map_err(|e| e.to_string());
        let sample = written.and_then(|()| {
            sonevo_project::import_sample(self.working_folder(), &file).map_err(|e| e.to_string())
        });
        let _ = std::fs::remove_dir_all(&apart);
        let sample = sample?;
        let seconds = recording.left.len() as f64 / f64::from(rate);
        let length = (seconds * self.song.tempo * PPQ as f64 / 60.0).ceil() as Ticks;
        let taken = self.song.tracks.iter().filter(|t| t.name.starts_with("Recording")).count();
        let track = Track { id: Id::new(), name: format!("Recording {}", taken + 1), mute: false };
        let clip = Clip {
            id: Id::new(),
            track: track.id,
            start: recording.start.round() as Ticks,
            length: length.max(1),
            offset: 0,
            content: ClipContent::Audio { sample, gain: 1.0 },
        };
        self.edit(Edit::Batch { edits: vec![Edit::PutTrack { track, index: None }, Edit::PutClip { clip }] })
    }
}

/// The names of the sounds `song` plays, as its folder's `samples` holds them.
pub fn used_samples(song: &Song) -> Vec<String> {
    let instruments = song.channels.iter().filter_map(|c| c.instrument.sample.clone());
    let clips = song.clips.iter().filter_map(|c| match &c.content {
        sonevo_project::ClipContent::Audio { sample, .. } => Some(sample.clone()),
        _ => None,
    });
    let mut names: Vec<String> = instruments.chain(clips).collect();
    names.sort();
    names.dedup();
    names
}
