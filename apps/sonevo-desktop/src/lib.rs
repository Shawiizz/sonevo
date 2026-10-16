//! The Sonevo desktop application: the window, and the commands the interface calls. The
//! song lives here; the interface sends edits and gets the song back as it now is.

mod autosave;
mod demo;
mod settings;
mod studio;

use serde::{Deserialize, Serialize};
use settings::Settings;
use sonevo_engine::{write_wav, Depth, Mode};
use sonevo_project::{Edit, Id, Song, Ticks};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use studio::{Studio, View};
use tauri::{AppHandle, Emitter, Manager, State};

struct App {
    studio: Mutex<Studio>,
    plugins: Mutex<Vec<sonevo_plugins::Found>>,
    sample_infos: Mutex<std::collections::HashMap<String, SampleInfo>>,
    kept: Mutex<Vec<autosave::Kept>>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
}

impl App {
    fn studio(&self) -> std::sync::MutexGuard<'_, Studio> {
        self.studio.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn settings(&self) -> std::sync::MutexGuard<'_, Settings> {
        self.settings.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save_settings(&self) {
        let _ = self.settings().save(&self.settings_path);
    }
}

type Answer<T> = Result<T, String>;

/// The song as it now is, as the interface reads it.
fn view(studio: &Studio) -> serde_json::Value {
    serde_json::to_value::<View>(studio.view()).unwrap_or_default()
}

#[tauri::command]
fn song(app: State<App>) -> serde_json::Value {
    view(&app.studio())
}

#[tauri::command]
fn edit(app: State<App>, edit: Edit) -> Answer<serde_json::Value> {
    let mut studio = app.studio();
    studio.edit(edit)?;
    Ok(view(&studio))
}

#[tauri::command]
fn undo(app: State<App>) -> serde_json::Value {
    let mut studio = app.studio();
    studio.undo();
    view(&studio)
}

#[tauri::command]
fn redo(app: State<App>) -> serde_json::Value {
    let mut studio = app.studio();
    studio.redo();
    view(&studio)
}

#[tauri::command]
fn new_song(app: State<App>, demo: bool) -> serde_json::Value {
    let mut studio = app.studio();
    studio.replace(if demo { demo::song() } else { Song::new("Untitled") }, None);
    view(&studio)
}

#[derive(Serialize)]
struct Opened {
    view: serde_json::Value,
    /// What was mended in the song read, said plainly.
    mended: Vec<String>,
}

#[tauri::command(async)]
fn open_song(app: State<App>, folder: PathBuf) -> Answer<Opened> {
    let mended = app.studio().open(&folder)?;
    app.settings().opened(&folder);
    app.save_settings();
    Ok(Opened { view: view(&app.studio()), mended })
}

/// Saves where the song was saved last: false if it never was, for a folder to be chosen.
#[tauri::command(async)]
fn save_song(app: State<App>) -> Answer<bool> {
    app.studio().save()
}

