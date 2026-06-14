#![windows_subsystem = "windows"]

mod capture;
mod clipboard;
mod shortcut;
mod startup;
mod tray;
mod util;

use std::sync::atomic::Ordering;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, MOD_CONTROL, MOD_NOREPEAT, VK_SNAPSHOT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DispatchMessageW, GetMessageW,
    LoadCursorW, PostQuitMessage, RegisterClassExW, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, IDC_ARROW, MSG, WM_COMMAND, WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_RBUTTONUP,
    WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};

use crate::capture::{copy_last_path, open_images_folder, start_capture, FULLSCREEN};
use crate::startup::{is_startup_enabled, register_startup, set_startup};
use crate::tray::{
    add_tray_icon, copy_recent_image_path, make_circle_icon, remove_tray_icon, show_tray_menu,
    COLOR_ORANGE, MAX_RECENT_IMAGES, MENU_AREA, MENU_EXIT, MENU_FULLSCREEN, MENU_IMAGE_BASE,
    MENU_OPEN_FOLDER, MENU_STARTUP, WM_TRAYICON,
};
use crate::util::wide;

const HOTKEY_CTRL_PRTSC: i32 = 1;

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            let id = wparam.0 as i32;
            if id == HOTKEY_CTRL_PRTSC {
                start_capture();
            }
            LRESULT(0)
        }
        WM_TRAYICON => {
            let event = lparam.0 as u32 & 0xFFFF;
            if event == WM_LBUTTONUP {
                copy_last_path();
            } else if event == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 as u32 & 0xFFFF {
                MENU_EXIT => {
                    let _ = DestroyWindow(hwnd);
                }
                MENU_AREA => {
                    FULLSCREEN.store(false, Ordering::Relaxed);
                }
                MENU_FULLSCREEN => {
                    FULLSCREEN.store(true, Ordering::Relaxed);
                }
                MENU_STARTUP => {
                    set_startup(!is_startup_enabled());
                }
                MENU_OPEN_FOLDER => {
                    open_images_folder();
                }
                id if (MENU_IMAGE_BASE..MENU_IMAGE_BASE + MAX_RECENT_IMAGES).contains(&id) => {
                    copy_recent_image_path(id);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            remove_tray_icon(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    let _mutex = unsafe {
        let name = wide("shot2path_singleton_mutex");
        match CreateMutexW(None, true, PCWSTR(name.as_ptr())) {
            Ok(handle) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    return;
                }
                handle
            }
            Err(_) => return,
        }
    };

    register_startup();
    shortcut::create_desktop_shortcut();

    unsafe {
        let hmodule: HMODULE = GetModuleHandleW(None).unwrap_or_default();
        let hinstance = HINSTANCE(hmodule.0);
        let class_name = wide("shot2path_wnd");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(wide("shot2path").as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(hinstance),
            None,
        )
        .unwrap_or_default();

        let icon = make_circle_icon(COLOR_ORANGE);
        add_tray_icon(hwnd, icon, "shot2path — Ctrl+PrintScreen to capture");
        let _ = DestroyIcon(icon);

        RegisterHotKey(
            Some(hwnd),
            HOTKEY_CTRL_PRTSC,
            MOD_CONTROL | MOD_NOREPEAT,
            VK_SNAPSHOT.0 as u32,
        )
        .unwrap_or_default();

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
