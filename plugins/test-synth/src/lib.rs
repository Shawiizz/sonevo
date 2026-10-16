//! A small CLAP synthesizer, to try Sonevo's hosting of plugins with without any installed:
//! sixteen voices of a soft electric piano, a note's start and end on the very frame asked,
//! and its volume a setting the host moves.

mod voice;

use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::{
    clap_event_header, clap_event_note, clap_event_param_value, clap_input_events, clap_output_events,
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON, CLAP_EVENT_PARAM_VALUE,
};
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_AUDIO_PORT_IS_MAIN, CLAP_EXT_AUDIO_PORTS,
    CLAP_PORT_STEREO,
};
use clap_sys::ext::note_ports::{
    clap_note_port_info, clap_plugin_note_ports, CLAP_EXT_NOTE_PORTS, CLAP_NOTE_DIALECT_CLAP,
};
use clap_sys::ext::params::{
    clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS, CLAP_PARAM_IS_AUTOMATABLE,
};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::id::clap_id;
use clap_sys::id::CLAP_INVALID_ID;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::plugin_features::{CLAP_PLUGIN_FEATURE_INSTRUMENT, CLAP_PLUGIN_FEATURE_SYNTHESIZER};
use clap_sys::process::{clap_process, clap_process_status, CLAP_PROCESS_CONTINUE};
use clap_sys::version::CLAP_VERSION;
use std::ffi::{c_char, c_void, CStr};
use voice::Voices;

const ID: &CStr = c"dev.sonevo.test-synth";

/// The features, ended by a null as CLAP wants: pointers to static strings, shared freely.
struct Features([*const c_char; 3]);
unsafe impl Sync for Features {}

static FEATURES: Features = Features([
    CLAP_PLUGIN_FEATURE_INSTRUMENT.as_ptr(),
    CLAP_PLUGIN_FEATURE_SYNTHESIZER.as_ptr(),
    std::ptr::null(),
]);

static DESCRIPTOR: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: ID.as_ptr(),
    name: c"Sonevo Test Synth".as_ptr(),
    vendor: c"Sonevo".as_ptr(),
    url: c"https://github.com/Shawiizz/sonevo".as_ptr(),
    manual_url: c"".as_ptr(),
    support_url: c"".as_ptr(),
    version: c"0.1.0".as_ptr(),
    description: c"A soft electric piano, to try plugin hosting with".as_ptr(),
    features: FEATURES.0.as_ptr(),
};

#[allow(non_upper_case_globals)]
#[no_mangle]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(entry_factory),
};

static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(factory_count),
    get_plugin_descriptor: Some(factory_descriptor),
    create_plugin: Some(factory_create),
};

unsafe extern "C" fn entry_init(_path: *const c_char) -> bool {
    true
}

unsafe extern "C" fn entry_deinit() {}

unsafe extern "C" fn entry_factory(id: *const c_char) -> *const c_void {
    if unsafe { CStr::from_ptr(id) } == CLAP_PLUGIN_FACTORY_ID {
        &FACTORY as *const clap_plugin_factory as *const c_void
    } else {
        std::ptr::null()
    }
}

unsafe extern "C" fn factory_count(_factory: *const clap_plugin_factory) -> u32 {
    1
}

unsafe extern "C" fn factory_descriptor(
    _factory: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    if index == 0 {
        &DESCRIPTOR
    } else {
        std::ptr::null()
    }
}

/// The plugin CLAP sees first, then its own state: a pointer to one is a pointer to both.
#[repr(C)]
struct Synth {
    plugin: clap_plugin,
    voices: Voices,
    /// The setting `VOLUME`: the gain the voices are played at, from 0 to 1.
    volume: f64,
}

unsafe extern "C" fn factory_create(
    _factory: *const clap_plugin_factory,
    _host: *const clap_host,
    id: *const c_char,
) -> *const clap_plugin {
    if unsafe { CStr::from_ptr(id) } != ID {
        return std::ptr::null();
    }
    let synth = Box::new(Synth {
        plugin: clap_plugin {
            desc: &DESCRIPTOR,
            plugin_data: std::ptr::null_mut(),
            init: Some(plugin_init),
            destroy: Some(plugin_destroy),
            activate: Some(plugin_activate),
            deactivate: Some(plugin_nothing),
            start_processing: Some(plugin_start),
            stop_processing: Some(plugin_nothing),
            reset: Some(plugin_reset),
            process: Some(plugin_process),
            get_extension: Some(plugin_extension),
            on_main_thread: Some(plugin_nothing),
        },
        voices: Voices::new(48_000.0),
        volume: DEFAULT_VOLUME,
    });
    Box::into_raw(synth) as *const clap_plugin
}

