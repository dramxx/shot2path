# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

shot2path is a Windows system-tray utility (no console window — `windows_subsystem = "windows"`) that captures
screenshots, saves them to a fixed path (`%TEMP%\cshot_latest.png`), and copies that path to the clipboard. See
README.md for the user-facing feature list (hotkey, tray menu, etc.).

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
  - `WM_TRAYICON` → left-click copies the last screenshot path, right-click opens the tray menu
  - `WM_COMMAND` → tray menu actions (switch area/fullscreen mode, toggle startup, open last image, exit)
- **capture.rs** — capture logic and the fixed output path (`image_path()` always returns
  `%TEMP%\cshot_latest.png`).
  - `FULLSCREEN` (`AtomicBool`) selects area vs. fullscreen capture; `CAPTURING` (`AtomicBool`) prevents
    overlapping captures.
  - Area capture launches the Snipping Tool via the `ms-screenclip:` URI, then polls the clipboard sequence
    number (10s timeout) until a new `CF_DIB` appears.
  - Fullscreen capture uses GDI `BitBlt`/`GetDIBits` across the full virtual screen (all monitors).
  - `start_capture()` runs the whole flow on a spawned thread with its own `CoInitializeEx`
    (apartment-threaded), since the Snipping Tool/clipboard interaction needs COM.
- **clipboard.rs** — low-level clipboard interop: reads `CF_DIB` and manually parses the `BITMAPINFOHEADER`
  (handles 24/32-bit depths, top-down/bottom-up rows, `BI_BITFIELDS` masks) into an RGBA buffer, encodes it to
  PNG via the `image` crate, and writes `CF_UNICODETEXT` when copying paths to the clipboard.
- **tray.rs** — builds the tray icon (a solid circle rendered at runtime via `CreateIcon`, color
  `COLOR_ORANGE`) and the right-click context menu (checkmarks reflect `FULLSCREEN` mode and startup state).
- **startup.rs** — manages the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` entry: `register_startup`
  sets it once on first run if absent, `set_startup`/`is_startup_enabled` back the "Run at startup" tray toggle.
- **util.rs** — `wide()` helper for the UTF-16 string conversion required by Win32 APIs.

## Key invariant

The output path is always `%TEMP%\cshot_latest.png` (`capture::image_path()`) — this fixed, predictable path is
the whole point of the tool (consumers paste the path copied to the clipboard).
