//! Keyboards plugged in: their keys played on the channel chosen, those plugged in later
//! found as they come.

use midir::{MidiInput, MidiInputConnection};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A key pressed (its velocity from 0 to 1) or let go of, as a MIDI message says.
pub fn note(message: &[u8]) -> Option<(u8, f32, bool)> {
    match *message {
        [status, key, velocity, ..] if status & 0xF0 == 0x90 && velocity > 0 => {
            Some((key & 127, f32::from(velocity) / 127.0, true))
        }
        [status, key, _, ..] if status & 0xF0 == 0x80 || status & 0xF0 == 0x90 => {
            Some((key & 127, 0.0, false))
        }
        _ => None,
    }
}

/// How often the keyboards are looked for again.
const LOOK: Duration = Duration::from_secs(2);

/// The keyboards being listened to, by name.
pub type Inputs = Arc<Mutex<BTreeMap<String, MidiInputConnection<()>>>>;

/// Listens to every keyboard there is or comes, `played` called with each key.
pub fn listen(played: impl Fn(u8, f32, bool) + Send + Sync + 'static) -> Inputs {
    let inputs: Inputs = Arc::default();
    let played = Arc::new(played);
    let kept = inputs.clone();
    std::thread::spawn(move || loop {
        if let Ok(finder) = MidiInput::new("Sonevo") {
            let ports: Vec<_> = finder
                .ports()
                .into_iter()
                .filter_map(|p| finder.port_name(&p).ok().map(|n| (n, p)))
                .collect();
            let mut open = kept.lock().unwrap_or_else(|e| e.into_inner());
            // a keyboard unplugged is let go of
            open.retain(|name, _| ports.iter().any(|(n, _)| n == name));
            for (name, port) in ports {
                if open.contains_key(&name) {
                    continue;
                }
                let Ok(input) = MidiInput::new("Sonevo") else { continue };
                let played = played.clone();
                let connected = input.connect(
                    &port,
                    "Sonevo",
                    move |_, message, _| {
                        if let Some((key, velocity, on)) = note(message) {
                            played(key, velocity, on);
                        }
                    },
                    (),
                );
                if let Ok(connection) = connected {
                    open.insert(name, connection);
                }
            }
        }
        std::thread::sleep(LOOK);
    });
    inputs
}

#[cfg(test)]
mod tests {
    use super::note;

    #[test]
    fn notes_are_read_on_every_channel() {
        assert_eq!(note(&[0x90, 60, 127]), Some((60, 1.0, true)));
        assert_eq!(note(&[0x93, 64, 0]), Some((64, 0.0, false)));
        assert_eq!(note(&[0x85, 67, 40]), Some((67, 0.0, false)));
        assert_eq!(note(&[0xB0, 7, 100]), None);
        assert_eq!(note(&[0xF8]), None);
    }
}
