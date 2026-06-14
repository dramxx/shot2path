# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

shot2path is a Windows system-tray utility (no console window — `windows_subsystem = "windows"`) that captures
screenshots, saves each one as a timestamped PNG under `%USERPROFILE%\Pictures\shot2path\`, and copies its path
to the clipboard. See README.md for the user-facing feature list (hotkey, tray menu, etc.).

## Commands

- `cargo build` / `cargo build --release` — build (release profile uses LTO, strip, `panic = "abort"`,
  `codegen-units = 1` for a small binary)
- `cargo run` — run locally (registers itself for Windows startup on first launch via the registry)
- `cargo install --path .` — install to `~\.cargo\bin\shot2path.exe`
- Target: `x86_64-pc-windows-msvc` (required for the `windows` crate / GDI calls)
- No automated tests exist; verify changes by running the binary and exercising the hotkey / tray menu manually

## Architecture

The whole app is a single hidden window driven by a classic Win32 message loop (`main.rs`). There's no async
runtime — everything is synchronous Win32 API calls plus one spawned thread for the capture flow.

- **main.rs** — creates a singleton mutex (`shot2path_singleton_mutex`; a second launch exits immediately),
  registers the `Ctrl+PrintScreen` hotkey, creates the hidden window/tray icon, and dispatches messages in
  `wnd_proc`:
  - `WM_HOTKEY` → `capture::start_capture()`
  - `WM_TRAYICON` → left-click copies the most recent screenshot's path, right-click opens the tray menu
  - `WM_COMMAND` → tray menu actions (switch area/fullscreen mode, toggle startup, open images
    folder/recent image, exit)
- **capture.rs** — capture logic and image storage under `images_dir()`
  (`%USERPROFILE%\Pictures\shot2path\`).
  - `FULLSCREEN` (`AtomicBool`) selects area vs. fullscreen capture. `CAPTURE_GEN` (`AtomicU64`) is a
    generation counter: each `start_capture()` call bumps it, and `capture_area()`'s poll loop bails out
    as soon as it's no longer the latest generation — so a fresh Ctrl+PrintScreen always supersedes a
    stuck/cancelled snip instead of waiting out the 10s timeout.
  - Every capture is written to a new file named from the current local time
    (`new_image_path()` → `YYYY-MM-DD_HHMMSS_mmm.png` via `GetLocalTime`), never overwritten.
  - `recent_images(n)` lists the `n` most recent screenshots (newest first) by sorting filenames
    descending — the timestamp format is lexicographically sortable.
  - Area capture launches the Snipping Tool via the `ms-screenclip:` URI, then polls the clipboard sequence
    number (10s timeout) until a new `CF_DIB` appears.
  - Fullscreen capture uses GDI `BitBlt`/`GetDIBits` across the full virtual screen (all monitors).
  - `start_capture()` runs the whole flow (`capture_flow`) on a spawned thread with its own
    `CoInitializeEx` (apartment-threaded), since the Snipping Tool/clipboard interaction needs COM.
- **clipboard.rs** — low-level clipboard interop: reads `CF_DIB` and manually parses the `BITMAPINFOHEADER`
  (handles 24/32-bit depths, top-down/bottom-up rows, `BI_BITFIELDS` masks) into an RGBA buffer, encodes it to
  PNG via the `image` crate, and writes `CF_UNICODETEXT` when copying paths to the clipboard. `OpenClipboard`
  calls retry briefly (`open_clipboard_retry()`) since the Snipping Tool can transiently hold the clipboard.
- **tray.rs** — builds the tray icon (a solid circle rendered at runtime via `CreateIcon`, color
  `COLOR_ORANGE`) and the right-click context menu (checkmarks reflect `FULLSCREEN` mode and startup state).
  "Copy Image Path" is a native flyout submenu (`MF_POPUP`) listing up to `MAX_RECENT_IMAGES` recent
  screenshots by timestamp; the path for each submenu item is cached in `RECENT_IMAGES` when the menu is
  built, and copied to the clipboard via `copy_recent_image_path(id)` when `WM_COMMAND` fires for one of the
  `MENU_IMAGE_BASE..` IDs.
- **startup.rs** — manages the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` entry: `register_startup`
  sets it once on first run if absent, `set_startup`/`is_startup_enabled` back the "Run at startup" tray toggle.
- **shortcut.rs** — `create_desktop_shortcut()` (called once from `main()`, skipped if
  `%USERPROFILE%\Desktop\shot2path.lnk` already exists) renders the tray's orange circle to
  `%LOCALAPPDATA%\shot2path\icon.ico` via the `image` crate and creates the desktop shortcut pointing at
  `current_exe()` with that icon, using the `mslnk` crate (pure-Rust `.lnk` writer, no COM).
- **util.rs** — `wide()` helper for the UTF-16 string conversion required by Win32 APIs.

## Key invariant

Screenshots accumulate (one PNG per capture, never overwritten) under `capture::images_dir()`
(`%USERPROFILE%\Pictures\shot2path\`), named by capture timestamp. Nothing currently prunes old screenshots.
