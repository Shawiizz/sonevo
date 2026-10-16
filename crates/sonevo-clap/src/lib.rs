//! CLAP plugins hosted: a bundle (`.clap`) loaded, its plugins listed, one of them made,
//! activated and played, notes and settings in and stereo out.
//!
//! The rules of CLAP's threads hold: a bundle is opened, and an instance made, activated
//! and dropped, on one thread (the main one); [`Instance::process`] runs on the audio
//! thread, allocating nothing.

use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::{
    clap_event_header, clap_event_note, clap_event_param_value, clap_input_events, clap_output_events,
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON, CLAP_EVENT_PARAM_VALUE,
};
use clap_sys::ext::gui::{
    clap_plugin_gui, clap_window, clap_window_handle, CLAP_EXT_GUI, CLAP_WINDOW_API_WIN32,
};
use clap_sys::ext::params::{
    clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS, CLAP_PARAM_IS_HIDDEN, CLAP_PARAM_IS_READONLY,
    CLAP_PARAM_IS_STEPPED,
};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::plugin::clap_plugin;
use clap_sys::process::{clap_process, CLAP_PROCESS_ERROR};
use clap_sys::version::CLAP_VERSION;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use std::sync::Arc;

/// The notes one call to [`Instance::process`] takes at most: more are dropped.
pub const MAX_NOTES: usize = 256;
/// The settings changed in one call to [`Instance::process`] at most: more are dropped.
pub const MAX_PARAMS: usize = 64;

#[derive(Debug)]
pub struct ClapError(String);

impl std::fmt::Display for ClapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ClapError {}

fn fail(what: impl Into<String>) -> ClapError {
    ClapError(what.into())
}

/// A note starting or ending at a frame of the block; laid out as C does, it can be shared
/// with another process.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    /// The frame of the block it falls on.
    pub at: u32,
    pub key: u8,
    /// From 0 to 1.
    pub velocity: f32,
    pub on: bool,
}

/// A setting of the plugin set to `value` from the frame `at` of the block on; laid out as
/// C does, to be shared with another process.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamChange {
    pub at: u32,
    pub id: u32,
    pub value: f64,
}

/// A setting a plugin offers to be moved from outside.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamInfo {
    pub id: u32,
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// Whole steps only: a choice, a switch.
    pub stepped: bool,
}

/// A plugin a bundle offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub features: Vec<String>,
}

/// A `.clap` file loaded: its entry initialised until it is dropped, after every instance.
pub struct Bundle {
    entry: *const clap_plugin_entry,
    factory: *const clap_plugin_factory,
    // dropped last: the entry and the factory live in it
    _library: libloading::Library,
}

// The entry and the factory are only used under CLAP's threading rules, from the main thread.
unsafe impl Send for Bundle {}
unsafe impl Sync for Bundle {}

