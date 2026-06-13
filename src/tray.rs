use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::Ordering;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconGetRect, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD,
    NIM_DELETE, NOTIFYICONDATAW, NOTIFYICONIDENTIFIER,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIcon, CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW,
    SetForegroundWindow, TrackPopupMenu, HICON, HMENU, MENU_ITEM_FLAGS, MF_CHECKED, MF_SEPARATOR,
    MF_STRING, MF_UNCHECKED, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WM_APP, WM_NULL,
};

use crate::capture::FULLSCREEN;
use crate::startup::is_startup_enabled;
use crate::util::wide;

pub const WM_TRAYICON: u32 = WM_APP + 1;
pub const TRAY_ID: u32 = 1;
pub const MENU_EXIT: u32 = 1001;
pub const MENU_AREA: u32 = 1002;
pub const MENU_FULLSCREEN: u32 = 1003;
pub const MENU_STARTUP: u32 = 1004;
pub const MENU_OPEN_IMAGE: u32 = 1005;

pub const COLOR_ORANGE: [u8; 4] = [0xD4, 0x57, 0x0A, 0xFF];

pub fn make_circle_icon(rgba: [u8; 4]) -> HICON {
    let size: u32 = 16;
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    let r = (size as f32 / 2.0) - 1.0;

    let mut and_bits = vec![0xFFu8; (size * size / 8) as usize];
    let mut xor_bits = vec![0u32; (size * size) as usize];

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r * r {
                let idx = (y * size + x) as usize;
                let byte = idx / 8;
                let bit = 7 - (idx % 8);
                and_bits[byte] &= !(1 << bit);
                xor_bits[idx] = u32::from_le_bytes([rgba[2], rgba[1], rgba[0], 0xFF]);
            }
        }
    }

    unsafe {
        CreateIcon(
            None,
            size as i32,
            size as i32,
            1,
            32,
            and_bits.as_ptr(),
            xor_bits.as_ptr() as *const u8,
        )
        .unwrap_or_default()
    }
}

fn fill_tip(tooltip: &str) -> [u16; 128] {
    let mut tip = [0u16; 128];
    let w: Vec<u16> = OsStr::new(tooltip).encode_wide().collect();
    let len = w.len().min(127);
    tip[..len].copy_from_slice(&w[..len]);
    tip
}

fn notify_data(hwnd: HWND, icon: HICON, tooltip: &str) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: icon,
        szTip: fill_tip(tooltip),
        ..Default::default()
    }
}

pub fn add_tray_icon(hwnd: HWND, icon: HICON, tooltip: &str) {
    let nid = notify_data(hwnd, icon, tooltip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

pub fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

unsafe fn append_item(menu: HMENU, flags: MENU_ITEM_FLAGS, id: u32, text: &str) {
    let label = wide(text);
    let _ = AppendMenuW(menu, flags, id as usize, PCWSTR(label.as_ptr()));
}

unsafe fn append_separator(menu: HMENU) {
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
}

fn check_flag(checked: bool) -> MENU_ITEM_FLAGS {
    if checked {
        MF_CHECKED
    } else {
        MF_UNCHECKED
    }
}

pub fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let nii = NOTIFYICONIDENTIFIER {
            cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            ..Default::default()
        };
        let anchor = match Shell_NotifyIconGetRect(&nii) {
            Ok(RECT { right, top, .. }) => POINT { x: right, y: top },
            Err(_) => {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                pt
            }
        };

        let Ok(menu) = CreatePopupMenu() else {
            return;
        };

        let fullscreen = FULLSCREEN.load(Ordering::Relaxed);
        let startup = is_startup_enabled();

        append_item(
            menu,
            MF_STRING | check_flag(!fullscreen),
            MENU_AREA,
            "Area screenshot",
        );
        append_item(
            menu,
            MF_STRING | check_flag(fullscreen),
            MENU_FULLSCREEN,
            "Fullscreen",
        );
        append_separator(menu);
        append_item(
            menu,
            MF_STRING | check_flag(startup),
            MENU_STARTUP,
            "Run at startup",
        );
        append_item(menu, MF_STRING, MENU_OPEN_IMAGE, "Open image");
        append_separator(menu);
        append_item(menu, MF_STRING, MENU_EXIT, "Exit");

        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
            anchor.x,
            anchor.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}
