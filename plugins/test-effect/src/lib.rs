//! A small CLAP effect, to try Sonevo's hosting of effects with: the sound that comes in goes
//! out louder or quieter, by a gain the host moves.

use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::{
    clap_event_header, clap_event_param_value, clap_input_events, clap_output_events,
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_PARAM_VALUE,
};
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_AUDIO_PORT_IS_MAIN, CLAP_EXT_AUDIO_PORTS,
    CLAP_PORT_STEREO,
};
use clap_sys::ext::params::{
    clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS, CLAP_PARAM_IS_AUTOMATABLE,
};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::id::{clap_id, CLAP_INVALID_ID};
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::plugin_features::{CLAP_PLUGIN_FEATURE_AUDIO_EFFECT, CLAP_PLUGIN_FEATURE_UTILITY};
use clap_sys::process::{clap_process, clap_process_status, CLAP_PROCESS_CONTINUE};
use clap_sys::version::CLAP_VERSION;
use std::ffi::{c_char, c_void, CStr};

const ID: &CStr = c"dev.sonevo.test-effect";

/// The features, ended by a null as CLAP wants: pointers to static strings, shared freely.
struct Features([*const c_char; 3]);
unsafe impl Sync for Features {}

static FEATURES: Features = Features([
    CLAP_PLUGIN_FEATURE_AUDIO_EFFECT.as_ptr(),
    CLAP_PLUGIN_FEATURE_UTILITY.as_ptr(),
    std::ptr::null(),
]);

static DESCRIPTOR: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: ID.as_ptr(),
    name: c"Sonevo Test Gain".as_ptr(),
    vendor: c"Sonevo".as_ptr(),
    url: c"https://github.com/Shawiizz/sonevo".as_ptr(),
    manual_url: c"".as_ptr(),
    support_url: c"".as_ptr(),
    version: c"0.1.0".as_ptr(),
    description: c"A gain, to try hosting effects with".as_ptr(),
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
struct Gain {
    plugin: clap_plugin,
    /// The setting `GAIN`: what the sound is multiplied by, from 0 to 2.
    gain: f64,
}

unsafe extern "C" fn factory_create(
    _factory: *const clap_plugin_factory,
    _host: *const clap_host,
    id: *const c_char,
) -> *const clap_plugin {
    if unsafe { CStr::from_ptr(id) } != ID {
        return std::ptr::null();
    }
    let effect = Box::new(Gain {
        plugin: clap_plugin {
            desc: &DESCRIPTOR,
            plugin_data: std::ptr::null_mut(),
            init: Some(plugin_yes),
            destroy: Some(plugin_destroy),
            activate: Some(plugin_activate),
            deactivate: Some(plugin_nothing),
            start_processing: Some(plugin_yes),
            stop_processing: Some(plugin_nothing),
            reset: Some(plugin_nothing),
            process: Some(plugin_process),
            get_extension: Some(plugin_extension),
            on_main_thread: Some(plugin_nothing),
        },
        gain: DEFAULT_GAIN,
    });
    Box::into_raw(effect) as *const clap_plugin
}

/// The effect behind the plugin CLAP hands back.
///
/// # Safety
/// `plugin` is one [`factory_create`] made and [`plugin_destroy`] has not freed.
unsafe fn effect<'a>(plugin: *const clap_plugin) -> &'a mut Gain {
    unsafe { &mut *(plugin as *mut Gain) }
}

