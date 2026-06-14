use std::path::{Path, PathBuf};

use image::{Rgba, RgbaImage};
use mslnk::ShellLink;

use crate::tray::COLOR_ORANGE;

const ICON_SIZE: u32 = 48;

fn desktop_shortcut_path() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    PathBuf::from(home).join("Desktop").join("shot2path.lnk")
}

fn icon_path() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(local_app_data)
        .join("shot2path")
        .join("icon.ico")
}

/// Renders the same orange circle as the tray icon to an `.ico` file.
fn write_icon_file(path: &Path) -> image::ImageResult<()> {
    let mut img = RgbaImage::new(ICON_SIZE, ICON_SIZE);
    let c = ICON_SIZE as f32 / 2.0;
    let r = c - 1.0;
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            if dx * dx + dy * dy <= r * r {
                img.put_pixel(x, y, Rgba(COLOR_ORANGE));
            }
        }
    }
    img.save(path)
}

/// Creates a `shot2path.lnk` shortcut on the desktop (once) pointing at the running
/// executable, with an icon matching the tray icon.
pub fn create_desktop_shortcut() {
    let shortcut = desktop_shortcut_path();
    if shortcut.exists() {
        return;
    }

    let Ok(exe) = std::env::current_exe() else {
        return;
    };

    let icon = icon_path();
    if let Some(dir) = icon.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if write_icon_file(&icon).is_err() {
        return;
    }

    if let Ok(mut link) = ShellLink::new(&exe) {
        link.set_icon_location(Some(icon.to_string_lossy().to_string()));
        let _ = link.create_lnk(&shortcut);
    }
}
