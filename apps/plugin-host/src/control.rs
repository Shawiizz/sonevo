//! What the engine's process asks of the plugin besides playing, through named events, done
//! on the main thread as CLAP wants: its window, in a window of this process, closed by its
//! own close button; its state, saved to a file for the engine's process to take.

use sonevo_clap::{Gui, Keeper};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, LPARAM, LRESULT, RECT, WAIT_OBJECT_0, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent, WaitForMultipleObjects};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
    RegisterClassW, SetForegroundWindow, SetWindowPos, ShowWindow, TranslateMessage, CW_USEDEFAULT, MSG,
    PM_REMOVE, SWP_NOMOVE, SWP_NOZORDER, SW_SHOW, WM_CLOSE, WNDCLASSW, WS_CAPTION, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_SYSMENU,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

static CLOSED: AtomicBool = AtomicBool::new(false);
static CLASS: Once = Once::new();
const CLASS_NAME: &str = "SonevoPluginWindow";
const STYLE: u32 = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;

unsafe extern "system" fn handle(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message == WM_CLOSE {
        CLOSED.store(true, Ordering::Release);
        return 0;
    }
    unsafe { DefWindowProcW(window, message, w, l) }
}

/// The events the engine's process sets: `<name>-window` for the window, `<name>-save` for
/// the state, answered by `<name>-saved` once the state is in `<temp>/<name>.state`.
pub struct Control {
    /// The window and the save asked for, waited on together.
    asked: [HANDLE; 2],
    saved: HANDLE,
    gui: Option<Gui>,
    keeper: Option<Keeper>,
    state: PathBuf,
    title: String,
    window: HWND,
}

fn event(name: &str) -> HANDLE {
    let name = wide(&format!("Local\\{name}"));
    // SAFETY: a plain Win32 call, the name living through it
    unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) }
}

impl Control {
    pub fn new(name: &str, gui: Option<Gui>, keeper: Option<Keeper>, title: &str) -> Option<Control> {
        let asked = [event(&format!("{name}-window")), event(&format!("{name}-save"))];
        let saved = event(&format!("{name}-saved"));
        let control = Control {
            asked,
            saved,
            gui,
            keeper,
            state: std::env::temp_dir().join(format!("{name}.state")),
            title: title.to_owned(),
            window: std::ptr::null_mut(),
        };
        // closed by its drop, should one fail
        let opened = !asked.iter().chain([&saved]).any(|e| e.is_null());
        opened.then_some(control)
    }

    pub fn serve(&mut self, time: Duration) {
        let wait = if self.window.is_null() { time } else { Duration::from_millis(8) };
        // SAFETY: the events are open
        match unsafe { WaitForMultipleObjects(2, self.asked.as_ptr(), 0, wait.as_millis() as u32) } {
            w if w == WAIT_OBJECT_0 => self.show(),
            w if w == WAIT_OBJECT_0 + 1 => self.save(),
            _ => {}
        }
        if CLOSED.swap(false, Ordering::AcqRel) {
            self.close();
        }
        // SAFETY: this thread's messages, handled as Windows wants
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    fn save(&mut self) {
        let bytes = self.keeper.as_ref().and_then(Keeper::save);
        let written = bytes.is_some_and(|bytes| {
            let part = self.state.with_extension("part");
            std::fs::write(&part, bytes).and_then(|()| std::fs::rename(&part, &self.state)).is_ok()
        });
        if !written {
            let _ = std::fs::remove_file(&self.state);
        }
        // SAFETY: the event is open
        unsafe { SetEvent(self.saved) };
    }

    fn show(&mut self) {
        let Some(gui) = &mut self.gui else { return };
        if !self.window.is_null() {
            // SAFETY: the window is ours
            unsafe { SetForegroundWindow(self.window) };
            return;
        }
        let class = wide(CLASS_NAME);
        let title = wide(&self.title);
        // SAFETY: plain Win32 calls, every pointer living through them
        unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            CLASS.call_once(|| {
                let mut description: WNDCLASSW = std::mem::zeroed();
                description.lpfnWndProc = Some(handle);
                description.hInstance = module;
                description.lpszClassName = class.as_ptr();
                RegisterClassW(&description);
            });
            let window = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                STYLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                640,
                400,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                module,
                std::ptr::null(),
            );
            if window.is_null() {
                return;
            }
            match gui.open(window) {
                Ok((width, height)) => {
                    let mut frame = RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 };
                    AdjustWindowRectEx(&mut frame, STYLE, 0, 0);
                    let (w, h) = (frame.right - frame.left, frame.bottom - frame.top);
                    SetWindowPos(window, std::ptr::null_mut(), 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER);
                    ShowWindow(window, SW_SHOW);
                    SetForegroundWindow(window);
                    self.window = window;
                }
                Err(e) => {
                    eprintln!("sonevo-plugin-host: {e}");
                    DestroyWindow(window);
                }
            }
        }
    }

    fn close(&mut self) {
        if let Some(gui) = &mut self.gui {
            gui.close();
        }
        if !self.window.is_null() {
            // SAFETY: the window is ours
            unsafe { DestroyWindow(self.window) };
            self.window = std::ptr::null_mut();
        }
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        self.close();
        let _ = std::fs::remove_file(&self.state);
        // SAFETY: the events are ours
        for event in self.asked.into_iter().chain([self.saved]).filter(|e| !e.is_null()) {
            unsafe { CloseHandle(event) };
        }
    }
}