impl Bundle {
    pub fn open(path: &Path) -> Result<Arc<Bundle>, ClapError> {
        let shown = path.display();
        // SAFETY: a CLAP bundle is a library whose loading runs no code but its own
        // initialisers, which CLAP asks to be harmless
        let library = unsafe { libloading::Library::new(path) }.map_err(|e| fail(format!("{shown}: {e}")))?;
        // SAFETY: `clap_entry` is the one symbol every CLAP bundle exports, of this type
        let entry = unsafe { library.get::<*const clap_plugin_entry>(b"clap_entry\0") }
            .map_err(|_| fail(format!("{shown} is not a CLAP plugin")))?;
        let entry: *const clap_plugin_entry = *entry;
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| fail(e.to_string()))?;
        // SAFETY: the entry stays valid while the library is loaded
        let (init, get_factory) = unsafe { ((*entry).init, (*entry).get_factory) };
        if !init.is_some_and(|init| unsafe { init(path.as_ptr()) }) {
            return Err(fail(format!("{shown} would not start")));
        }
        let factory = get_factory
            .map(|get| unsafe { get(CLAP_PLUGIN_FACTORY_ID.as_ptr()) } as *const clap_plugin_factory)
            .filter(|f| !f.is_null());
        let Some(factory) = factory else {
            if let Some(deinit) = unsafe { (*entry).deinit } {
                unsafe { deinit() };
            }
            return Err(fail(format!("{shown} offers no plugin")));
        };
        Ok(Arc::new(Bundle { entry, factory, _library: library }))
    }

    pub fn plugins(&self) -> Vec<Descriptor> {
        // SAFETY: the factory is valid while the bundle is
        let factory = unsafe { &*self.factory };
        let count = factory.get_plugin_count.map_or(0, |count| unsafe { count(self.factory) });
        (0..count)
            .filter_map(|i| {
                let descriptor = unsafe { factory.get_plugin_descriptor?(self.factory, i) };
                // SAFETY: a descriptor and its strings live as long as the factory
                (!descriptor.is_null()).then(|| unsafe { describe(&*descriptor) })
            })
            .collect()
    }

    /// Makes the plugin `id`, initialised, not active yet.
    pub fn instantiate(self: &Arc<Self>, id: &str) -> Result<Instance, ClapError> {
        let host = Box::new(clap_host {
            clap_version: CLAP_VERSION,
            host_data: std::ptr::null_mut(),
            name: c"Sonevo".as_ptr(),
            vendor: c"Sonevo".as_ptr(),
            url: c"https://github.com/Shawiizz/sonevo".as_ptr(),
            version: c"0.1.0".as_ptr(),
            get_extension: Some(host_extension),
            request_restart: Some(host_request),
            request_process: Some(host_request),
            request_callback: Some(host_request),
        });
        let plugin_id = CString::new(id).map_err(|e| fail(e.to_string()))?;
        // SAFETY: the factory is valid; the host outlives the plugin, which the instance owns
        let plugin = unsafe {
            let create = (*self.factory).create_plugin.ok_or_else(|| fail("the factory makes no plugin"))?;
            create(self.factory, &*host, plugin_id.as_ptr())
        };
        if plugin.is_null() {
            return Err(fail(format!("no plugin {id}")));
        }
        // dropped on a failed start, the plugin is destroyed with it
        let mut instance = Instance {
            plugin,
            state: State::Made,
            params: std::ptr::null(),
            notes: Vec::with_capacity(MAX_NOTES),
            changes: Vec::with_capacity(MAX_PARAMS),
            events: Vec::with_capacity(MAX_NOTES + MAX_PARAMS),
            _host: host,
            _bundle: Arc::clone(self),
        };
        // SAFETY: a plugin made is initialised once, before anything else
        if !unsafe { (*plugin).init.is_some_and(|init| init(plugin)) } {
            return Err(fail(format!("{id} would not start")));
        }
        instance.state = State::Initialised;
        // SAFETY: once initialised, the plugin says which of CLAP's extensions it has
        instance.params = unsafe {
            (*plugin).get_extension.map_or(std::ptr::null(), |get| get(plugin, CLAP_EXT_PARAMS.as_ptr()))
        } as *const clap_plugin_params;
        Ok(instance)
    }
}

impl Drop for Bundle {
    fn drop(&mut self) {
        // SAFETY: every instance holds the bundle: none is left
        if let Some(deinit) = unsafe { (*self.entry).deinit } {
            unsafe { deinit() };
        }
    }
}

unsafe fn describe(d: &clap_sys::plugin::clap_plugin_descriptor) -> Descriptor {
    let text = |s: *const c_char| {
        if s.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
        }
    };
    let mut features = Vec::new();
    let mut at = d.features;
    while !at.is_null() && !unsafe { *at }.is_null() {
        features.push(text(unsafe { *at }));
        at = unsafe { at.add(1) };
    }
    Descriptor { id: text(d.id), name: text(d.name), vendor: text(d.vendor), features }
}

/// Nothing of the host's own is offered yet: no extension.
unsafe extern "C" fn host_extension(_host: *const clap_host, _id: *const c_char) -> *const c_void {
    std::ptr::null()
}

/// The plugin asks for something the host does not do yet: it goes on as it is.
unsafe extern "C" fn host_request(_host: *const clap_host) {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Made,
    Initialised,
    Active,
    Processing,
}

/// A plugin made from a bundle, played block by block once activated.
pub struct Instance {
    plugin: *const clap_plugin,
    state: State,
    /// Its settings, if it lets them be moved from outside.
    params: *const clap_plugin_params,
    /// The notes and the settings of the block being played, made into CLAP's events
    /// without allocating; and every one of them in time order.
    notes: Vec<clap_event_note>,
    changes: Vec<clap_event_param_value>,
    events: Vec<*const clap_event_header>,
    _host: Box<clap_host>,
    _bundle: Arc<Bundle>,
}

// An instance moves to the audio thread once activated; CLAP's rules are kept by its methods.
unsafe impl Send for Instance {}

