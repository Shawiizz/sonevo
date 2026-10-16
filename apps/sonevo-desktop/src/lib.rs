//! The Sonevo desktop application: the window, and the commands the interface calls. The
//! song lives here; the interface sends edits and gets the song back as it now is.

mod autosave;
mod demo;
mod midi;
mod settings;
mod studio;
mod templates;

use serde::{Deserialize, Serialize};
use settings::Settings;
use sonevo_engine::{write_wav, Depth, Mode};
use sonevo_project::{write_midi, Edit, Id, Midi, Note, Song, Ticks};
use std::collections::BTreeMap;
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
    /// The channel the keyboards play: the one chosen in the interface.
    played: Mutex<Option<Id>>,
    keyboards: midi::Inputs,
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

/// The song for the interface, with the notes it does not have as they are.
fn view(studio: &mut Studio) -> serde_json::Value {
    serde_json::to_value::<View>(studio.view(false)).unwrap_or_default()
}

/// The song for the interface, every pattern's notes with it.
#[tauri::command]
fn song(app: State<App>) -> serde_json::Value {
    serde_json::to_value::<View>(app.studio().view(true)).unwrap_or_default()
}

#[tauri::command]
fn edit(app: State<App>, edit: Edit) -> Answer<serde_json::Value> {
    let mut studio = app.studio();
    studio.edit(edit)?;
    Ok(view(&mut studio))
}

#[tauri::command]
fn undo(app: State<App>) -> serde_json::Value {
    let mut studio = app.studio();
    studio.undo();
    view(&mut studio)
}

#[tauri::command]
fn redo(app: State<App>) -> serde_json::Value {
    let mut studio = app.studio();
    studio.redo();
    view(&mut studio)
}

/// A new song: empty, the demo, one of Sonevo's templates or one saved (`mine:<name>`).
#[tauri::command]
fn new_song(app: State<App>, template: Option<String>) -> Answer<serde_json::Value> {
    if let Some(name) = template.as_deref().and_then(|t| t.strip_prefix("mine:")) {
        let folder = template_folder(&app, name)?;
        let mut studio = app.studio();
        studio.start_from_template(&folder)?;
        return Ok(view(&mut studio));
    }
    let song = match template.as_deref() {
        Some("demo") => demo::song(),
        Some(name) => templates::song(name).unwrap_or_else(|| Song::new("Untitled")),
        None => Song::new("Untitled"),
    };
    let mut studio = app.studio();
    studio.replace(song, None);
    Ok(view(&mut studio))
}

/// Where the template `name` the user saved is kept.
fn template_folder(app: &App, name: &str) -> Answer<PathBuf> {
    let name = name.trim();
    if name.is_empty()
        || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        || name.starts_with('.')
    {
        return Err("A template's name cannot hold signs such as / or :".into());
    }
    Ok(app.settings_path.with_file_name("templates").join(format!("{name}.sonevo")))
}

/// The templates the user saved, by name.
#[tauri::command]
fn my_templates(app: State<App>) -> Vec<String> {
    let folder = app.settings_path.with_file_name("templates");
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| is_song(&e.path()))
        .filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// The song as it is kept as the template `name`, in place of one of that name.
