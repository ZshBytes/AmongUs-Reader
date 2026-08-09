use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, LoadIconW, SetLayeredWindowAttributes, SetWindowDisplayAffinity,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST, IDI_APPLICATION,
    LWA_ALPHA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOW, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    WINDOW_EX_STYLE, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
};

/// Apply layered transparency, topmost z-order, taskbar hiding, and capture exclusion.
pub fn apply_stream_proof_styles(window: &Window) {
    let hwnd = window_hwnd(window);

    unsafe {
        SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
            .expect("SetWindowDisplayAffinity failed");

        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // WS_EX_TOOLWINDOW hides window from taskbar, ~WS_EX_APPWINDOW ensures taskbar exclusion
        let new_style = (ex_style | (WS_EX_LAYERED.0 as isize) | (WS_EX_TOPMOST.0 as isize) | (WS_EX_TOOLWINDOW.0 as isize))
            & !(WS_EX_APPWINDOW.0 as isize);

        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)
            .expect("SetLayeredWindowAttributes failed");

        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }
}

/// Dynamically toggle Stream-Proof capture exclusion (ON/OFF)
pub fn set_stream_proof(window: &Window, enabled: bool) {
    let hwnd = window_hwnd(window);
    let affinity = if enabled {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };
    unsafe {
        let _ = SetWindowDisplayAffinity(hwnd, affinity);
    }
}

pub struct SystemTray {
    nid: NOTIFYICONDATAW,
}

impl SystemTray {
    pub fn create(window: &Window) -> Option<Self> {
        let hwnd = window_hwnd(window);
        unsafe {
            let icon = LoadIconW(None, IDI_APPLICATION).ok()?;
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1001;
            nid.uFlags = NIF_ICON | NIF_TIP | NIF_MESSAGE;
            nid.uCallbackMessage = 0x0400 + 1; // WM_USER + 1
            nid.hIcon = icon;

            let tip = "Among Us Live Overlay\0".encode_utf16().collect::<Vec<_>>();
            let len = tip.len().min(nid.szTip.len());
            nid.szTip[..len].copy_from_slice(&tip[..len]);

            if Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                Some(Self { nid })
            } else {
                None
            }
        }
    }
}

impl Drop for SystemTray {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
        }
    }
}

fn window_hwnd(window: &Window) -> HWND {
    match window.window_handle().expect("window handle").as_raw() {
        RawWindowHandle::Win32(handle) => HWND(handle.hwnd.get() as _),
        _ => panic!("overlay requires a Win32 window"),
    }
}

const WS_EX_TOPMOST: WINDOW_EX_STYLE = WINDOW_EX_STYLE(0x0000_0008);
