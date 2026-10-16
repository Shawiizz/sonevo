//! What Sonevo remembers from one time to the next: the sound card, the language, the songs
//! opened last.

use serde::{Deserialize, Serialize};
use sonevo_audio::Driver;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// `wasapi` or `asio`.
    pub driver: String,
    pub device: Option<String>,
    pub buffer: Option<u32>,
    /// The input recorded from; the driver's default if none.
    pub input: Option<String>,
    /// `en`, `fr`, or none for the system's.
    pub language: Option<String>,
    /// `dark`, `light`, or none for the system's.
    pub theme: Option<String>,
    /// The colour things chosen and played stand out in: `blue`, `green`…, none for violet.
    pub accent: Option<String>,
    pub recent: Vec<PathBuf>,
    /// The folders the browser shows sounds from.
    pub folders: Vec<PathBuf>,
    /// Folders of plugins, besides those plugins are installed in.
    pub plugin_folders: Vec<PathBuf>,
    /// Whether the demo was opened already.
    pub welcomed: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            driver: "wasapi".into(),
            device: None,
            buffer: None,
            input: None,
            language: None,
            theme: None,
            accent: None,
            recent: Vec::new(),
            folders: Vec::new(),
            plugin_folders: Vec::new(),
            welcomed: false,
        }
    }
}

const RECENT: usize = 12;

impl Settings {
    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text + "\n")
    }

    pub fn driver(&self) -> Driver {
        if self.driver == "asio" {
            Driver::Asio
        } else {
            Driver::Wasapi
        }
    }

    pub fn audio(&self) -> sonevo_audio::Settings {
        sonevo_audio::Settings { driver: self.driver(), device: self.device.clone(), buffer: self.buffer }
    }

    /// `folder` first among the songs opened last.
    pub fn opened(&mut self, folder: &Path) {
        self.recent.retain(|r| r != folder);
        self.recent.insert(0, folder.to_owned());
        self.recent.truncate(RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_read_back_and_fill_in_what_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        let mut settings = Settings { driver: "asio".into(), ..Settings::default() };
        for i in 0..20 {
            settings.opened(Path::new(&format!("song {i}")));
        }
        settings.opened(Path::new("song 3"));
        settings.save(&path).unwrap();
        let read = Settings::load(&path);
        assert_eq!(read, settings);
        assert_eq!(read.recent.len(), RECENT);
        assert_eq!(read.recent[0], Path::new("song 3"));
        std::fs::write(&path, r#"{"language":"fr"}"#).unwrap();
        assert_eq!(Settings::load(&path).driver, "wasapi");
    }
}