#[tauri::command(async)]
fn save_template(app: State<App>, name: String) -> Answer<()> {
    let folder = template_folder(&app, &name)?;
    // written beside first: the one of that name goes only once the new one is whole
    let new = folder.with_extension("sonevo-new");
    if new.exists() {
        std::fs::remove_dir_all(&new).map_err(|e| e.to_string())?;
    }
    app.studio().save_template(&new)?;
    if folder.exists() {
        std::fs::remove_dir_all(&folder).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&new, &folder).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_template(app: State<App>, name: String) -> Answer<()> {
    std::fs::remove_dir_all(template_folder(&app, &name)?).map_err(|e| e.to_string())
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
    Ok(Opened { view: view(&mut app.studio()), mended })
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
    Ok(view(&mut app.studio()))
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
    Ok(view(&mut studio))
}

#[tauri::command(async)]
fn restore_version(app: State<App>, id: String) -> Answer<serde_json::Value> {
    let mut studio = app.studio();
    studio.restore_version(&id)?;
    Ok(view(&mut studio))
}

/// Plays the version `id` in place of the song, or the song again without one.
#[tauri::command(async)]
fn compare_version(app: State<App>, id: Option<String>) -> Answer<serde_json::Value> {
    let mut studio = app.studio();
    studio.compare(id.as_deref())?;
    Ok(view(&mut studio))
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
    if !sonevo_engine::Samples::plain_name(&name) {
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
    view(&mut studio)
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

#[tauri::command]
fn devices() -> Vec<DeviceInfo> {
    sonevo_dsp::BUILTINS
        .iter()
        .map(|b| DeviceInfo {
            name: b.name,
            label: b.label,
            instrument: b.instrument,
            params: b.params.iter().map(param_info).collect(),
        })
        .collect()
}

fn param_info(p: &sonevo_dsp::Param) -> ParamInfo {
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
}

/// Opens the window of the plugin the device `device` plays: false if it has none.
#[tauri::command]
fn open_plugin_window(device: Id) -> bool {
    sonevo_plugins::open_window(device)
}

#[tauri::command(async)]
fn plugin_params(bundle: String, id: String) -> Vec<ParamInfo> {
    sonevo_plugins::params(&bundle, &id).iter().map(param_info).collect()
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

/// Plays the sound at `path` as it is, to hear it before using it; none stops it.
#[tauri::command(async)]
fn listen(app: State<App>, path: Option<PathBuf>) -> Answer<()> {
    let sample = match path {
        Some(path) => Some(std::sync::Arc::new(sonevo_engine::decode(&path).map_err(|e| e.to_string())?)),
        None => None,
    };
    if let Some(engine) = app.studio().engine() {
        engine.listen(sample);
    }
    Ok(())
}

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

/// What is played to files.
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Part {
    /// The whole song, into the file `path`.
    Song,
    /// The loop only, into the file `path`.
    Loop,
    /// Each mixer track alone, into a file each in the folder `path`.
    Stems,
}

#[derive(Serialize, Default)]
struct Exported {
    /// What could not be played, said plainly.
    missing: Vec<String>,
    /// How loud the file sounds as a whole, in LUFS, and its loudest sample in dB: not for stems.
    loudness: Option<f64>,
    peak: Option<f64>,
}

/// `rendered` made as loud as streaming services play songs, -14 LUFS, as far as its peak
/// stays a decibel under the top.
fn level_for_streaming(rendered: &mut sonevo_engine::Rendered) {
    let Some(lufs) = sonevo_engine::loudness(&rendered.left, &rendered.right, rendered.sample_rate) else {
        return;
    };
    let loudest = rendered.left.iter().chain(&rendered.right).fold(0.0f32, |a, s| a.max(s.abs()));
    if loudest <= 0.0 {
        return;
    }
    let wanted = 10f64.powf((-14.0 - lufs) / 20.0);
    let ceiling = 10f64.powf(-1.0 / 20.0) / f64::from(loudest);
    let gain = wanted.min(ceiling) as f32;
    rendered.left.iter_mut().chain(rendered.right.iter_mut()).for_each(|s| *s *= gain);
}

/// The song played through into WAV or FLAC files at `path`.
#[tauri::command(async)]
fn export_audio(
    app: State<App>,
    path: PathBuf,
    part: Part,
    format: String,
    depth: String,
    sample_rate: u32,
    streaming: bool,
) -> Answer<Exported> {
    let (song, folder) = {
        let studio = app.studio();
        (studio.song.clone(), studio.working_folder().to_owned())
    };
    let flac = format == "flac";
    let bits = match depth.as_str() {
        "16" => 16,
        "32" if !flac => 32,
        _ => 24,
    };
    let depth = match bits {
        16 => Depth::Pcm16,
        32 => Depth::Float32,
        _ => Depth::Pcm24,
    };
    let rate = sample_rate.clamp(22_050, 192_000);
    let write = |rendered: &sonevo_engine::Rendered, path: &Path| -> Answer<()> {
        let (rate, left, right) = (rendered.sample_rate, &rendered.left, &rendered.right);
        if flac {
            sonevo_engine::write_flac(path, rate, left, right, bits).map_err(|e| e.to_string())
        } else {
            write_wav(path, rate, left, right, depth).map_err(|e| e.to_string())
        }
    };
    // a whole file, how loud it sounds said with it
    let measured = |mut rendered: sonevo_engine::Rendered| -> Answer<Exported> {
        if streaming {
            level_for_streaming(&mut rendered);
        }
        write(&rendered, &path)?;
        let loudest = rendered.left.iter().chain(&rendered.right).fold(0.0f32, |a, s| a.max(s.abs()));
        Ok(Exported {
            loudness: sonevo_engine::loudness(&rendered.left, &rendered.right, rendered.sample_rate),
            peak: (loudest > 0.0).then(|| 20.0 * f64::from(loudest).log10()),
            missing: rendered.missing,
        })
    };
    match part {
        Part::Song => measured(sonevo_engine::render_part(&song, Some(&folder), rate, None)),
        Part::Loop => measured(sonevo_engine::render_part(&song, Some(&folder), rate, song.loop_region)),
        Part::Stems => {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            let mut missing = Vec::new();
            // what goes straight to the master first, then each track alone
            let tracks = std::iter::once((0, &song.master))
                .chain(song.inserts.iter().enumerate().map(|(i, t)| (i + 1, t)));
            for (i, insert) in tracks {
                let name: String = insert.name.chars().filter(|c| !r#"<>:"/\\|?*"#.contains(*c)).collect();
                let rendered = sonevo_engine::render_stem(&song, Some(&folder), rate, insert.id);
                write(&rendered, &path.join(format!("{i:02} {name}.{}", if flac { "flac" } else { "wav" })))?;
                missing.extend(rendered.missing);
            }
            missing.dedup();
            Ok(Exported { missing, ..Exported::default() })
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    name: String,
    path: PathBuf,
    folder: bool,
}

const SOUNDS: [&str; 9] = ["wav", "flac", "mp3", "ogg", "aif", "aiff", "wave", "mid", "midi"];

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

#[derive(Serialize)]
struct PresetInfo {
    name: String,
    factory: bool,
    values: BTreeMap<String, f64>,
}

/// Where the presets saved for the built-in device `device` are, if it is one.
fn preset_folder(app: &App, device: &str, name: Option<&str>) -> Answer<PathBuf> {
    if sonevo_dsp::builtin(device).is_none() {
        return Err(format!("{device} is not a device of Sonevo's"));
    }
    let folder = app.settings_path.with_file_name("presets").join(device);
    match name.map(str::trim) {
        None => Ok(folder),
        Some(name) if name.is_empty() || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) => {
            Err("A preset's name cannot hold signs such as / or :".into())
        }
        Some(name) => Ok(folder.join(format!("{name}.json"))),
    }
}

/// The presets of the built-in device `device`: those made for it, then those saved.
#[tauri::command]
fn presets(app: State<App>, device: String) -> Answer<Vec<PresetInfo>> {
    let folder = preset_folder(&app, &device, None)?;
    let mut presets: Vec<PresetInfo> = sonevo_dsp::presets(&device)
        .map(|p| PresetInfo {
            name: p.name.into(),
            factory: true,
            values: p.values.iter().map(|&(id, v)| (id.to_owned(), v)).collect(),
        })
        .collect();
    let mut saved: Vec<PresetInfo> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != "json" {
                return None;
            }
            let values = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
            Some(PresetInfo {
                name: path.file_stem()?.to_string_lossy().into_owned(),
                factory: false,
                values,
            })
        })
        .collect();
    saved.sort_by_key(|p| p.name.to_lowercase());
    presets.extend(saved);
    Ok(presets)
}

#[tauri::command]
fn save_preset(app: State<App>, device: String, name: String, values: BTreeMap<String, f64>) -> Answer<()> {
    let path = preset_folder(&app, &device, Some(&name))?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&values).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_preset(app: State<App>, device: String, name: String) -> Answer<()> {
    std::fs::remove_file(preset_folder(&app, &device, Some(&name))?).map_err(|e| e.to_string())
}

/// The song's sound `sample` played over `factor` times its length at its own pitch, kept in
/// the song as a sound of its own: its name.
#[tauri::command(async)]
fn stretch_sample(app: State<App>, sample: String, factor: f64) -> Answer<String> {
    if !(0.25..=4.0).contains(&factor) {
        return Err("A sound is stretched from a quarter to four times its length".into());
    }
    remake(&app, &sample, "stretched", |l, r| sonevo_dsp::stretch(l, r, factor))
}

/// The song's sound `sample` `semitones` higher or lower, as long as it was, kept in the song
/// as a sound of its own: its name.
#[tauri::command(async)]
fn transpose_sample(app: State<App>, sample: String, semitones: f64) -> Answer<String> {
    if !(-24.0..=24.0).contains(&semitones) {
        return Err("A sound is transposed by two octaves at most".into());
    }
    let named = if semitones > 0.0 { format!("up {semitones}") } else { format!("down {}", -semitones) };
    remake(&app, &sample, &named, |l, r| sonevo_dsp::transpose(l, r, semitones))
}

/// The song's sound `sample` made into another by `change`, named after it with `named`.
fn remake(
    app: &App,
    sample: &str,
    named: &str,
    change: impl FnOnce(&[f32], &[f32]) -> (Vec<f32>, Vec<f32>),
) -> Answer<String> {
    if !sonevo_engine::Samples::plain_name(sample) {
        return Err(format!("{sample} is not a sound of the song"));
    }
    let folder = app.studio().working_folder().to_owned();
    let decoded = sonevo_engine::decode(&folder.join("samples").join(sample)).map_err(|e| e.to_string())?;
    let (left, right) = change(&decoded.left, &decoded.right);
    let stem = Path::new(sample).file_stem().map_or_else(|| "sound".into(), |s| s.to_string_lossy());
    // the hash the name ends with left out: the import gives the new sound its own
    let stem = stem.rsplit_once(' ').map_or(&*stem, |(name, _)| name);
    let scratch = tempfile_path(&format!("{stem} {named}.wav"));
    write_wav(&scratch, decoded.sample_rate as u32, &left, &right, Depth::Float32)
        .map_err(|e| e.to_string())?;
    let made = app.studio().import_sample(&scratch);
    let _ = std::fs::remove_file(&scratch);
    made
}

/// A path in a folder of Sonevo's own under the system's temporary one.
fn tempfile_path(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join("sonevo");
    let _ = std::fs::create_dir_all(&folder);
    folder.join(name)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MixCheck {
    loudness: Option<f64>,
    peak: f64,
    rumble: f64,
    air: f64,
    correlation: f64,
    /// Whether the master ends with a limiter.
    limited: bool,
}

/// The song played through and looked at as a whole, for advice on its mix.
#[tauri::command(async)]
fn check_mix(app: State<App>) -> MixCheck {
    let (song, folder) = {
        let studio = app.studio();
        (studio.song.clone(), studio.working_folder().to_owned())
    };
    let rendered = sonevo_engine::render(&song, Some(&folder), 48_000);
    let found = sonevo_engine::analyse(&rendered.left, &rendered.right, rendered.sample_rate);
    let limited = song
        .master
        .effects
        .iter()
        .any(|d| d.enabled && d.builtin_name() == Some(sonevo_project::builtin::LIMITER));
    MixCheck {
        loudness: found.loudness,
        peak: if found.peak.is_finite() { found.peak } else { -120.0 },
        rumble: found.rumble,
        air: found.air,
        correlation: found.correlation,
        limited,
    }
}

/// The MIDI file at `path`, its tracks that play notes.
#[tauri::command]
fn midi_file(path: PathBuf) -> Answer<Midi> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let midi = sonevo_project::read_midi(&bytes).map_err(|e| e.to_string())?;
    if midi.tracks.is_empty() {
        return Err("This MIDI file plays no notes".into());
    }
    Ok(midi)
}

/// The pattern `pattern`, or the whole song as it plays, written as a MIDI file at `path`: a
/// track for each channel that plays.
#[tauri::command]
fn export_midi(app: State<App>, pattern: Option<Id>, path: PathBuf) -> Answer<()> {
    let bytes = {
        let studio = app.studio();
        let song = &studio.song;
        let notes: Vec<Vec<Note>> = match pattern {
            Some(id) => {
                let pattern = song.patterns.iter().find(|p| p.id == id).ok_or("No such pattern")?;
                song.channels.iter().map(|c| pattern.notes_of(c.id).to_vec()).collect()
            }
            None => sonevo_engine::played_notes(song),
        };
        let tracks: Vec<(&str, &[Note])> = song
            .channels
            .iter()
            .zip(&notes)
            .map(|(c, notes)| (c.name.as_str(), notes.as_slice()))
            .filter(|(_, notes)| !notes.is_empty())
            .collect();
        if tracks.is_empty() {
            return Err(if pattern.is_some() {
                "This pattern plays no notes"
            } else {
                "This song plays no notes"
            }
            .into());
        }
        write_midi(song.tempo, &tracks)
    };
    std::fs::write(path, bytes).map_err(|e| e.to_string())
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
    std::thread::spawn(move || {
        let mut tried = std::time::Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(33));
            let state = app.state::<App>();
            let status = state.studio().status();
            // a card unplugged or taken by another program: opened again, every few seconds
            if status.audio.failed && tried.elapsed() > Duration::from_secs(3) {
                tried = std::time::Instant::now();
                let audio = state.settings().audio();
                state.studio().start_audio(&audio);
            }
            if app.emit("status", status).is_err() {
                break;
            }
        }
    });
}

fn data(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("sonevo"))
}

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

/// Records the input chosen in the settings, the song playing from where it is.
#[tauri::command(async)]
fn record(app: State<App>) -> Answer<()> {
    let (driver, input, count_in) = {
        let settings = app.settings();
        (settings.driver(), settings.input.clone(), settings.count_in)
    };
    app.studio().record(driver, input.as_deref(), count_in)
}

/// Stops recording: what was heard on a new track.
#[tauri::command(async)]
fn stop_recording(app: State<App>) -> Answer<serde_json::Value> {
    let mut studio = app.studio();
    studio.stop_recording()?;
    Ok(view(&mut studio))
}

#[tauri::command(async)]
fn inputs(app: State<App>) -> Vec<String> {
    let driver = app.settings().driver();
    sonevo_audio::inputs(driver)
}

#[tauri::command]
fn play_with_keyboards(app: State<App>, channel: Option<Id>) {
    *app.played.lock().unwrap_or_else(|e| e.into_inner()) = channel;
}

#[tauri::command]
fn keyboards(app: State<App>) -> Vec<String> {
    app.keyboards.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect()
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
    Ok(view(&mut studio))
}

/// The song's changes kept at once, as every 20 seconds, to be offered back after a restart.
#[tauri::command]
fn keep_now(app: State<App>) -> Answer<()> {
    app.studio().autosave()
}

/// The song's changes left on purpose: what was kept of them forgotten.
#[tauri::command]
fn leave(app: State<App>) {
    app.studio().forget_autosave();
}

fn is_song(path: &Path) -> bool {
    path.join(sonevo_project::SONG_FILE).is_file()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
                played: Mutex::new(None),
                keyboards: midi::listen({
                    let handle = handle.clone();
                    move |key, velocity, on| {
                        let state = handle.state::<App>();
                        let channel = *state.played.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(channel) = channel {
                            if let Some(engine) = state.studio().engine() {
                                engine.note(channel, key, velocity, on);
                            }
                        }
                        let _ = handle.emit("key", (key, velocity, on));
                    }
                }),
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
            export_audio,
            midi_file,
            export_midi,
            stretch_sample,
            check_mix,
            my_templates,
            save_template,
            delete_template,
            transpose_sample,
            presets,
            save_preset,
            delete_preset,
            browse,
            places,
            versions,
            make_version,
            restore_version,
            compare_version,
            plugins,
            scan_plugins,
            listen,
            plugin_params,
            open_plugin_window,
            sample_info,
            kept,
            recover,
            keep_now,
            leave,
            play_with_keyboards,
            keyboards,
            record,
            stop_recording,
            inputs,
        ])
        .run(tauri::generate_context!())
        .expect("Sonevo could not start");
}
