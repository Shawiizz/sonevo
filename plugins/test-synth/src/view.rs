//! The synth's own window, as CLAP's gui extension shows it: a small panel naming it, placed
//! in the window the host gives.

use clap_sys::ext::gui::{clap_gui_resize_hints, clap_plugin_gui, clap_window, CLAP_WINDOW_API_WIN32};
use clap_sys::plugin::clap_plugin;
use std::ffi::{c_char, CStr};
use std::sync::Once;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode, SetTextColor, TextOutW,
    PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, RegisterClassW, ShowWindow, SW_HIDE,
    SW_SHOW, WM_PAINT, WNDCLASSW, WS_CHILD, WS_VISIBLE,
};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 160;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

pub static GUI: clap_plugin_gui = clap_plugin_gui {
    is_api_supported: Some(supported),
    get_preferred_api: Some(preferred),
    create: Some(create),
    destroy: Some(destroy),
    set_scale: Some(no_scale),
    get_size: Some(size),
    can_resize: Some(no_resize),
    get_resize_hints: Some(no_hints),
    adjust_size: Some(no_adjust),
    set_size: Some(no_set_size),
    set_parent: Some(set_parent),
    set_transient: Some(no_transient),
    suggest_title: Some(no_title),
    show: Some(show),
    hide: Some(hide),
};

/// The synth's window, once placed: none before.
fn view(plugin: *const clap_plugin) -> &'static mut HWND {
    &mut unsafe { crate::synth(plugin) }.view
}

unsafe extern "C" fn supported(_plugin: *const clap_plugin, api: *const c_char, floating: bool) -> bool {
    !floating && unsafe { CStr::from_ptr(api) } == CLAP_WINDOW_API_WIN32
}

unsafe extern "C" fn preferred(
    _plugin: *const clap_plugin,
    api: *mut *const c_char,
    floating: *mut bool,
) -> bool {
    unsafe {
        *api = CLAP_WINDOW_API_WIN32.as_ptr();
        *floating = false;
    }
    true
}

unsafe extern "C" fn create(_plugin: *const clap_plugin, api: *const c_char, floating: bool) -> bool {
    unsafe { supported(_plugin, api, floating) }
}

unsafe extern "C" fn destroy(plugin: *const clap_plugin) {
    let view = view(plugin);
    if !view.is_null() {
        unsafe { DestroyWindow(*view) };
        *view = std::ptr::null_mut();
    }
}

unsafe extern "C" fn no_scale(_plugin: *const clap_plugin, _scale: f64) -> bool {
    false
}

unsafe extern "C" fn size(_plugin: *const clap_plugin, width: *mut u32, height: *mut u32) -> bool {
    unsafe {
        *width = WIDTH;
        *height = HEIGHT;
    }
    true
}

unsafe extern "C" fn no_resize(_plugin: *const clap_plugin) -> bool {
    false
}

unsafe extern "C" fn no_hints(_plugin: *const clap_plugin, _hints: *mut clap_gui_resize_hints) -> bool {
    false
}

unsafe extern "C" fn no_adjust(_plugin: *const clap_plugin, _width: *mut u32, _height: *mut u32) -> bool {
    false
}

unsafe extern "C" fn no_set_size(_plugin: *const clap_plugin, _width: u32, _height: u32) -> bool {
    false
}

unsafe extern "C" fn no_transient(_plugin: *const clap_plugin, _window: *const clap_window) -> bool {
    false
}

unsafe extern "C" fn no_title(_plugin: *const clap_plugin, _title: *const c_char) {}

static CLASS: Once = Once::new();
const CLASS_NAME: &str = "SonevoTestSynthView";

unsafe extern "C" fn set_parent(plugin: *const clap_plugin, window: *const clap_window) -> bool {
    let parent = unsafe { (*window).specific.win32 };
    let class = wide(CLASS_NAME);
    let module = unsafe { GetModuleHandleW(std::ptr::null()) };
    CLASS.call_once(|| {
        let mut description: WNDCLASSW = unsafe { std::mem::zeroed() };
        description.lpfnWndProc = Some(paint);
        description.hInstance = module;
        description.lpszClassName = class.as_ptr();
        unsafe { RegisterClassW(&description) };
    });
    let made = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            WIDTH as i32,
            HEIGHT as i32,
            parent,
            std::ptr::null_mut(),
            module,
            std::ptr::null(),
        )
    };
    *view(plugin) = made;
    !made.is_null()
}

unsafe extern "C" fn show(plugin: *const clap_plugin) -> bool {
    let view = *view(plugin);
    !view.is_null() && {
        unsafe { ShowWindow(view, SW_SHOW) };
        true
    }
}

unsafe extern "C" fn hide(plugin: *const clap_plugin) -> bool {
    let view = *view(plugin);
    !view.is_null() && {
        unsafe { ShowWindow(view, SW_HIDE) };
        true
    }
}

/// Draws the panel: a dark ground, the synth's name.
unsafe extern "system" fn paint(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message != WM_PAINT {
        return unsafe { DefWindowProcW(window, message, w, l) };
    }
    unsafe {
        let mut paint: PAINTSTRUCT = std::mem::zeroed();
        let dc = BeginPaint(window, &mut paint);
        let mut area: RECT = std::mem::zeroed();
        GetClientRect(window, &mut area);
        let ground = CreateSolidBrush(0x001b_1614);
        FillRect(dc, &area, ground);
        DeleteObject(ground);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x00f6_f2f0);
        for (i, line) in
            ["Sonevo Test Synth", "A window of the plugin's own,", "shown by Sonevo."].iter().enumerate()
        {
            let text: Vec<u16> = line.encode_utf16().collect();
            TextOutW(dc, 24, 24 + i as i32 * 26, text.as_ptr(), text.len() as i32);
        }
        EndPaint(window, &paint);
    }
    0
}
