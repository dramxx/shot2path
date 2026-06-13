use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "shot2path";

fn exe_path() -> String {
    std::env::current_exe()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

pub fn register_startup() {
    if let Ok(run) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_QUERY_VALUE)
    {
        if run.get_value::<String, _>(VALUE_NAME).is_err() {
            let _ = run.set_value(VALUE_NAME, &exe_path());
        }
    }
}

pub fn is_startup_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_QUERY_VALUE)
        .and_then(|run| run.get_value::<String, _>(VALUE_NAME))
        .is_ok()
}

pub fn set_startup(enabled: bool) {
    if let Ok(run) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_QUERY_VALUE)
    {
        if enabled {
            let _ = run.set_value(VALUE_NAME, &exe_path());
        } else {
            let _ = run.delete_value(VALUE_NAME);
        }
    }
}
