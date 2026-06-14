# shot2path

A Windows system-tray utility that adds **Ctrl+PrintScreen** keybind, captures a screenshot, saves it to `Pictures\shot2path\` with a timestamped filename, and copies that path to the clipboard, so you can paste it into Claude Code, or whatever.

## What it does

| Action                    | Result                                                                               |
| ------------------------- | ------------------------------------------------------------------------------------ |
| **Ctrl+PrintScreen**      | Captures a screenshot, saves `Pictures\shot2path\<timestamp>.png`, and copies its path to clipboard |
| **Left-click tray icon**  | Copies the path of the most recent screenshot to clipboard                          |
| **Right-click tray icon** | Opens the context menu                                                               |

### Context menu

- **Area screenshot** _(default)_ — opens the Windows Snipping Tool for region selection
- **Fullscreen** — captures all monitors directly via GDI
- **Run at startup** — toggles the `HKCU\...\Run` registry entry
- **Copy Image Path** — hover to pick one of the most recent screenshots (by timestamp); clicking it copies that screenshot's path to the clipboard
- **Open images folder** — opens `Pictures\shot2path` in Explorer
- **Exit** — removes the tray icon and quits

## Requirements

- Windows 10 1809 or later (requires `ms-screenclip:` URI / Snipping Tool)
- Rust toolchain (MSVC target: `x86_64-pc-windows-msvc`)

## Install

```powershell
cargo install --path .
```

The binary is installed to `~\.cargo\bin\shot2path.exe`. On first launch it registers itself to run at Windows startup (can be toggled off via the tray menu) and creates a `shot2path` shortcut on the desktop.
