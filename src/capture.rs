use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    SRCCOPY,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::SystemInformation::GetLocalTime;
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
static CAPTURE_GEN: AtomicU64 = AtomicU64::new(0);

pub fn images_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    PathBuf::from(home).join("Pictures").join("shot2path")
}

fn timestamp_string() -> String {
    let st = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02}_{:02}{:02}{:02}_{:03}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond, st.wMilliseconds
    )
}

fn new_image_path() -> PathBuf {
    let dir = images_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{}.png", timestamp_string()))
}

/// Returns up to `max` screenshots from `images_dir()`, most recent first.
pub fn recent_images(max: usize) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(images_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("png"))
                .collect()
        })
        .unwrap_or_default();
    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    entries.truncate(max);
    entries
}

pub fn copy_last_path() {
    if let Some(path) = recent_images(1).into_iter().next() {
        copy_image_path(&path);
    }
}

pub fn copy_image_path(path: &Path) {
    let _ = set_clipboard_text(&path.to_string_lossy());
}

pub fn open_images_folder() {
    let dir = images_dir();
    let _ = std::fs::create_dir_all(&dir);
    shell_open(&dir.to_string_lossy());
}

fn shell_open(target: &str) {
    let wtarget = wide(target);
    let verb = wide("open");
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(wtarget.as_ptr()),
            None,
            None,
            SW_SHOW,
        );
    }
}

fn launch_snipping_tool() {
    shell_open("ms-screenclip:");
}

fn capture_flow(gen: u64) {
    if FULLSCREEN.load(Ordering::Relaxed) {
        capture_fullscreen();
    } else {
        capture_area(gen);
    }
}

fn save_and_copy(png: &[u8]) {
    let out_path = new_image_path();
    if std::fs::write(&out_path, png).is_err() {
        return;
    }
    let path_str = out_path.to_string_lossy().to_string();
    let _ = set_clipboard_text(&path_str);
}

fn capture_area(gen: u64) {
    let seq_before = clipboard_seq();
    launch_snipping_tool();

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if CAPTURE_GEN.load(Ordering::SeqCst) != gen || Instant::now() > deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
        if clipboard_seq() != seq_before && clipboard_has_dib() {
            break;
        }
    }

    if let Some(png) = grab_clipboard_bitmap() {
        save_and_copy(&png);
    }
}

fn capture_fullscreen() {
    if let Some(png) = capture_fullscreen_png() {
        save_and_copy(&png);
    }
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
    let gen = CAPTURE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        capture_flow(gen);
        unsafe {
            CoUninitialize();
        }
    });
}