#[tauri::command(async)]
fn save_song_as(app: State<App>, folder: PathBuf) -> Answer<serde_json::Value> {
    let folder = if folder.extension().is_some_and(|e| e == "sonevo") {
        folder
    } else {
        folder.with_extension("sonevo")
    };
    app.studio().save_as(&folder)?;
    app.settings().opened(&folder);
    app.save_settings();
    Ok(view(&app.studio()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionView {
    id: String,
    name: String,
    automatic: bool,
    time: i64,
}

#[tauri::command(async)]
fn versions(app: State<App>) -> Answer<Vec<VersionView>> {
    let versions = app.studio().versions()?;
    Ok(versions
        .into_iter()
        .map(|v| VersionView { id: v.id, name: v.name, automatic: v.automatic, time: v.time })
        .collect())
}

#[tauri::command(async)]
fn make_version(app: State<App>, name: String) -> Answer<serde_json::Value> {
    let mut studio = app.studio();
    studio.make_version(&name)?;
    Ok(view(&studio))
}

#[tauri::command(async)]
fn restore_version(app: State<App>, id: String) -> Answer<serde_json::Value> {
    let mut studio = app.studio();
    studio.restore_version(&id)?;
    Ok(view(&studio))
}

/// Plays the version `id` in place of the song, or the song again without one.
#[tauri::command(async)]
fn compare_version(app: State<App>, id: Option<String>) -> Answer<serde_json::Value> {
    let mut studio = app.studio();
    studio.compare(id.as_deref())?;
    Ok(view(&studio))
}

/// A sound of the song, as the playlist draws it.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SampleInfo {
    seconds: f64,
    /// The loudest sample of each of its equal parts, from its start to its end.
    peaks: Vec<f32>,
}

/// How many parts a sound's outline is drawn from.
const PEAKS: usize = 1024;

#[tauri::command(async)]
fn sample_info(app: State<App>, name: String) -> Answer<SampleInfo> {
    if let Some(info) = app.sample_infos.lock().unwrap_or_else(|e| e.into_inner()).get(&name) {
        return Ok(info.clone());
    }
    if name.contains(['/', '\\']) {
        return Err(format!("{name} is not a sound of the song"));
    }
    let path = app.studio().working_folder().join("samples").join(&name);
    let sample = sonevo_engine::decode(&path).map_err(|e| e.to_string())?;
    let frames = sample.frames();
    let part = frames.div_ceil(PEAKS).max(1);
    let peaks = (0..frames)
        .step_by(part)
        .map(|from| {
            let to = (from + part).min(frames);
            let side = |s: &[f32]| s[from..to].iter().fold(0.0f32, |a, v| a.max(v.abs()));
            side(&sample.left).max(side(&sample.right))
        })
        .collect();
    let info = SampleInfo { seconds: frames as f64 / f64::from(sample.sample_rate), peaks };
    app.sample_infos.lock().unwrap_or_else(|e| e.into_inner()).insert(name, info.clone());
    Ok(info)
}

#[tauri::command(async)]
fn import_sample(app: State<App>, path: PathBuf) -> Answer<String> {
    app.studio().import_sample(&path)
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Transport {
    Play,
    Pause,
    Stop,
    Seek { tick: Ticks },
    Metronome { on: bool },
    Panic,
}

#[tauri::command]
fn transport(app: State<App>, action: Transport) {
    let mut studio = app.studio();
    let Some(engine) = studio.engine() else { return };
    match action {
        Transport::Play => engine.play(),
        Transport::Pause => engine.pause(),
        Transport::Stop => engine.stop(),
        Transport::Seek { tick } => engine.seek(tick),
        Transport::Metronome { on } => engine.set_metronome(on),
        Transport::Panic => engine.panic(),
    }
}

/// Plays one pattern over and over, or the whole song.
#[tauri::command]
fn set_mode(app: State<App>, pattern: Option<Id>) -> serde_json::Value {
    let mut studio = app.studio();
    studio.set_mode(pattern.map_or(Mode::Song, Mode::Pattern));
    view(&studio)
}

#[tauri::command]
fn note(app: State<App>, channel: Id, key: u8, velocity: f32, on: bool) {
    if let Some(engine) = app.studio().engine() {
        engine.note(channel, key, velocity, on);
    }
}

/// A knob or a fader moving: heard at once, the song changed when it is let go of.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Live {
    Param { device: Id, param: String, value: f64 },
    Channel { id: Id, volume: f32, pan: f32 },
    Insert { id: Id, volume: f32, pan: f32 },
}

#[tauri::command]
fn live(app: State<App>, change: Live) {
    let mut studio = app.studio();
    let Some(engine) = studio.engine() else { return };
    match change {
        Live::Param { device, param, value } => {
            engine.set_param(device, &param, value);
        }
        Live::Channel { id, volume, pan } => engine.set_channel_level(id, volume, pan),
        Live::Insert { id, volume, pan } => engine.set_insert_level(id, volume, pan),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParamInfo {
    id: &'static str,
    name: &'static str,
    min: f64,
    max: f64,
    default: f64,
    unit: &'static str,
    /// `linear`, `log`, or `choice` with its choices.
    scale: &'static str,
    choices: &'static [&'static str],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceInfo {
    name: &'static str,
    label: &'static str,
    instrument: bool,
    params: Vec<ParamInfo>,
}

/// The devices built into Sonevo, their settings described.
#[tauri::command]
fn devices() -> Vec<DeviceInfo> {
    sonevo_dsp::BUILTINS
        .iter()
        .map(|b| DeviceInfo {
            name: b.name,
            label: b.label,
            instrument: b.instrument,
            params: b
                .params
                .iter()
                .map(|p| {
                    let (scale, choices) = match p.scale {
                        sonevo_dsp::Scale::Linear => ("linear", &[][..]),
                        sonevo_dsp::Scale::Log => ("log", &[][..]),
                        sonevo_dsp::Scale::Choice(choices) => ("choice", choices),
                    };
                    ParamInfo {
                        id: p.id,
                        name: p.name,
                        min: p.min,
                        max: p.max,
                        default: p.default,
                        unit: p.unit,
                        scale,
                        choices,
                    }
                })
                .collect(),
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginView {
    bundle: PathBuf,
    id: String,
    name: String,
    vendor: String,
    instrument: bool,
}

/// The plugins found on this computer.
#[tauri::command]
fn plugins(app: State<App>) -> Vec<PluginView> {
    app.plugins
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .map(|f| PluginView {
            bundle: f.bundle.clone(),
            id: f.id.clone(),
            name: f.name.clone(),
            vendor: f.vendor.clone(),
            instrument: f.instrument,
        })
        .collect()
}

/// Looks for the plugins again, in the folders they are installed in and those added.
#[tauri::command(async)]
fn scan_plugins(app: State<App>) -> Vec<PluginView> {
    let mut folders = sonevo_plugins::standard_folders();
    folders.extend(app.settings().plugin_folders.iter().cloned());
    *app.plugins.lock().unwrap_or_else(|e| e.into_inner()) = sonevo_plugins::scan(&folders);
    plugins(app)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputView {
    driver: &'static str,
    name: String,
    default: bool,
}

#[tauri::command(async)]
fn outputs() -> Vec<OutputView> {
    [(sonevo_audio::Driver::Wasapi, "wasapi"), (sonevo_audio::Driver::Asio, "asio")]
        .into_iter()
        .flat_map(|(driver, id)| {
            sonevo_audio::outputs(driver).into_iter().map(move |o| OutputView {
                driver: id,
                name: o.name,
                default: o.default,
            })
        })
        .collect()
}

#[tauri::command]
fn settings(app: State<App>) -> Settings {
    app.settings().clone()
}

/// New settings: the sound card opened again if it changed.
#[tauri::command(async)]
fn set_settings(app: State<App>, settings: Settings) -> studio::AudioStatus {
    let audio_changed = app.settings().audio() != settings.audio();
    *app.settings() = settings;
    app.save_settings();
    let mut studio = app.studio();
    if audio_changed {
        let audio = app.settings().audio();
        studio.start_audio(&audio);
    }
    studio.audio()
}

/// The song played through into a WAV file at `path`.
#[tauri::command(async)]
fn export_wav(app: State<App>, path: PathBuf, depth: String, sample_rate: u32) -> Answer<Vec<String>> {
    let (song, folder) = {
        let studio = app.studio();
        (studio.song.clone(), studio.working_folder().to_owned())
    };
    let rendered = sonevo_engine::render(&song, Some(&folder), sample_rate.clamp(22_050, 192_000));
    let depth = match depth.as_str() {
        "16" => Depth::Pcm16,
        "32" => Depth::Float32,
        _ => Depth::Pcm24,
    };
    write_wav(&path, rendered.sample_rate, &rendered.left, &rendered.right, depth)
        .map_err(|e| e.to_string())?;
    Ok(rendered.missing)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    name: String,
    path: PathBuf,
    folder: bool,
}

const SOUNDS: [&str; 7] = ["wav", "flac", "mp3", "ogg", "aif", "aiff", "wave"];

/// The folders and sounds in `folder`, folders first.
#[tauri::command(async)]
fn browse(folder: PathBuf) -> Answer<Vec<Entry>> {
    let mut entries: Vec<Entry> = std::fs::read_dir(&folder)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            let folder = path.is_dir();
            let sound = path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| SOUNDS.contains(&x.to_lowercase().as_str()));
            (!name.starts_with('.') && (folder || sound)).then_some(Entry { name, path, folder })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.folder.cmp(&a.folder).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// The folders the browser starts from: the song's sounds, those added, the user's music.
#[tauri::command]
fn places(app: AppHandle, state: State<App>) -> Vec<Entry> {
    let mut places = Vec::new();
    let samples = state.studio().working_folder().join("samples");
    places.push(Entry { name: "Song".into(), path: samples, folder: true });
    for folder in &state.settings().folders {
        let name = folder
            .file_name()
            .map_or_else(|| folder.display().to_string(), |n| n.to_string_lossy().into_owned());
        places.push(Entry { name, path: folder.clone(), folder: true });
    }
    if let Ok(music) = app.path().audio_dir() {
        places.push(Entry { name: "Music".into(), path: music, folder: true });
    }
    places
}

/// Sends what changes as the song plays, thirty times a second.
fn watch(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(33));
        let state = app.state::<App>();
        let status = state.studio().status();
        if app.emit("status", status).is_err() {
            break;
        }
    });
}

fn data(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("sonevo"))
}

/// Keeps the song as it changes, every little while.
fn keep(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(20));
        let state = app.state::<App>();
        let kept = state.studio().autosave();
        if let Err(e) = kept {
            let _ = app.emit("autosave-failed", e);
        }
    });
}

/// The songs an earlier run kept and never saved, found as Sonevo started.
#[tauri::command]
fn kept(app: State<App>) -> Vec<autosave::Kept> {
    app.kept.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Opens a song as it was kept, or (`open` false) forgets it.
#[tauri::command(async)]
fn recover(app: State<App>, path: PathBuf, open: bool) -> Answer<serde_json::Value> {
    app.kept.lock().unwrap_or_else(|e| e.into_inner()).retain(|k| k.path != path);
    let mut studio = app.studio();
    if open {
        studio.recover(&path)?;
    } else {
        autosave::forget(&path);
    }
    Ok(view(&studio))
}

/// The song's changes left on purpose: what was kept of them forgotten.
#[tauri::command]
fn leave(app: State<App>) {
    app.studio().forget_autosave();
}

/// Whether `path` is a song's folder.
fn is_song(path: &Path) -> bool {
    path.join(sonevo_project::SONG_FILE).is_file()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let config = app.path().app_config_dir().unwrap_or_else(|_| std::env::temp_dir().join("sonevo"));
            let settings_path = config.join("settings.json");
            let mut settings = Settings::load(&settings_path);
            let first = !settings.welcomed;
            settings.welcomed = true;
            let song = if first { demo::song() } else { Song::new("Untitled") };
            let mut studio = Studio::new(&data(&handle), song);
            let kept = studio.kept();
            studio.start_audio(&settings.audio());
            // the song opened last, if there is one
            if !first {
                if let Some(last) = settings.recent.first().filter(|p| is_song(p)).cloned() {
                    let _ = studio.open(&last);
                }
            }
            let mut folders = sonevo_plugins::standard_folders();
            folders.extend(settings.plugin_folders.iter().cloned());
            let state = App {
                studio: Mutex::new(studio),
                plugins: Mutex::new(sonevo_plugins::scan(&folders)),
                sample_infos: Mutex::new(std::collections::HashMap::new()),
                kept: Mutex::new(kept),
                settings: Mutex::new(settings),
                settings_path,
            };
            state.save_settings();
            app.manage(state);
            keep(handle.clone());
            watch(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            song,
            edit,
            undo,
            redo,
            new_song,
            open_song,
            save_song,
            save_song_as,
            import_sample,
            transport,
            set_mode,
            note,
            live,
            devices,
            outputs,
            settings,
            set_settings,
            export_wav,
            browse,
            places,
            versions,
            make_version,
            restore_version,
            compare_version,
            plugins,
            scan_plugins,
            sample_info,
            kept,
            recover,
            leave,
        ])
        .run(tauri::generate_context!())
        .expect("Sonevo could not start");
}
