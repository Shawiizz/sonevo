//! The plugin's window, in a window of this process: opened when the engine's process asks
//! through a named event, closed by its own close button, on the main thread as CLAP wants.

use sonevo_clap::Gui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, LPARAM, LRESULT, RECT, WAIT_OBJECT_0, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
    RegisterClassW, SetForegroundWindow, SetWindowPos, ShowWindow, TranslateMessage, CW_USEDEFAULT, MSG,
    PM_REMOVE, SWP_NOMOVE, SWP_NOZORDER, SW_SHOW, WM_CLOSE, WNDCLASSW, WS_CAPTION, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_SYSMENU,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

/// Its close button was pressed.
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

/// The plugin's window, and the event that asks for it.
pub struct Windows {
    asked: HANDLE,
    gui: Gui,
    title: String,
    window: HWND,
}

impl Windows {
    /// Listens for the event `name` asking for the plugin's window, titled `title`.
    pub fn new(name: &str, gui: Gui, title: &str) -> Option<Windows> {
        let name = wide(&format!("Local\\{name}"));
        // SAFETY: a plain Win32 call, the name living through it
        let asked = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
        (!asked.is_null()).then(|| Windows {
            asked,
            gui,
            title: title.to_owned(),
            window: std::ptr::null_mut(),
        })
    }

    /// Waits up to `time` for anything to do, and does it: the window opened or brought to
    /// the front when asked, closed when its button was pressed, its messages handled.
    pub fn serve(&mut self, time: Duration) {
        let wait = if self.window.is_null() { time } else { Duration::from_millis(8) };
        // SAFETY: the event is open
        if unsafe { WaitForSingleObject(self.asked, wait.as_millis() as u32) } == WAIT_OBJECT_0 {
            self.show();
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

    fn show(&mut self) {
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
            match self.gui.open(window) {
                Ok((width, height)) => {
                    // the window made as large as the plugin's, its frame around it
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
        self.gui.close();
        if !self.window.is_null() {
            // SAFETY: the window is ours
            unsafe { DestroyWindow(self.window) };
            self.window = std::ptr::null_mut();
        }
    }
}

impl Drop for Windows {
    fn drop(&mut self) {
        self.close();
        // SAFETY: the event is ours
        unsafe { CloseHandle(self.asked) };
    }
}
