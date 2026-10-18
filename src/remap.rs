//! Optional physical input routing. Native desktop bindings are not stored here.
use crate::model::{Result, valid_id};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

pub const HELPER: &str = "/usr/local/libexec/omarsket-input";
pub const DEFAULT: &str = "ts_uinput";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceMap {
    pub path: String,
    pub vendor: u16,
    pub product: u16,
    #[serde(default)]
    pub keys: BTreeMap<u16, Vec<u16>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub devices: Vec<DeviceMap>,
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        valid_id(&self.name)?;
        if self.name.eq_ignore_ascii_case(DEFAULT) {
            return Err("The native default is read-only".into());
        }
        if self.devices.is_empty() || self.devices.len() > 8 {
            return Err("Choose 1–8 physical devices".into());
        }
        let mut seen = BTreeSet::new();
        for d in &self.devices {
            let p = Path::new(&d.path);
            if !(p.parent() == Some(Path::new("/dev/input/by-id"))
                || p.parent() == Some(Path::new("/dev/input/by-path")))
                || p.file_name().is_none()
                || d.path.contains(['\0', '\n', '\r'])
                || !seen.insert(&d.path)
            {
                return Err("Choose a distinct physical device by-id or by-path".into());
            }
            if d.keys.len() > 768 {
                return Err("Too many input mappings".into());
            }
            for (source, target) in &d.keys {
                if *source == 0
                    || *source > 767
                    || target.len() > 8
                    || target.iter().any(|k| *k == 0 || *k > 767)
                    || target.iter().collect::<BTreeSet<_>>().len() != target.len()
                {
                    return Err(
                        "Mappings need physical key codes 1–767 and at most eight distinct outputs"
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }
}

/// Tracks held output references, so two physical keys mapped to G cannot release
/// one another. Only live state exists; no event sequence is saved or exposed.
#[derive(Default)]
pub struct Keys {
    held: BTreeMap<u16, Vec<u16>>,
    counts: BTreeMap<u16, usize>,
}
impl Keys {
    pub fn event(
        &mut self,
        map: &BTreeMap<u16, Vec<u16>>,
        code: u16,
        value: i32,
    ) -> Vec<(u16, i32)> {
        if value == 2 {
            return self
                .held
                .get(&code)
                .and_then(|v| v.last())
                .map(|k| vec![(*k, 2)])
                .unwrap_or_default();
        }
        let mut out = Vec::new();
        if value == 1 && !self.held.contains_key(&code) {
            let targets = map.get(&code).cloned().unwrap_or_else(|| vec![code]);
            for key in &targets {
                let n = self.counts.entry(*key).or_default();
                if *n == 0 {
                    out.push((*key, 1))
                }
                *n += 1;
            }
            self.held.insert(code, targets);
        } else if value == 0
            && let Some(targets) = self.held.remove(&code)
        {
            for key in targets.iter().rev() {
                if let Some(n) = self.counts.get_mut(key) {
                    *n -= 1;
                    if *n == 0 {
                        self.counts.remove(key);
                        out.push((*key, 0));
                    }
                }
            }
        }
        out
    }
    pub fn emergency(&self) -> bool {
        [1, 14, 28].iter().all(|k| self.held.contains_key(k))
    }
    pub fn release(&mut self) -> Vec<(u16, i32)> {
        let out = self.counts.keys().map(|k| (*k, 0)).collect();
        self.counts.clear();
        self.held.clear();
        out
    }
}
fn profile_path(root: &Path, name: &str) -> Result<PathBuf> {
    valid_id(name)?;
    if name.eq_ignore_ascii_case(DEFAULT) {
        return Err("The native default is read-only".into());
    }
    Ok(root.join(format!("{name}.json")))
}
pub fn read_profile(path: &Path) -> Result<Profile> {
    if fs::symlink_metadata(path).map_err(|e| e.to_string())?.len() > 128 * 1024 {
        return Err("Input profile exceeds 128 KiB".into());
    }
    let p: Profile = serde_json::from_value(crate::store::read_json(path)?)
        .map_err(|_| "Invalid input profile")?;
    p.validate()?;
    Ok(p)
}
pub fn profiles(root: &Path) -> Result<Value> {
    let mut items = Vec::new();
    if root.exists() {
        for file in fs::read_dir(root).map_err(|e| e.to_string())? {
            let file = file.map_err(|e| e.to_string())?.path();
            if file.extension().is_some_and(|s| s == "json") {
                let p = read_profile(&file)?;
                if profile_path(root, &p.name)? != file {
                    return Err("Input profile name differs from its file".into());
                }
                items.push(p);
            }
            if items.len() > 256 {
                return Err("Input profile library exceeds 256 profiles".into());
            }
        }
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(
        json!({"profiles":items,"default":DEFAULT,"devices":devices(),"nativeTouch":Path::new("/usr/bin/ts_uinput").exists(),"helper":trusted_helper(),"active":client("status").ok(),"error":activation_error()}),
    )
}
pub fn active_profile() -> Value {
    active_routing()
}
pub fn request(root: &Path, r: &Value) -> Result<Value> {
    match r["action"].as_str().unwrap_or("snapshot") {
        "snapshot" => profiles(root),
        "save" => {
            let p: Profile = serde_json::from_value(r["profile"].clone())
                .map_err(|_| "Invalid input profile")?;
            p.validate()?;
            if serde_json::to_vec(&p).map_err(|e| e.to_string())?.len() > 128 * 1024 {
                return Err("Input profile exceeds 128 KiB".into());
            }
            let file = profile_path(root, &p.name)?;
            let existing = if file.exists() {
                Some(serde_json::to_value(read_profile(&file)?).unwrap())
            } else {
                None
            };
            if r.get("expected") != Some(existing.as_ref().unwrap_or(&Value::Null)) {
                return Err("Input profile changed; reload before saving".into());
            }
            if client("status")
                .ok()
                .is_some_and(|v| v["profile"] == p.name)
            {
                return Err("Stop this profile before editing it".into());
            }
            crate::store::private_dir(root)?;
            crate::store::atomic_json(&file, &serde_json::to_value(p).unwrap())?;
            profiles(root)
        }
        "delete" => {
            let name = r["name"].as_str().ok_or("Choose an input profile")?;
            if client("status").ok().is_some_and(|v| v["profile"] == name) {
                return Err("Stop this profile before deleting it".into());
            }
            fs::remove_file(profile_path(root, name)?).map_err(|e| e.to_string())?;
            profiles(root)
        }
        "start" => {
            let path = profile_path(root, r["name"].as_str().ok_or("Choose an input profile")?)?;
            read_profile(&path)?;
            start(&path)?;
            Ok(json!({"authorizing":true}))
        }
        "stop" => {
            client("stop")?;
            Ok(json!({"stopped":true}))
        }
        _ => Err("Unknown input router action".into()),
    }
}

#[cfg(target_os = "linux")]
#[path = "remap_linux.rs"]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{
    activation_error, active_routing, client, devices, report_error, run, start, trusted_helper,
};
#[cfg(not(target_os = "linux"))]
fn devices() -> Value {
    json!([])
}
#[cfg(not(target_os = "linux"))]
fn trusted_helper() -> bool {
    false
}
#[cfg(not(target_os = "linux"))]
fn client(_: &str) -> Result<Value> {
    Err("Input routing requires Linux".into())
}
#[cfg(not(target_os = "linux"))]
fn start(_: &Path) -> Result<()> {
    Err("Input routing requires Linux".into())
}
#[cfg(not(target_os = "linux"))]
fn activation_error() -> Option<String> {
    None
}
#[cfg(not(target_os = "linux"))]
fn active_routing() -> Value {
    Value::Null
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_references_repeats_and_chords_are_balanced() {
        let map = BTreeMap::from([(33, vec![29, 34]), (35, vec![34]), (36, vec![])]);
        let mut keys = Keys::default();
        assert_eq!(keys.event(&map, 33, 1), vec![(29, 1), (34, 1)]);
        assert_eq!(keys.event(&map, 35, 1), vec![]);
        assert_eq!(keys.event(&map, 33, 2), vec![(34, 2)]);
        assert_eq!(keys.event(&map, 33, 0), vec![(29, 0)]);
        assert_eq!(keys.event(&map, 35, 0), vec![(34, 0)]);
        assert!(keys.event(&map, 36, 1).is_empty());
        keys.event(&map, 1, 1);
        keys.event(&map, 14, 1);
        keys.event(&map, 28, 1);
        assert!(keys.emergency());
        assert_eq!(keys.release().len(), 3);
        assert!(!keys.emergency());
    }
    #[test]
    fn routing_profiles_cannot_target_files_or_replace_the_native_default() {
        let mut p = Profile {
            name: "Desk".into(),
            devices: vec![DeviceMap {
                path: "/dev/input/by-id/usb-keyboard-event-kbd".into(),
                vendor: 1,
                product: 2,
                keys: BTreeMap::from([(33, vec![34])]),
            }],
        };
        assert!(p.validate().is_ok());
        p.name = DEFAULT.into();
        assert!(p.validate().is_err());
        p.name = "Desk".into();
        p.devices[0].path = "/etc/shadow".into();
        assert!(p.validate().is_err());
        p.devices[0].path = "/dev/input/by-id/../event0".into();
        assert!(p.validate().is_err());
    }
    #[test]
    fn profile_saves_are_explicit_and_guard_outside_edits() {
        let root = std::env::temp_dir().join(format!(
            "omarsket-remap-{}-{}",
            std::process::id(),
            crate::engine::now_ms()
        ));
        assert!(profiles(&root).is_ok());
        assert!(!root.exists());
        let value = json!({"name":"Desk","devices":[{"path":"/dev/input/by-id/example-event-kbd","vendor":1,"product":2,"keys":{"33":[34]}}]});
        request(
            &root,
            &json!({"action":"save","profile":value,"expected":null}),
        )
        .unwrap();
        assert!(
            request(
                &root,
                &json!({"action":"save","profile":value,"expected":null})
            )
            .is_err()
        );
        let mut edited = value.clone();
        edited["devices"][0]["keys"]["33"] = json!([35]);
        request(
            &root,
            &json!({"action":"save","profile":edited,"expected":value}),
        )
        .unwrap();
        assert_eq!(
            read_profile(&root.join("Desk.json")).unwrap().devices[0].keys[&33],
            vec![35]
        );
        assert!(request(&root, &json!({"action":"delete","name":"ts_uinput"})).is_err());
        request(&root, &json!({"action":"delete","name":"Desk"})).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn repeats_unknown_releases_and_resync_deltas_do_not_stick_keys() {
        let map = BTreeMap::from([(33, vec![34])]);
        let mut keys = Keys::default();
        assert!(keys.event(&map, 33, 2).is_empty());
        assert!(keys.event(&map, 33, 0).is_empty());
        assert_eq!(keys.event(&map, 33, 1), vec![(34, 1)]);
        assert!(keys.event(&map, 33, 1).is_empty());
        assert!(keys.event(&map, 34, 1).is_empty());
        assert!(keys.event(&map, 33, 0).is_empty());
        assert_eq!(keys.event(&map, 34, 0), vec![(34, 0)]);
        assert!(keys.release().is_empty());
    }
}
