use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    SRCCOPY,
};
use windows::Win32::Storage::FileSystem::GetTempPathW;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    SW_SHOW,
};

use crate::clipboard::{
    clipboard_has_dib, clipboard_seq, encode_rgba_png, grab_clipboard_bitmap, set_clipboard_text,
};
use crate::util::wide;

pub static FULLSCREEN: AtomicBool = AtomicBool::new(false);
static CAPTURING: AtomicBool = AtomicBool::new(false);

pub fn image_path() -> PathBuf {
    let mut buf = [0u16; 260];
    unsafe {
        GetTempPathW(Some(&mut buf));
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let path = String::from_utf16_lossy(&buf[..len]);
    PathBuf::from(path).join("cshot_latest.png")
}

pub fn copy_last_path() {
    let path = image_path();
    if !path.exists() {
        return;
    }
    let _ = set_clipboard_text(&path.to_string_lossy());
}

pub fn open_last_image() {
    let path = image_path();
    if !path.exists() {
        return;
    }
    let wpath = wide(&path.to_string_lossy());
    let verb = wide("open");
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(wpath.as_ptr()),
            None,
            None,
            SW_SHOW,
        );
    }
}

fn launch_snipping_tool() {
    let verb = wide("open");
    let url = wide("ms-screenclip:");
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(url.as_ptr()),
            None,
            None,
            SW_SHOW,
        );
    }
}

fn capture_flow() {
    if FULLSCREEN.load(Ordering::Relaxed) {
        capture_fullscreen();
    } else {
        capture_area();
    }
}

fn save_and_copy(png: &[u8]) {
    let out_path = image_path();
    if std::fs::write(&out_path, png).is_err() {
        return;
    }
    let path_str = out_path.to_string_lossy().to_string();
    let _ = set_clipboard_text(&path_str);
}

fn capture_area() {
    let seq_before = clipboard_seq();
    launch_snipping_tool();

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if Instant::now() > deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
        if clipboard_seq() != seq_before && clipboard_has_dib() {
            break;
        }
    }

    let png = match grab_clipboard_bitmap() {
        Some(b) => b,
        None => return,
    };

    save_and_copy(&png);
}

fn capture_fullscreen() {
    let png = match capture_fullscreen_png() {
        Some(b) => b,
        None => return,
    };
    save_and_copy(&png);
}

fn capture_fullscreen_png() -> Option<Vec<u8>> {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if w <= 0 || h <= 0 {
            return None;
        }

        let screen_dc = GetDC(None);
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap = CreateCompatibleBitmap(screen_dc, w, h);
        let old = SelectObject(mem_dc, HGDIOBJ(bitmap.0));
        let blt = BitBlt(mem_dc, 0, 0, w, h, Some(screen_dc), x, y, SRCCOPY);
        SelectObject(mem_dc, old);

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
        let lines = GetDIBits(
            mem_dc,
            bitmap,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);

        if blt.is_err() || lines == 0 {
            return None;
        }

        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }

        encode_rgba_png(w as u32, h as u32, buf)
    }
}

pub fn start_capture() {
    if CAPTURING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    std::thread::spawn(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        capture_flow();
        unsafe {
            CoUninitialize();
        }
        CAPTURING.store(false, Ordering::SeqCst);
    });
}
