use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use image::{ImageBuffer, Rgba};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_BITFIELDS};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_DIB, CF_UNICODETEXT};

pub fn clipboard_seq() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

pub fn clipboard_has_dib() -> bool {
    unsafe { IsClipboardFormatAvailable(CF_DIB.0 as u32).is_ok() }
}

pub fn grab_clipboard_bitmap() -> Option<Vec<u8>> {
    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }
        let hdata = match GetClipboardData(CF_DIB.0 as u32) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return None;
            }
        };
        let hmem_dib = HGLOBAL(hdata.0 as *mut core::ffi::c_void);
        let ptr = GlobalLock(hmem_dib) as *const u8;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return None;
        }

        let bih = ptr as *const BITMAPINFOHEADER;
        let width = (*bih).biWidth as u32;
        let height = (*bih).biHeight;
        let abs_height = height.unsigned_abs();
        let bit_count = (*bih).biBitCount as u32;

        let colors_used = if bit_count <= 8 {
            if (*bih).biClrUsed != 0 {
                (*bih).biClrUsed
            } else {
                1u32 << bit_count
            }
        } else {
            0
        };

        let header_size = (*bih).biSize as usize;
        let palette_size = (colors_used * 4) as usize;
        let mask_size =
            if bit_count > 8 && (*bih).biCompression == BI_BITFIELDS.0 && header_size == 40 {
                12
            } else {
                0
            };
        let pixel_data = ptr.add(header_size + palette_size + mask_size);

        let stride = ((width * bit_count + 31) / 32) * 4;
        let mut rgba_buf: Vec<u8> = Vec::with_capacity((width * abs_height * 4) as usize);

        let top_down = height < 0;

        for row in 0..abs_height {
            let src_row = if top_down { row } else { abs_height - 1 - row };
            let row_ptr = pixel_data.add((src_row * stride) as usize);
            for col in 0..width {
                let (r, g, b, a) = match bit_count {
                    24 => {
                        let p = row_ptr.add((col * 3) as usize);
                        (*p.add(2), *p.add(1), *p, 255)
                    }
                    32 => {
                        let p = row_ptr.add((col * 4) as usize);
                        (*p.add(2), *p.add(1), *p, 255)
                    }
                    _ => (0, 0, 0, 255),
                };
                rgba_buf.push(r);
                rgba_buf.push(g);
                rgba_buf.push(b);
                rgba_buf.push(a);
            }
        }

        let _ = GlobalUnlock(hmem_dib);
        let _ = CloseClipboard();

        encode_rgba_png(width, abs_height, rgba_buf)
    }
}

pub fn encode_rgba_png(width: u32, height: u32, rgba: Vec<u8>) -> Option<Vec<u8>> {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, rgba)?;
    let mut png_bytes: Vec<u8> = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_bytes),
        image::ImageFormat::Png,
    )
    .ok()?;
    Some(png_bytes)
}

pub fn set_clipboard_text(text: &str) -> bool {
    let wide_text: Vec<u16> = OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let byte_len = wide_text.len() * 2;
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        let Ok(hmem) = hmem else {
            let _ = CloseClipboard();
            return false;
        };
        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(wide_text.as_ptr(), ptr, wide_text.len());
        let _ = GlobalUnlock(hmem);
        let _ = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hmem.0)));
        let _ = CloseClipboard();
    }
    true
}