unsafe extern "C" fn plugin_yes(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn plugin_nothing(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_destroy(plugin: *const clap_plugin) {
    drop(unsafe { Box::from_raw(plugin as *mut Gain) });
}

unsafe extern "C" fn plugin_activate(_plugin: *const clap_plugin, _rate: f64, _min: u32, _max: u32) -> bool {
    true
}

/// The setting changed by `header`, if it is a change of it.
fn apply(effect: &mut Gain, header: &clap_event_header) {
    if header.space_id == CLAP_CORE_EVENT_SPACE_ID && header.type_ == CLAP_EVENT_PARAM_VALUE {
        let change = unsafe { &*(header as *const clap_event_header as *const clap_event_param_value) };
        if change.param_id == GAIN {
            effect.gain = change.value.clamp(0.0, 2.0);
        }
    }
}

fn each_event(events: &clap_input_events, mut seen: impl FnMut(&clap_event_header)) {
    let count = events.size.map_or(0, |size| unsafe { size(events) });
    for i in 0..count {
        if let Some(header) = events.get.map(|get| unsafe { get(events, i) }).filter(|h| !h.is_null()) {
            seen(unsafe { &*header });
        }
    }
}

unsafe extern "C" fn plugin_process(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    let effect = unsafe { effect(plugin) };
    let process = unsafe { &*process };
    each_event(unsafe { &*process.in_events }, |header| apply(effect, header));
    let frames = process.frames_count as usize;
    if process.audio_inputs_count == 0 || process.audio_outputs_count == 0 || frames == 0 {
        return CLAP_PROCESS_CONTINUE;
    }
    let (input, output) = unsafe { (&*process.audio_inputs, &*process.audio_outputs) };
    let ins = unsafe { std::slice::from_raw_parts(input.data32, input.channel_count as usize) };
    let outs = unsafe { std::slice::from_raw_parts(output.data32, output.channel_count as usize) };
    let gain = effect.gain as f32;
    for (c, &out) in outs.iter().enumerate() {
        // a mono input goes to both sides
        let Some(&from) = ins.get(c).or(ins.first()) else { continue };
        let (from, out) = unsafe {
            (std::slice::from_raw_parts(from, frames), std::slice::from_raw_parts_mut(out, frames))
        };
        for (o, i) in out.iter_mut().zip(from) {
            *o = i * gain;
        }
    }
    CLAP_PROCESS_CONTINUE
}

static AUDIO_PORTS: clap_plugin_audio_ports =
    clap_plugin_audio_ports { count: Some(audio_ports_count), get: Some(audio_ports_get) };

/// The one setting: the gain.
const GAIN: clap_id = 0;
const DEFAULT_GAIN: f64 = 0.5;

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
    info.id = GAIN;
    info.flags = CLAP_PARAM_IS_AUTOMATABLE;
    info.cookie = std::ptr::null_mut();
    write_name(&mut info.name, c"Gain");
    write_name(&mut info.module, c"");
    info.min_value = 0.0;
    info.max_value = 2.0;
    info.default_value = DEFAULT_GAIN;
    true
}

unsafe extern "C" fn params_value(plugin: *const clap_plugin, id: clap_id, value: *mut f64) -> bool {
    if id != GAIN {
        return false;
    }
    unsafe { *value = effect(plugin).gain };
    true
}

/// Settings changed while nothing plays.
unsafe extern "C" fn params_flush(
    plugin: *const clap_plugin,
    events: *const clap_input_events,
    _out: *const clap_output_events,
) {
    let effect = unsafe { effect(plugin) };
    each_event(unsafe { &*events }, |header| apply(effect, header));
}

unsafe extern "C" fn plugin_extension(_plugin: *const clap_plugin, id: *const c_char) -> *const c_void {
    let id = unsafe { CStr::from_ptr(id) };
    if id == CLAP_EXT_PARAMS {
        &PARAMS as *const clap_plugin_params as *const c_void
    } else if id == CLAP_EXT_AUDIO_PORTS {
        &AUDIO_PORTS as *const clap_plugin_audio_ports as *const c_void
    } else {
        std::ptr::null()
    }
}

/// One stereo pair comes in, one goes out.
unsafe extern "C" fn audio_ports_count(_plugin: *const clap_plugin, _is_input: bool) -> u32 {
    1
}

unsafe extern "C" fn audio_ports_get(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_audio_port_info,
) -> bool {
    if index != 0 {
        return false;
    }
    let info = unsafe { &mut *info };
    info.id = 0;
    write_name(&mut info.name, if is_input { c"Input" } else { c"Output" });
    info.flags = CLAP_AUDIO_PORT_IS_MAIN;
    info.channel_count = 2;
    info.port_type = CLAP_PORT_STEREO.as_ptr();
    info.in_place_pair = CLAP_INVALID_ID;
    true
}

fn write_name(to: &mut [c_char], name: &CStr) {
    let bytes = name.to_bytes_with_nul();
    for (to, &from) in to.iter_mut().zip(bytes) {
        *to = from as c_char;
    }
}