/// The synth behind the plugin CLAP hands back.
///
/// # Safety
/// `plugin` is one [`factory_create`] made and [`plugin_destroy`] has not freed.
unsafe fn synth<'a>(plugin: *const clap_plugin) -> &'a mut Synth {
    unsafe { &mut *(plugin as *mut Synth) }
}

unsafe extern "C" fn plugin_init(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn plugin_destroy(plugin: *const clap_plugin) {
    drop(unsafe { Box::from_raw(plugin as *mut Synth) });
}

unsafe extern "C" fn plugin_activate(
    plugin: *const clap_plugin,
    sample_rate: f64,
    _min: u32,
    _max: u32,
) -> bool {
    unsafe { synth(plugin) }.voices = Voices::new(sample_rate as f32);
    true
}

unsafe extern "C" fn plugin_start(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn plugin_nothing(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_reset(plugin: *const clap_plugin) {
    unsafe { synth(plugin) }.voices.silence();
}

unsafe extern "C" fn plugin_process(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    let synth = unsafe { synth(plugin) };
    let process = unsafe { &*process };
    let frames = process.frames_count as usize;
    if process.audio_outputs_count == 0 || frames == 0 {
        return CLAP_PROCESS_CONTINUE;
    }
    let out = unsafe { &*process.audio_outputs };
    let channels = unsafe { std::slice::from_raw_parts(out.data32, out.channel_count as usize) };
    let (left, right) = match channels {
        [left, right, ..] => unsafe {
            (std::slice::from_raw_parts_mut(*left, frames), std::slice::from_raw_parts_mut(*right, frames))
        },
        [mono] => {
            let mono = unsafe { std::slice::from_raw_parts_mut(*mono, frames) };
            synth.voices.render(mono, None);
            return CLAP_PROCESS_CONTINUE;
        }
        [] => return CLAP_PROCESS_CONTINUE,
    };
    // the events come in time order: the block is played in stretches between them
    let events = unsafe { &*process.in_events };
    let count = events.size.map_or(0, |size| unsafe { size(events) });
    let mut done = 0;
    for i in 0..count {
        let Some(header) = events.get.map(|get| unsafe { get(events, i) }).filter(|h| !h.is_null()) else {
            continue;
        };
        let header: &clap_event_header = unsafe { &*header };
        let at = (header.time as usize).min(frames);
        if at > done {
            synth.voices.render(&mut left[done..at], Some(&mut right[done..at]));
            done = at;
        }
        if header.space_id != CLAP_CORE_EVENT_SPACE_ID {
            continue;
        }
        let note = || unsafe { &*(header as *const clap_event_header as *const clap_event_note) };
        match header.type_ {
            CLAP_EVENT_NOTE_ON => synth.voices.press(note().key, note().velocity as f32),
            CLAP_EVENT_NOTE_OFF => synth.voices.release(note().key),
            CLAP_EVENT_PARAM_VALUE => {
                let change =
                    unsafe { &*(header as *const clap_event_header as *const clap_event_param_value) };
                if change.param_id == VOLUME {
                    synth.volume = change.value.clamp(0.0, 1.0);
                }
            }
            _ => {}
        }
    }
    synth.voices.render(&mut left[done..], Some(&mut right[done..]));
    let gain = synth.volume as f32 / DEFAULT_VOLUME as f32;
    left.iter_mut().chain(right.iter_mut()).for_each(|s| *s *= gain);
    CLAP_PROCESS_CONTINUE
}

static AUDIO_PORTS: clap_plugin_audio_ports =
    clap_plugin_audio_ports { count: Some(audio_ports_count), get: Some(audio_ports_get) };

static NOTE_PORTS: clap_plugin_note_ports =
    clap_plugin_note_ports { count: Some(note_ports_count), get: Some(note_ports_get) };

/// The one setting: the volume.
const VOLUME: clap_id = 0;
const DEFAULT_VOLUME: f64 = 0.8;

static PARAMS: clap_plugin_params = clap_plugin_params {
    count: Some(params_count),
    get_info: Some(params_info),
    get_value: Some(params_value),
    value_to_text: None,
    text_to_value: None,
    flush: Some(params_flush),
};

unsafe extern "C" fn params_count(_plugin: *const clap_plugin) -> u32 {
    1
}

unsafe extern "C" fn params_info(
    _plugin: *const clap_plugin,
    index: u32,
    info: *mut clap_param_info,
) -> bool {
    if index != 0 {
        return false;
    }
    let info = unsafe { &mut *info };
    info.id = VOLUME;
    info.flags = CLAP_PARAM_IS_AUTOMATABLE;
    info.cookie = std::ptr::null_mut();
    write_name(&mut info.name, c"Volume");
    write_name(&mut info.module, c"");
    info.min_value = 0.0;
    info.max_value = 1.0;
    info.default_value = DEFAULT_VOLUME;
    true
}

unsafe extern "C" fn params_value(plugin: *const clap_plugin, id: clap_id, value: *mut f64) -> bool {
    if id != VOLUME {
        return false;
    }
    unsafe { *value = synth(plugin).volume };
    true
}

/// Settings changed while nothing plays.
unsafe extern "C" fn params_flush(
    plugin: *const clap_plugin,
    events: *const clap_input_events,
    _out: *const clap_output_events,
) {
    let events = unsafe { &*events };
    let count = events.size.map_or(0, |size| unsafe { size(events) });
    for i in 0..count {
        let Some(header) = events.get.map(|get| unsafe { get(events, i) }).filter(|h| !h.is_null()) else {
            continue;
        };
        if unsafe { (*header).type_ } == CLAP_EVENT_PARAM_VALUE {
            let change = unsafe { &*(header as *const clap_event_param_value) };
            if change.param_id == VOLUME {
                unsafe { synth(plugin) }.volume = change.value.clamp(0.0, 1.0);
            }
        }
    }
}

unsafe extern "C" fn plugin_extension(_plugin: *const clap_plugin, id: *const c_char) -> *const c_void {
    let id = unsafe { CStr::from_ptr(id) };
    if id == CLAP_EXT_PARAMS {
        &PARAMS as *const clap_plugin_params as *const c_void
    } else if id == CLAP_EXT_AUDIO_PORTS {
        &AUDIO_PORTS as *const clap_plugin_audio_ports as *const c_void
    } else if id == CLAP_EXT_NOTE_PORTS {
        &NOTE_PORTS as *const clap_plugin_note_ports as *const c_void
    } else {
        std::ptr::null()
    }
}

/// No audio comes in; one stereo pair goes out.
unsafe extern "C" fn audio_ports_count(_plugin: *const clap_plugin, is_input: bool) -> u32 {
    u32::from(!is_input)
}

unsafe extern "C" fn audio_ports_get(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_audio_port_info,
) -> bool {
    if is_input || index != 0 {
        return false;
    }
    let info = unsafe { &mut *info };
    info.id = 0;
    write_name(&mut info.name, c"Output");
    info.flags = CLAP_AUDIO_PORT_IS_MAIN;
    info.channel_count = 2;
    info.port_type = CLAP_PORT_STEREO.as_ptr();
    info.in_place_pair = CLAP_INVALID_ID;
    true
}

/// Notes come in, in CLAP's own events; none go out.
unsafe extern "C" fn note_ports_count(_plugin: *const clap_plugin, is_input: bool) -> u32 {
    u32::from(is_input)
}

unsafe extern "C" fn note_ports_get(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_note_port_info,
) -> bool {
    if !is_input || index != 0 {
        return false;
    }
    let info = unsafe { &mut *info };
    info.id = 0;
    info.supported_dialects = CLAP_NOTE_DIALECT_CLAP;
    info.preferred_dialect = CLAP_NOTE_DIALECT_CLAP;
    write_name(&mut info.name, c"Notes");
    true
}

fn write_name(to: &mut [c_char], name: &CStr) {
    let bytes = name.to_bytes_with_nul();
    for (to, &from) in to.iter_mut().zip(bytes) {
        *to = from as c_char;
    }
}
