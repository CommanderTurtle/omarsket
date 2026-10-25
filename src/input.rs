//! Edit native user configuration. No parallel binding store or generated includes.
use crate::model::Result;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
#[cfg(unix)]
use std::{io::Read, time::Duration};
static UPDATE: Mutex<()> = Mutex::new(());
static TEMP_ID: AtomicU64 = AtomicU64::new(0);
fn literal(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\{:03}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
pub fn ipc(command: &str) -> Result<Value> {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        let runtime = std::env::var("XDG_RUNTIME_DIR").map_err(|e| e.to_string())?;
        let instance = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").map_err(|e| e.to_string())?;
        if instance.contains('/') || instance.contains("..") {
            return Err("Invalid Hyprland instance".into());
        }
        let mut stream = UnixStream::connect(format!("{runtime}/hypr/{instance}/.socket.sock"))
            .map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(format!("j/{command}").as_bytes())
            .map_err(|e| e.to_string())?;
        let mut text = String::new();
        stream
            .take(2 * 1024 * 1024)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.trim() == "ok" {
            Ok(json!(true))
        } else {
            serde_json::from_str(&text).map_err(|_| text)
        }
    }
    #[cfg(not(unix))]
    {
        let _ = command;
        Err("Native input configuration requires Hyprland".into())
    }
}
fn config_path(file: &str) -> Result<PathBuf> {
    if !["bindings.lua", "input.lua"].contains(&file) {
        return Err("Choose a native Omarchy input file".into());
    }
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .ok_or("Set HOME or XDG_CONFIG_HOME")?;
    let file = root.join("hypr").join(file);
    if file.exists() {
        fs::canonicalize(file).map_err(|e| e.to_string())
    } else {
        Ok(file)
    }
}
fn privileged(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > 1024 * 1024 {
        return Err("Native configuration must be a regular file of at most 1 MiB".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(meta.uid() != unsafe { libc::geteuid() })
    }
    #[cfg(not(unix))]
    {
        Ok(false)
    }
}
fn contents(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    if privileged(path)? {
        return Err("Open privileged configuration through the native administrator editor".into());
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}
fn atomic_text(path: &Path, text: &str) -> Result<()> {
    let temp = path.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let mode = fs::metadata(path)
            .map(|m| m.mode() & 0o777)
            .unwrap_or(0o600);
        options.mode(mode);
    }
    let mut created = false;
    let result = (|| -> std::io::Result<()> {
        let mut out = options.open(&temp)?;
        created = true;
        out.write_all(text.as_bytes())?;
        out.sync_all()?;
        drop(out);
        fs::rename(&temp, path)
    })();
    if result.is_err() && created {
        let _ = fs::remove_file(temp);
    }
    result.map_err(|e| e.to_string())
}
fn restore(path: &Path, before: Option<&str>, written: &str) -> Result<()> {
    let current = if path.exists() {
        Some(contents(path)?)
    } else {
        None
    };
    if current.as_deref() == before {
        return Ok(());
    }
    if current.as_deref() != Some(written) {
        return Err("Configuration changed outside Omarsket; not overwritten".into());
    }
    if let Some(text) = before {
        atomic_text(path, text)
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}
fn keymap() -> Value {
    let result = (|| -> Result<Value> {
        let devices = ipc("devices")?;
        let keyboards = devices["keyboards"]
            .as_array()
            .ok_or("No keyboard metadata")?;
        let keyboard = keyboards
            .iter()
            .find(|k| k["main"] == true)
            .or(keyboards.first())
            .ok_or("No keyboard metadata")?;
        let root = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .and_then(|p| p.parent())
            .ok_or("Missing runtime")?
            .to_path_buf();
        let python = std::env::var_os("OMARSKET_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("backend/.venv/bin/python"));
        let mut child = Command::new(python)
            .arg(root.join("backend/keymap.py"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        child
            .stdin
            .take()
            .ok_or("No keymap input")?
            .write_all(keyboard.to_string().as_bytes())
            .map_err(|e| e.to_string())?;
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() || output.stdout.len() > 2 * 1024 * 1024 {
            return Err("Native XKB keymap unavailable".into());
        }
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
    })();
    result.unwrap_or_else(|_| json!({"error":"Native XKB keymap unavailable"}))
}
fn snapshot(file: &str) -> Result<Value> {
    let path = config_path(file)?;
    let admin = privileged(&path)?;
    let omarchy = std::env::var_os("OMARCHY_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/share/omarchy"));
    let default_path = omarchy.join("config/hypr").join(file);
    let mut defaults = fs::read_to_string(default_path).unwrap_or_default();
    if file == "bindings.lua" {
        // Read the actual packaged bindings, not just the personal-file template.
        read_binding_sources(&omarchy.join("default/hypr/bindings"), &mut defaults);
    }
    // Retain the existing snapshot response contract for other local clients.
    // The UI publishes active metadata only from its serialized describe path.
    Ok(
        json!({"path":path,"text":if admin{String::new()}else{contents(&path)?},"privileged":admin,"defaults":defaults,"bindings":ipc("binds").unwrap_or(json!([])),"keymap":keymap(),"routing":crate::remap::active_profile()}),
    )
}
// Independent of the file editor: refreshing active contracts never reads or
// replaces an unsaved draft, compiles Lua, reloads config, or touches devices.
fn describe() -> Result<Value> {
    let bindings = ipc("binds")?;
    if !bindings.is_array() {
        return Err("Native bindings unavailable".into());
    }
    Ok(json!({"bindings":bindings,"keymap":keymap(),"routing":crate::remap::active_profile()}))
}
fn read_binding_sources(dir: &Path, out: &mut String) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if out.len() >= 1024 * 1024 {
            break;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            read_binding_sources(&path, out);
        } else if meta.is_file()
            && path.extension().is_some_and(|e| e == "lua")
            && meta.len() <= 256 * 1024
            && let Ok(text) = fs::read_to_string(&path)
            && out.len() + text.len() < 1024 * 1024
        {
            out.push('\n');
            out.push_str(&text);
        }
    }
}
pub fn request(r: &Value) -> Result<Value> {
    let _guard = UPDATE.lock().map_err(|_| "Input editor stopped")?;
    #[cfg(unix)]
    if unsafe { libc::geteuid() } == 0 {
        return Err("Do not run the Omarsket UI as root".into());
    }
    if r["action"] == "describe" {
        return describe();
    }
    let file = r["file"].as_str().unwrap_or("bindings.lua");
    let path = config_path(file)?;
    match r["action"].as_str().unwrap_or("snapshot") {
        "snapshot" => snapshot(file),
        "admin-edit" => {
            if !privileged(&path)? {
                return Err("This is user-owned configuration; use the native file editor".into());
            }
            Command::new("omarchy-launch-terminal")
                .arg("--")
                .arg("sudoedit")
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(json!({"editing":true}))
        }
        "save" => {
            #[cfg(unix)]
            if unsafe { libc::geteuid() } == 0 {
                return Err("Do not run the Omarsket UI as root".into());
            }
            let previous = contents(&path)?;
            if r["expected"].as_str() != Some(&previous) {
                return Err(
                    "Native configuration changed outside this editor; reload before saving".into(),
                );
            }
            let text = r["text"]
                .as_str()
                .filter(|s| s.len() <= 1024 * 1024)
                .ok_or("Native configuration exceeds 1 MiB")?;
            // The native REPL clears its diagnostic buffer; preserve the baseline
            // before syntax checking, then let reload repopulate current errors.
            let previous_errors = ipc("configerrors")?;
            if !previous_errors.is_array() {
                return Err("Cannot verify native configuration errors".into());
            }
            if ipc(&format!("repl return load({}) ~= nil", literal(text)))? != true {
                return Err("Native Lua syntax rejected the changes".into());
            }
            let before = path.exists().then_some(previous.as_str());
            atomic_text(&path, text)?;
            let result = (|| -> Result<()> {
                ipc("reload")?;
                let errors = ipc("configerrors")?;
                if errors
                    .as_array()
                    .ok_or("Cannot verify native configuration")?
                    .iter()
                    .any(|e| !previous_errors.as_array().unwrap().contains(e))
                {
                    return Err("Native configuration rejected the changes".into());
                }
                Ok(())
            })();
            if let Err(error) = result {
                let restored = restore(&path, before, text);
                let _ = ipc("reload");
                return Err(if let Err(e) = restored {
                    format!("{error}; {e}")
                } else {
                    error
                });
            }
            snapshot(file)
        }
        _ => Err("Unknown native configuration action".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_does_not_inject_lua() {
        assert_eq!(literal("\"\n\\"), "\"\\\"\\n\\\\\"");
        assert!(config_path("../escape").is_err());
    }
    #[test]
    fn rollback_never_discards_external_edits() {
        let dir = std::env::temp_dir().join(format!(
            "omarsket-input-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("bindings.lua");
        fs::write(&path, "external").unwrap();
        assert!(restore(&path, Some("before"), "our edit").is_err());
        assert_eq!(contents(&path).unwrap(), "external");
        fs::write(&path, "our edit").unwrap();
        restore(&path, Some("before"), "our edit").unwrap();
        assert_eq!(contents(&path).unwrap(), "before");
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
