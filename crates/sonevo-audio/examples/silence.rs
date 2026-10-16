//! Opens the default output and plays an empty song for a second: the output works, and
//! nothing is heard.

use sonevo_audio::{outputs, Driver, Output, Settings};
use sonevo_engine::{Engine, Mode};
use sonevo_project::Song;
use std::sync::mpsc;

fn main() {
    for driver in [Driver::Wasapi, Driver::Asio] {
        for output in outputs(driver) {
            println!("{driver:?}: {}{}", output.name, if output.default { " (default)" } else { "" });
        }
    }
    let (engines, engine_in) = mpsc::channel();
    let output = Output::start(&Settings::default(), move |rate| {
        let (engine, renderer) = Engine::new(rate as f32);
        let _ = engines.send(engine);
        renderer
    })
    .expect("no output");
    let mut engine: Engine = engine_in.recv().unwrap();
    engine.load(&Song::new("Silence"), Mode::Song, None);
    engine.play();
    std::thread::sleep(std::time::Duration::from_secs(1));
    println!(
        "{} at {} Hz, {} channels: {:.1} ms latency, at tick {:.0}, failed: {}",
        output.name,
        output.sample_rate,
        output.channels,
        output.latency() * 1000.0,
        engine.position(),
        output.failed()
    );
}