impl Instance {
    /// Gets the plugin ready to play at `sample_rate`, in blocks of at most `max_frames`.
    pub fn activate(&mut self, sample_rate: f64, max_frames: u32) -> Result<(), ClapError> {
        if self.state != State::Initialised {
            return Err(fail("the plugin is already active"));
        }
        // SAFETY: activated once, from the thread that made it
        let activated =
            unsafe { (*self.plugin).activate.is_some_and(|go| go(self.plugin, sample_rate, 1, max_frames)) };
        if !activated {
            return Err(fail("the plugin would not activate"));
        }
        self.state = State::Active;
        Ok(())
    }

    /// The settings the plugin lets be moved from outside, those hidden or read only left out.
    pub fn params(&self) -> Vec<ParamInfo> {
        if self.params.is_null() {
            return Vec::new();
        }
        // SAFETY: on the main thread; the extension lives as long as the plugin
        let params = unsafe { &*self.params };
        let count = params.count.map_or(0, |count| unsafe { count(self.plugin) });
        (0..count)
            .filter_map(|index| {
                // SAFETY: the info is filled by the plugin before it is read
                let mut info: clap_param_info = unsafe { std::mem::zeroed() };
                let got = params.get_info.is_some_and(|get| unsafe { get(self.plugin, index, &mut info) });
                if !got || info.flags & (CLAP_PARAM_IS_HIDDEN | CLAP_PARAM_IS_READONLY) != 0 {
                    return None;
                }
                let name = unsafe { CStr::from_ptr(info.name.as_ptr()) }.to_string_lossy().into_owned();
                Some(ParamInfo {
                    id: info.id,
                    name,
                    min: info.min_value,
                    max: info.max_value,
                    default: info.default_value,
                    stepped: info.flags & CLAP_PARAM_IS_STEPPED != 0,
                })
            })
            .collect()
    }

    /// Plays one block: the `notes` and the settings `changes` given, each in time order,
    /// sound into `left` and `right`, as long as each other and no longer than the
    /// activation said. False when the plugin failed.
    pub fn process(
        &mut self,
        notes: &[Note],
        changes: &[ParamChange],
        left: &mut [f32],
        right: &mut [f32],
    ) -> bool {
        debug_assert_eq!(left.len(), right.len());
        if self.state == State::Active {
            // SAFETY: on the audio thread, once active
            let started = unsafe { (*self.plugin).start_processing.is_none_or(|start| start(self.plugin)) };
            if !started {
                return false;
            }
            self.state = State::Processing;
        }
        if self.state != State::Processing {
            return false;
        }
        self.notes.clear();
        self.notes.extend(notes.iter().take(MAX_NOTES).map(|note| clap_event_note {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_note>() as u32,
                time: note.at,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: if note.on { CLAP_EVENT_NOTE_ON } else { CLAP_EVENT_NOTE_OFF },
                flags: 0,
            },
            note_id: -1,
            port_index: 0,
            channel: 0,
            key: note.key as i16,
            velocity: f64::from(note.velocity),
        }));
        self.changes.clear();
        self.changes.extend(changes.iter().take(MAX_PARAMS).map(|change| clap_event_param_value {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_param_value>() as u32,
                time: change.at,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_PARAM_VALUE,
                flags: 0,
            },
            param_id: change.id,
            cookie: std::ptr::null_mut(),
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value: change.value,
        }));
        // the two lists, each in time order, merged into one as CLAP wants: a setting
        // before a note of the same frame
        self.events.clear();
        let (mut n, mut c) = (0, 0);
        while n < self.notes.len() || c < self.changes.len() {
            let change_first = c < self.changes.len()
                && (n == self.notes.len() || self.changes[c].header.time <= self.notes[n].header.time);
            if change_first {
                self.events.push(&self.changes[c].header);
                c += 1;
            } else {
                self.events.push(&self.notes[n].header);
                n += 1;
            }
        }
        let input = clap_input_events {
            ctx: &self.events as *const Vec<*const clap_event_header> as *mut c_void,
            size: Some(events_size),
            get: Some(events_get),
        };
        let output = clap_output_events { ctx: std::ptr::null_mut(), try_push: Some(events_push) };
        let mut channels = [left.as_mut_ptr(), right.as_mut_ptr()];
        let mut out = clap_audio_buffer {
            data32: channels.as_mut_ptr(),
            data64: std::ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let process = clap_process {
            steady_time: -1,
            frames_count: left.len() as u32,
            transport: std::ptr::null(),
            audio_inputs: std::ptr::null(),
            audio_outputs: &mut out,
            audio_inputs_count: 0,
            audio_outputs_count: 1,
            in_events: &input,
            out_events: &output,
        };
        // SAFETY: on the audio thread, processing; every pointer outlives the call
        let status =
            unsafe { (*self.plugin).process.map_or(CLAP_PROCESS_ERROR, |go| go(self.plugin, &process)) };
        status != CLAP_PROCESS_ERROR
    }
}

