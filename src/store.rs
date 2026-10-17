use crate::model::{Result, Snippet, Task, valid_id};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_FILE: u64 = 16 * 1024 * 1024;
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub tasks: BTreeMap<String, Task>,
    #[serde(default)]
    pub data: BTreeMap<String, Snippet>,
    #[serde(skip)]
    pub notices: Vec<String>,
    #[serde(skip)]
    pub preferences: BTreeMap<String, bool>,
}

/// --library retains its old meaning for migration; new callers pass a directory.
pub fn root_path(path: &Path) -> &Path {
    if path.extension().is_some_and(|e| e == "json") {
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    } else {
        path
    }
}
pub fn read_json(path: &Path) -> Result<Value> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > MAX_FILE {
        return Err(format!(
            "{} must be a regular JSON file of at most 16 MiB",
            path.display()
        ));
    }
    serde_json::from_reader(fs::File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{}: {e}", path.display()))
}
fn optional_json(path: &Path) -> Result<Option<Value>> {
    match fs::symlink_metadata(path) {
        Ok(_) => read_json(path).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub fn atomic_json(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err("A schedule/data file exceeds 16 MiB".into());
    }
    let temp = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
impl Store {
    pub fn validate(&self) -> Result<()> {
        for (id, task) in &self.tasks {
            task.validate()?;
            if id != &task.id {
                return Err("Stored task id does not match its key".into());
            }
        }
        for (id, item) in &self.data {
            valid_id(id)?;
            item.validate()?;
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let root = root_path(path);
        private_dir(root)?;
        let schedules = root.join("schedules");
        private_dir(&schedules)?;
        let mut store = Self::default();
        if let Some(preferences) = optional_json(&root.join("preferences.json"))? {
            store.preferences = serde_json::from_value(preferences).map_err(|e| e.to_string())?;
        }
        if let Some(data) = optional_json(&root.join("data.json"))? {
            store.data = serde_json::from_value(data).map_err(|e| e.to_string())?;
        }
        for entry in fs::read_dir(&schedules).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let file = entry.path();
            if file
                .extension()
                .is_none_or(|s| !s.eq_ignore_ascii_case("omscht"))
            {
                continue;
            }
            let t: Task = serde_json::from_value(read_json(&file)?)
                .map_err(|e| format!("{}: {e}", file.display()))?;
            if file.file_stem().and_then(|s| s.to_str()) != Some(&t.id) {
                return Err(format!(
                    "Schedule name and file name differ: {}",
                    file.display()
                ));
            }
            t.validate()?;
            store.tasks.insert(t.id.clone(), t);
        }
        store.validate()?;
        // Retain originals. The journal prevents deleted imports returning on launch.
        let journal = root.join("migrations.json");
        let mut seen: BTreeSet<String> = optional_json(&journal)?
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let legacy = if path.extension().is_some_and(|e| e == "json") {
            path.to_path_buf()
        } else {
            root.join("library.json")
        };
        let legacy_key = legacy.to_string_lossy().into_owned();
        if legacy.exists() && !seen.contains(&legacy_key) {
            let previous = store.clone();
            let old: Self =
                serde_json::from_value(read_json(&legacy)?).map_err(|e| e.to_string())?;
            old.validate()?;
            for (id, task) in old.tasks {
                store.tasks.entry(id).or_insert(task);
            }
            for (id, item) in old.data {
                store.data.entry(id).or_insert(item);
            }
            store.save_changes(path, &previous)?;
            seen.insert(legacy_key);
            atomic_json(&journal, &serde_json::to_value(&seen).unwrap())?;
            store.notices.push(
                "Converted library.json to individual .omscht schedules; original retained".into(),
            );
        }
        for folder in [&schedules, &root.join("saved_tasks"), root] {
            if !folder.is_dir() {
                continue;
            }
            let mut files: Vec<_> = fs::read_dir(folder)
                .map_err(|e| e.to_string())?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension()
                        .is_some_and(|s| s.eq_ignore_ascii_case("scht"))
                })
                .collect();
            files.sort();
            for file in files {
                let key = file.to_string_lossy().into_owned();
                if seen.contains(&key) {
                    continue;
                }
                let result = (|| {
                    let id = file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or("Invalid schedule filename")?;
                    if store.tasks.contains_key(id) {
                        return Err(format!(
                            "Schedule {id} already exists; original .scht left for manual import"
                        ));
                    }
                    let task = Task::import(id, read_json(&file)?)?;
                    let previous = store.clone();
                    store.tasks.insert(id.into(), task);
                    if let Err(e) = store.save_changes(path, &previous) {
                        store = previous;
                        return Err(e);
                    }
                    Ok(())
                })();
                match result {
                    Ok(()) => {
                        seen.insert(key);
                        atomic_json(&journal, &serde_json::to_value(&seen).unwrap())?;
                        store
                            .notices
                            .push(format!("Converted {} (original retained)", file.display()));
                    }
                    Err(e) => store.notices.push(format!("{}: {e}", file.display())),
                }
            }
        }
        Ok(store)
    }
    /// Rewrite only touched files. Detect external edits, then roll back failed batches.
    pub fn save_changes(&self, path: &Path, previous: &Self) -> Result<()> {
        self.validate()?;
        let root = root_path(path);
        private_dir(root)?;
        private_dir(&root.join("schedules"))?;
        let mut changes: Vec<(PathBuf, Option<Value>, Option<Value>)> = Vec::new();
        let ids: BTreeSet<_> = self.tasks.keys().chain(previous.tasks.keys()).collect();
        for id in ids {
            let old = previous
                .tasks
                .get(id)
                .map(|v| serde_json::to_value(v).unwrap());
            let new = self.tasks.get(id).map(|v| serde_json::to_value(v).unwrap());
            if old != new {
                changes.push((
                    root.join("schedules").join(format!("{id}.omscht")),
                    old,
                    new,
                ));
            }
        }
        let old = serde_json::to_value(&previous.data).unwrap();
        let new = serde_json::to_value(&self.data).unwrap();
        if old != new {
            let file = root.join("data.json");
            let expected = if previous.data.is_empty() && !file.exists() {
                None
            } else {
                Some(old)
            };
            changes.push((file, expected, Some(new)));
        }
        if self.preferences != previous.preferences {
            let file = root.join("preferences.json");
            let old = if previous.preferences.is_empty() && !file.exists() {
                None
            } else {
                Some(serde_json::to_value(&previous.preferences).unwrap())
            };
            changes.push((
                file,
                old,
                Some(serde_json::to_value(&self.preferences).unwrap()),
            ));
        }
        for (file, old, _) in &changes {
            let mut actual = optional_json(file)?;
            if file.extension().is_some_and(|s| s == "omscht") && actual.is_some() {
                let task: Task =
                    serde_json::from_value(actual.take().unwrap()).map_err(|e| e.to_string())?;
                actual = Some(serde_json::to_value(task).unwrap());
            }
            if &actual != old {
                return Err(format!(
                    "{} changed on disk; reload before editing",
                    file.display()
                ));
            }
        }
        for (i, (file, _, new)) in changes.iter().enumerate() {
            let result = match new {
                Some(v) => atomic_json(file, v),
                None => fs::remove_file(file).map_err(|e| e.to_string()),
            };
            if let Err(e) = result {
                let mut errors = vec![e];
                for (file, old, _) in changes[..i].iter().rev() {
                    let result = match old {
                        Some(v) => atomic_json(file, v),
                        None => fs::remove_file(file).map_err(|e| e.to_string()),
                    };
                    if let Err(e) = result {
                        errors.push(format!("Rollback {}: {e}", file.display()));
                    }
                }
                return Err(errors.join("; "));
            }
        }
        Ok(())
    }
}
pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if m.file_type().is_symlink() || m.uid() != unsafe { libc::geteuid() } {
            return Err(
                "State/runtime directory must be owned by this user and not a symlink".into(),
            );
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn data_path() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok_or("Set HOME or XDG_DATA_HOME")?;
    Ok(base.join("omarsket"))
}
