//! Settings ready made for the built-in devices: a sound to start from. What a preset leaves
//! out is at its default.

pub struct Preset {
    pub device: &'static str,
    pub name: &'static str,
    pub values: &'static [(&'static str, f64)],
}

const fn preset(device: &'static str, name: &'static str, values: &'static [(&'static str, f64)]) -> Preset {
    Preset { device, name, values }
}

const SYNTH: &str = "sonevo.synth";
const DRUMS: &str = "sonevo.drums";
const FM: &str = "sonevo.fm";
const EQ: &str = "sonevo.eq";
const COMPRESSOR: &str = "sonevo.compressor";
const DELAY: &str = "sonevo.delay";
const REVERB: &str = "sonevo.reverb";
const DISTORTION: &str = "sonevo.distortion";
const FILTER: &str = "sonevo.filter";
const LIMITER: &str = "sonevo.limiter";
const CHORUS: &str = "sonevo.chorus";
const PHASER: &str = "sonevo.phaser";

#[rustfmt::skip]
pub const PRESETS: &[Preset] = &[
    preset(SYNTH, "Supersaw lead", &[
        ("wave1", 0.0), ("wave2", 0.0), ("detune2", 12.0), ("unison", 3.0), ("spread", 25.0),
        ("cutoff", 6000.0), ("resonance", 0.2), ("envAmount", 0.3), ("decay", 0.4), ("sustain", 0.8),
        ("release", 0.3), ("volume", -10.0),
    ]),
    preset(SYNTH, "Sub bass", &[
        ("wave1", 3.0), ("wave2", 2.0), ("semitones2", -12.0), ("detune2", 0.0), ("mix", 0.3),
        ("cutoff", 800.0), ("resonance", 0.1), ("envAmount", 0.1), ("attack", 0.003), ("decay", 0.3),
        ("sustain", 0.9), ("release", 0.12), ("volume", -6.0), ("voices", 1.0), ("glide", 0.04),
    ]),
    preset(SYNTH, "808 glide", &[
        ("wave1", 3.0), ("mix", 0.0), ("cutoff", 20000.0), ("envAmount", 0.0), ("attack", 0.002),
        ("decay", 1.5), ("sustain", 0.3), ("release", 0.4), ("volume", -4.0), ("voices", 1.0),
        ("glide", 0.12),
    ]),
    preset(SYNTH, "Acid bass", &[
        ("wave1", 0.0), ("mix", 0.0), ("cutoff", 300.0), ("resonance", 0.8), ("envAmount", 0.7),
        ("filterDecay", 0.2), ("filterSustain", 0.1), ("decay", 0.3), ("sustain", 0.6),
        ("release", 0.08), ("volume", -8.0), ("voices", 1.0), ("glide", 0.06),
    ]),
    preset(SYNTH, "Wobble bass", &[
        ("wave1", 0.0), ("wave2", 1.0), ("semitones2", -12.0), ("cutoff", 300.0), ("resonance", 0.5),
        ("envAmount", 0.0), ("sustain", 1.0), ("release", 0.1), ("lfoTarget", 0.0), ("lfoSync", 4.0),
        ("lfoAmount", 0.7), ("volume", -8.0), ("voices", 1.0),
    ]),
    preset(SYNTH, "Soft pad", &[
        ("wave1", 0.0), ("wave2", 2.0), ("detune2", 10.0), ("unison", 2.0), ("spread", 20.0),
        ("cutoff", 1500.0), ("resonance", 0.15), ("envAmount", 0.2), ("filterAttack", 0.8),
        ("attack", 0.6), ("decay", 1.5), ("sustain", 0.8), ("release", 1.5), ("lfoRate", 0.3),
        ("lfoAmount", 0.15), ("volume", -12.0),
    ]),
    preset(SYNTH, "Pluck", &[
        ("wave1", 0.0), ("wave2", 1.0), ("cutoff", 400.0), ("resonance", 0.35), ("envAmount", 0.8),
        ("filterDecay", 0.25), ("filterSustain", 0.0), ("attack", 0.002), ("decay", 0.35),
        ("sustain", 0.0), ("release", 0.25), ("volume", -6.0),
    ]),
    preset(SYNTH, "Keys", &[
        ("wave1", 1.0), ("wave2", 2.0), ("mix", 0.6), ("detune2", 4.0), ("cutoff", 3000.0),
        ("envAmount", 0.3), ("filterDecay", 0.6), ("decay", 0.8), ("sustain", 0.4), ("release", 0.4),
        ("volume", -9.0),
    ]),
    preset(SYNTH, "Brass", &[
        ("wave1", 0.0), ("wave2", 0.0), ("detune2", 8.0), ("cutoff", 900.0), ("envAmount", 0.5),
        ("filterAttack", 0.08), ("filterDecay", 0.5), ("filterSustain", 0.5), ("attack", 0.04),
        ("sustain", 0.8), ("release", 0.2), ("volume", -9.0),
    ]),
    preset(SYNTH, "Vibrato lead", &[
        ("wave1", 1.0), ("wave2", 0.0), ("cutoff", 4000.0), ("envAmount", 0.2), ("sustain", 0.8),
        ("release", 0.2), ("lfoTarget", 1.0), ("lfoRate", 5.5), ("lfoAmount", 0.15), ("voices", 1.0),
        ("glide", 0.05), ("volume", -9.0),
    ]),
    preset(FM, "Electric piano", &[
        ("ratio", 1.0), ("index", 0.8), ("bloom", 3.5), ("fade", 0.6), ("decay", 2.5), ("sustain", 0.15),
        ("release", 0.5), ("volume", -8.0),
    ]),
    preset(FM, "Bell", &[
        ("ratio", 3.5), ("index", 1.5), ("bloom", 5.0), ("fade", 2.0), ("attack", 0.001), ("decay", 4.0),
        ("sustain", 0.0), ("release", 2.0), ("volume", -10.0),
    ]),
    preset(FM, "FM bass", &[
        ("ratio", 1.0), ("index", 1.5), ("bloom", 6.0), ("fade", 0.12), ("feedback", 0.2), ("decay", 0.6),
        ("sustain", 0.6), ("release", 0.1), ("volume", -6.0),
    ]),
    preset(FM, "Glass", &[
        ("ratio", 7.0), ("index", 0.6), ("bloom", 2.0), ("fade", 1.0), ("decay", 3.0), ("sustain", 0.2),
        ("release", 1.5), ("volume", -10.0),
    ]),
    preset(FM, "Metal lead", &[
        ("ratio", 2.0), ("index", 3.0), ("bloom", 2.0), ("fade", 0.3), ("feedback", 0.6), ("sustain", 0.8),
        ("release", 0.2), ("volume", -12.0),
    ]),
    preset(DRUMS, "Punchy kick", &[("sound", 0.0), ("decay", 0.8), ("tone", 0.6)]),
    preset(DRUMS, "Long 808 kick", &[("sound", 0.0), ("tune", -5.0), ("decay", 3.0), ("tone", 0.3)]),
    preset(DRUMS, "Tight snare", &[("sound", 1.0), ("decay", 0.6), ("tone", 0.7)]),
    preset(DRUMS, "Crisp hat", &[("sound", 2.0), ("decay", 0.5), ("tone", 0.8), ("level", -4.0)]),
    preset(DRUMS, "Big clap", &[("sound", 3.0), ("decay", 1.6), ("tone", 0.5)]),
    preset(EQ, "Brighter", &[("highGain", 4.0)]),
    preset(EQ, "Warmer", &[("lowFrequency", 120.0), ("lowGain", 3.0), ("highGain", -3.0)]),
    preset(EQ, "Clear the mud", &[("midFrequency", 300.0), ("midGain", -4.0), ("midQ", 1.2)]),
    preset(EQ, "Cut the lows", &[("lowFrequency", 200.0), ("lowGain", -12.0)]),
    preset(COMPRESSOR, "Gentle glue", &[
        ("threshold", -20.0), ("ratio", 2.0), ("attack", 30.0), ("release", 200.0), ("makeup", 2.0),
    ]),
    preset(COMPRESSOR, "Punchy drums", &[
        ("threshold", -18.0), ("ratio", 4.0), ("attack", 15.0), ("release", 100.0), ("knee", 4.0),
        ("makeup", 3.0),
    ]),
    preset(COMPRESSOR, "Pump under the kick", &[
        ("threshold", -30.0), ("ratio", 8.0), ("attack", 0.5), ("release", 150.0), ("knee", 3.0),
    ]),
    preset(COMPRESSOR, "Parallel squash", &[
        ("threshold", -35.0), ("ratio", 10.0), ("attack", 1.0), ("release", 80.0), ("knee", 0.0),
        ("makeup", 10.0), ("mix", 0.4),
    ]),
    preset(DELAY, "Slapback", &[("time", 0.0), ("feedback", 0.1), ("mix", 0.25), ("tone", 8000.0)]),
    preset(DELAY, "Dotted eighth", &[("time", 2.0), ("feedback", 0.45), ("mix", 0.3)]),
    preset(DELAY, "Ping-pong quarter", &[
        ("time", 3.0), ("feedback", 0.5), ("mix", 0.3), ("mode", 1.0), ("tone", 5000.0),
    ]),
    preset(DELAY, "Dub echo", &[("time", 4.0), ("feedback", 0.75), ("mix", 0.35), ("tone", 1800.0)]),
    preset(REVERB, "Small room", &[("size", 0.25), ("damping", 0.6), ("mix", 0.18)]),
    preset(REVERB, "Big hall", &[("size", 0.9), ("damping", 0.3), ("mix", 0.3)]),
    preset(REVERB, "Bright plate", &[("size", 0.6), ("damping", 0.1), ("mix", 0.25)]),
    preset(REVERB, "Wash", &[("size", 1.0), ("damping", 0.5), ("mix", 0.6)]),
    preset(DISTORTION, "Warm drive", &[("drive", 8.0), ("tone", 9000.0), ("output", -4.0)]),
    preset(DISTORTION, "Crunch", &[("drive", 20.0), ("tone", 6000.0), ("output", -10.0)]),
    preset(DISTORTION, "Fuzz", &[("drive", 34.0), ("tone", 4000.0), ("output", -16.0)]),
    preset(DISTORTION, "Parallel grit", &[("drive", 24.0), ("mix", 0.35), ("output", -6.0)]),
    preset(FILTER, "Telephone", &[("mode", 2.0), ("cutoff", 1500.0), ("resonance", 0.4)]),
    preset(FILTER, "Dark", &[("mode", 0.0), ("cutoff", 1200.0), ("resonance", 0.2)]),
    preset(FILTER, "Thin", &[("mode", 1.0), ("cutoff", 400.0), ("resonance", 0.2)]),
    preset(FILTER, "Resonant sweep", &[("mode", 0.0), ("cutoff", 600.0), ("resonance", 0.75)]),
    preset(LIMITER, "Loud master", &[("ceiling", -0.3), ("release", 80.0)]),
    preset(LIMITER, "Safe", &[("ceiling", -1.0), ("release", 200.0)]),
    preset(CHORUS, "Wide chorus", &[("rate", 0.6), ("depth", 0.6), ("delay", 15.0)]),
    preset(CHORUS, "Subtle thickener", &[
        ("rate", 0.3), ("depth", 0.3), ("delay", 10.0), ("width", 0.8), ("mix", 0.3),
    ]),
    preset(CHORUS, "Jet flanger", &[("rate", 0.15), ("depth", 1.0), ("delay", 2.0), ("feedback", 0.75)]),
    preset(CHORUS, "Metallic flanger", &[
        ("rate", 2.0), ("depth", 0.4), ("delay", 1.5), ("feedback", -0.8), ("width", 0.6),
    ]),
    preset(PHASER, "Slow swirl", &[("rate", 0.15), ("depth", 0.8), ("center", 800.0), ("stages", 2.0)]),
    preset(PHASER, "Fast phase", &[
        ("rate", 3.0), ("depth", 0.5), ("center", 1200.0), ("feedback", 0.3), ("stages", 0.0),
    ]),
    preset(PHASER, "Deep", &[
        ("rate", 0.4), ("depth", 1.0), ("center", 600.0), ("feedback", 0.8), ("stages", 2.0),
    ]),
];

/// The presets made for the device `name`.
pub fn presets(name: &str) -> impl Iterator<Item = &'static Preset> + '_ {
    PRESETS.iter().filter(move |p| p.device == name)
}
