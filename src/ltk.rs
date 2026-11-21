//! Talking to LTK Manager: process detection, the ltk:// protocol, trusted domain setup.

use crate::data::{self, DOWNLOAD_DOMAIN};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;

/// LTK Manager is a Tauri app. The protocol handler is often an 8.3 name
/// (LTK-MA~1.EXE), not ltk.exe. The patcher host also counts as running.
const EXE_HINTS: &[&str] = &[
    "ltk-manager", "ltk manager", "ltk.exe", "ltk-ma~", "ltk_patcher", "cslol-host", "cslol_host",
];

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

/// Executable names registered as the `ltk://` handler (short and long form).
fn protocol_basenames() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        let mut names = HashSet::new();
        for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            let Ok(key) = winreg::RegKey::predef(hive).open_subkey(r"Software\Classes\ltk\shell\open\command")
            else { continue };
            let Ok(cmd) = key.get_value::<String, _>("") else { continue };
            let Some(path) = exe_from_command(&cmd) else { continue };
            if let Some(base) = basename(&path) {
                names.insert(base);
            }
            if let Some(long) = long_path(&path) {
                if let Some(base) = basename(&long) {
                    names.insert(base);
                }
            }
        }
        names
    })
}

fn exe_from_command(cmd: &str) -> Option<String> {
    let lc = cmd.to_lowercase();
    if let Some(rest) = cmd.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    let end = lc.find(".exe")? + 4;
    Some(cmd[..end].trim().to_string())
}

fn basename(path: &str) -> Option<String> {
    PathBuf::from(path).file_name().map(|n| n.to_string_lossy().to_lowercase())
}

fn long_path(path: &str) -> Option<String> {
    let w = wide(path);
    let mut buf = vec![0u16; 32768];
    let n = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetLongPathNameW(w.as_ptr(), buf.as_mut_ptr(), buf.len() as u32)
    };
    (n > 0).then(|| from_wide(&buf))
}

fn running_exe_names() -> Vec<String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE || snap.is_null() {
            return out;
        }
        let mut pe: PROCESSENTRY32W = std::mem::zeroed();
        pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut pe) != 0 {
            loop {
                out.push(from_wide(&pe.szExeFile).to_lowercase());
                if Process32NextW(snap, &mut pe) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    out
}

pub fn is_running() -> bool {
    let protocol = protocol_basenames();
    running_exe_names()
        .iter()
        .any(|n| protocol.contains(n) || EXE_HINTS.iter().any(|h| n.contains(h)))
}

/// ShellExecute "open", same as Python's os.startfile. Works for URLs and protocols.
pub fn shell_open(target: &str) -> bool {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let op = wide("open");
    let file = wide(target);
    let r = unsafe {
        ShellExecuteW(std::ptr::null_mut(), op.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL)
    };
    (r as isize) > 32
}

pub fn open_notepad(path: &std::path::Path) {
    let _ = std::process::Command::new("notepad").arg(path).spawn();
}

pub fn install_link(url: &str, name: &str) -> String {
    format!(
        "ltk://install?url={}&name={}&author=LeagueSkins&source=HalitChanger",
        urlencoding::encode(url),
        urlencoding::encode(name)
    )
}

/// Adds raw.githubusercontent.com to LTK's trusted download domains.
pub fn prepare_trusted_domain() {
    let Some(appdata) = std::env::var_os("APPDATA") else { return };
    let path = PathBuf::from(appdata).join("dev.leaguetoolkit.manager").join("settings.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) else {
        data::log("LTK settings parse failed");
        return;
    };
    let Some(obj) = v.as_object_mut() else { return };
    let domains = obj
        .entry("trustedDomains")
        .or_insert_with(|| serde_json::Value::Array(vec![]));
    let Some(arr) = domains.as_array_mut() else { return };
    if arr.iter().any(|d| d.as_str() == Some(DOWNLOAD_DOMAIN)) {
        return;
    }
    arr.push(serde_json::Value::String(DOWNLOAD_DOMAIN.into()));
    match serde_json::to_string_pretty(&v) {
        Ok(s) if std::fs::write(&path, &s).is_ok() => data::log(&format!("Trusted domain added: {DOWNLOAD_DOMAIN}")),
        _ => data::log("LTK settings update failed"),
    }
}

pub fn detect_turkish_ui() -> bool {
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    (id & 0xFF) == 0x1F // LANG_TURKISH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_protocol_command() {
        assert_eq!(
            exe_from_command(r#""C:\Program Files\LTK\LTK-MA~1.EXE" "%1""#).as_deref(),
            Some(r"C:\Program Files\LTK\LTK-MA~1.EXE")
        );
        assert_eq!(exe_from_command(r"C:\x\ltk.exe %1").as_deref(), Some(r"C:\x\ltk.exe"));
    }

    #[test]
    fn link_is_encoded() {
        let l = install_link("https://a/b c.fantome", "Annie - Goth");
        assert!(l.starts_with("ltk://install?url=https%3A%2F%2Fa%2Fb%20c.fantome&name=Annie%20-%20Goth"));
    }
}