/// A plugin's own window, opened and closed on the main thread while its instance plays on
/// the audio one. Closed before the instance is dropped.
pub struct Gui {
    plugin: *const clap_plugin,
    gui: *const clap_plugin_gui,
    open: bool,
}

// Used on the main thread only, as CLAP wants of its window's calls.
unsafe impl Send for Gui {}

impl Instance {
    /// The plugin's window, if it has one Windows can show.
    pub fn gui(&self) -> Option<Gui> {
        // SAFETY: on the main thread, the plugin initialised
        let gui = unsafe {
            (*self.plugin)
                .get_extension
                .map_or(std::ptr::null(), |get| get(self.plugin, CLAP_EXT_GUI.as_ptr()))
        } as *const clap_plugin_gui;
        if gui.is_null() {
            return None;
        }
        // SAFETY: the extension lives as long as the plugin
        let supported = unsafe {
            (*gui).is_api_supported.is_some_and(|ok| ok(self.plugin, CLAP_WINDOW_API_WIN32.as_ptr(), false))
        };
        supported.then_some(Gui { plugin: self.plugin, gui, open: false })
    }
}

impl Gui {
    /// Shows the plugin's window inside `parent`, a window of Windows: its size, to make the
    /// parent fit it.
    pub fn open(&mut self, parent: *mut c_void) -> Result<(u32, u32), ClapError> {
        if self.open {
            return Err(fail("the window is open already"));
        }
        // SAFETY: on the main thread; every pointer outlives the calls
        unsafe {
            let gui = &*self.gui;
            if !gui.create.is_some_and(|create| create(self.plugin, CLAP_WINDOW_API_WIN32.as_ptr(), false)) {
                return Err(fail("the plugin made no window"));
            }
            self.open = true;
            let (mut width, mut height) = (0u32, 0u32);
            if !gui.get_size.is_some_and(|size| size(self.plugin, &mut width, &mut height)) {
                (width, height) = (640, 400);
            }
            let window = clap_window {
                api: CLAP_WINDOW_API_WIN32.as_ptr(),
                specific: clap_window_handle { win32: parent },
            };
            if !gui.set_parent.is_some_and(|set| set(self.plugin, &window)) {
                self.close();
                return Err(fail("the plugin's window could not be placed"));
            }
            if let Some(show) = gui.show {
                show(self.plugin);
            }
            Ok((width, height))
        }
    }

    /// Hides and destroys the plugin's window, if open.
    pub fn close(&mut self) {
        if !std::mem::take(&mut self.open) {
            return;
        }
        // SAFETY: on the main thread, the window made
        unsafe {
            let gui = &*self.gui;
            if let Some(hide) = gui.hide {
                hide(self.plugin);
            }
            if let Some(destroy) = gui.destroy {
                destroy(self.plugin);
            }
        }
    }
}

impl Instance {
    /// Ends the processing, on the audio thread, before the instance goes back to the main
    /// one to be dropped.
    pub fn stop(&mut self) {
        if self.state == State::Processing {
            // SAFETY: on the audio thread, processing
            if let Some(stop) = unsafe { (*self.plugin).stop_processing } {
                unsafe { stop(self.plugin) };
            }
            self.state = State::Active;
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: from the thread that made it, back down the states it went through
        unsafe {
            let plugin = &*self.plugin;
            if self.state == State::Processing {
                if let Some(stop) = plugin.stop_processing {
                    stop(self.plugin);
                }
            }
            if matches!(self.state, State::Active | State::Processing) {
                if let Some(deactivate) = plugin.deactivate {
                    deactivate(self.plugin);
                }
            }
            if let Some(destroy) = plugin.destroy {
                destroy(self.plugin);
            }
        }
    }
}

unsafe extern "C" fn events_size(list: *const clap_input_events) -> u32 {
    let events = unsafe { &*((*list).ctx as *const Vec<*const clap_event_header>) };
    events.len() as u32
}

unsafe extern "C" fn events_get(list: *const clap_input_events, index: u32) -> *const clap_event_header {
    let events = unsafe { &*((*list).ctx as *const Vec<*const clap_event_header>) };
    events.get(index as usize).copied().unwrap_or(std::ptr::null())
}

/// What the plugin sends back (its own notes, its parameters moved) is not listened to yet.
unsafe extern "C" fn events_push(_list: *const clap_output_events, _event: *const clap_event_header) -> bool {
    true
}
