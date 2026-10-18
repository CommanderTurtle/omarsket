use omarsket::{
    backend::Backend,
    engine::Engine,
    model::{Action, Result, Snippet, Task, task_name, valid_id},
    store::{Store, data_path, read_json as read_file_json},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, BufRead, Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
static TERMINATE: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
extern "C" fn interrupted(_: i32) {
    TERMINATE.store(true, Ordering::Relaxed);
}
fn string<'a>(request: &'a Value, key: &str) -> Result<&'a str> {
    request[key]
        .as_str()
        .ok_or_else(|| format!("Missing string: {key}"))
}
fn read_json(path: &str) -> Result<Value> {
    read_file_json(std::path::Path::new(path))
}
fn export_file(path: &str, value: &Value) -> Result<Value> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Export exceeds the 16 MiB import limit; export schedules individually".into());
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error.to_string());
    }
    Ok(json!({"path":path}))
}
fn request(engine: &Arc<Engine>, r: &Value) -> Result<Value> {
    match string(r, "method")? {
        "snapshot" => Ok(engine.snapshot()),
        "input-config" => omarsket::input::request(r),
        "input-router" => omarsket::remap::request(&data_path()?.join("input-profiles"), r),
        "hardware-infer" => omarsket::hardware::infer(r),
        "reload" => engine.reload(),
        "validate-task" => {
            let task: Task =
                serde_json::from_value(r["task"].clone()).map_err(|e| e.to_string())?;
            task.validate()?;
            Ok(serde_json::to_value(task).unwrap())
        }
        "preference" => {
            let key = string(r, "name")?;
            if !["auto_scroll", "pinned"].contains(&key) {
                return Err("Unknown preference".into());
            }
            let value = r["value"].as_bool().ok_or("Expected boolean preference")?;
            engine.edit(|s| {
                s.preferences.insert(key.into(), value);
                Ok(json!(true))
            })
        }
        "save" => {
            let t: Task = serde_json::from_value(r["task"].clone()).map_err(|e| e.to_string())?;
            t.validate()?;
            engine.edit(|s| {
                if let Some(old) = r["previous"].as_str().filter(|old| *old != t.id) {
                    if s.tasks.contains_key(&t.id) {
                        return Err("Schedule name already exists".into());
                    }
                    s.tasks.remove(old).ok_or("Original schedule not found")?;
                    for task in s.tasks.values_mut() {
                        for action in &mut task.actions {
                            if let Action::Task { name, .. } = action
                                && task_name(name) == old
                            {
                                *name = t.id.clone();
                            }
                        }
                    }
                } else if r["create"] == true && s.tasks.contains_key(&t.id) {
                    return Err("Schedule name already exists".into());
                }
                s.tasks.insert(t.id.clone(), t);
                Ok(json!(true))
            })
        }
        "delete" => {
            let id = string(r, "name")?;
            engine.edit(|s| {
                s.tasks.remove(id).ok_or("Task not found")?;
                Ok(json!(true))
            })
        }
        "run" => engine.queue(string(r, "name")?, r["delay"].as_u64(), r["loop"].as_i64()),
        "run-temporary" => {
            let t: Task = serde_json::from_value(r["task"].clone()).map_err(|e| e.to_string())?;
            engine.queue_task(t, true)
        }
        "duplicate" => {
            let old = string(r, "name")?;
            let name = string(r, "new_name")?;
            valid_id(name)?;
            engine.edit(|s| {
                if s.tasks.contains_key(name) {
                    return Err("Schedule name already exists".into());
                }
                let mut t = s.tasks.get(old).ok_or("Schedule not found")?.clone();
                t.id = name.into();
                t.auto_start = false;
                s.tasks.insert(name.into(), t);
                Ok(json!(true))
            })
        }
        "stop" => {
            if !r["session"].is_null() && r["session"] != engine.session {
                return Err("Engine session changed; task was not stopped".into());
            }
            engine.stop(r["number"].as_u64());
            Ok(json!(true))
        }
        "import" => {
            let t = Task::import(string(r, "name")?, read_json(string(r, "path")?)?)?;
            engine.edit(|s| {
                if s.tasks.contains_key(&t.id) {
                    return Err("Task exists; choose a different import name".into());
                }
                s.tasks.insert(t.id.clone(), t);
                Ok(json!(true))
            })
        }
        "export" => {
            let s = engine.store.lock().unwrap();
            let t = s.tasks.get(string(r, "name")?).ok_or("Task not found")?;
            let value = if r["format"] == "tasket" {
                t.tasket_json(&s.data)?
            } else {
                let mut portable = t.clone();
                for action in &mut portable.actions {
                    *action = action.resolve(&s.data)?;
                }
                serde_json::to_value(portable).unwrap()
            };
            if let Some(path) = r["path"].as_str() {
                export_file(path, &value)
            } else {
                Ok(value)
            }
        }
        "shortcut" => {
            let name = string(r, "name")?;
            valid_id(name)?;
            if !engine.store.lock().unwrap().tasks.contains_key(name) {
                return Err("Task not found".into());
            }
            let path = string(r, "path")?;
            let quote = |s: &str| {
                format!(
                    "\"{}\"",
                    s.replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('`', "\\`")
                        .replace('$', "\\$")
                        .replace('%', "%%")
                )
            };
            let binary = std::env::current_exe().map_err(|e| e.to_string())?;
            let text = format!(
                "[Desktop Entry]\nType=Application\nName=Omarsket — {name}\nExec={} run {} --directory {}\nIcon=input-keyboard\nTerminal=false\n",
                quote(&binary.to_string_lossy()),
                quote(name),
                quote(
                    engine.snapshot()["directory"]
                        .as_str()
                        .ok_or("Invalid schedule directory")?
                )
            );
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| e.to_string())?;
            file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            Ok(json!({"path":path}))
        }
        "rescale" => {
            let name = string(r, "name")?;
            let old: [u32; 2] =
                serde_json::from_value(r["old"].clone()).map_err(|e| e.to_string())?;
            let new: [u32; 2] =
                serde_json::from_value(r["new"].clone()).map_err(|e| e.to_string())?;
            engine.edit(|s| {
                s.tasks
                    .get_mut(name)
                    .ok_or("Task not found")?
                    .rescale(old, new, &s.data)?;
                Ok(json!(true))
            })
        }
        "save-data" => {
            let name = string(r, "name")?.to_string();
            valid_id(&name)?;
            let item: Snippet =
                serde_json::from_value(r["item"].clone()).map_err(|e| e.to_string())?;
            item.validate()?;
            engine.edit(|s| {
                s.data.insert(name, item);
                Ok(json!(true))
            })
        }
        "delete-data" => {
            let name = string(r, "name")?;
            engine.edit(|s| {
                s.data.remove(name).ok_or("Data not found")?;
                Ok(json!(true))
            })
        }
        "export-data" => {
            let data = serde_json::to_value(&engine.store.lock().unwrap().data).unwrap();
            export_file(string(r, "path")?, &data)
        }
        "import-data" => {
            let data: std::collections::BTreeMap<String, Snippet> =
                serde_json::from_value(read_json(string(r, "path")?)?)
                    .map_err(|e| e.to_string())?;
            for (id, item) in &data {
                valid_id(id)?;
                item.validate()?;
            }
            engine.edit(|s| {
                for (id, item) in data {
                    if s.data.contains_key(&id) {
                        return Err(format!("Data {id} exists"));
                    }
                    s.data.insert(id, item);
                }
                Ok(json!(true))
            })
        }
        "export-library" => {
            let store = engine.store.lock().unwrap();
            export_file(string(r, "path")?, &serde_json::to_value(&*store).unwrap())
        }
        "import-library" => {
            let mut incoming: Store = serde_json::from_value(read_json(string(r, "path")?)?)
                .map_err(|e| e.to_string())?;
            incoming.validate()?;
            for task in incoming.tasks.values_mut() {
                task.auto_start = false;
                task.allow_system = false;
            }
            engine.edit(|s| {
                for id in incoming.tasks.keys() {
                    if s.tasks.contains_key(id) {
                        return Err(format!("Schedule {id} already exists"));
                    }
                }
                for id in incoming.data.keys() {
                    if s.data.contains_key(id) {
                        return Err(format!("Data {id} already exists"));
                    }
                }
                s.tasks.extend(incoming.tasks);
                s.data.extend(incoming.data);
                Ok(json!(true))
            })
        }
        "import-folder" => {
            let folder = std::path::Path::new(string(r, "path")?);
            let mut incoming = Vec::new();
            for entry in fs::read_dir(folder).map_err(|e| e.to_string())? {
                let file = entry.map_err(|e| e.to_string())?.path();
                if file.extension().is_none_or(|s| {
                    !s.eq_ignore_ascii_case("scht") && !s.eq_ignore_ascii_case("omscht")
                }) {
                    continue;
                }
                let name = file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid filename")?;
                incoming.push(Task::import(name, read_file_json(&file)?)?);
            }
            if incoming.is_empty() {
                return Err("No .scht or .omscht files in this folder".into());
            }
            engine.edit(|s| {
                let count = incoming.len();
                for task in incoming {
                    if s.tasks.contains_key(&task.id) {
                        return Err(format!("Schedule {} already exists", task.id));
                    }
                    s.tasks.insert(task.id.clone(), task);
                }
                Ok(json!({"imported":count}))
            })
        }
        "doctor" | "cursor" => {
            let mut backend = Backend::new(false)?;
            backend.call(&json!({"type":r["method"]}), &AtomicBool::new(false))
        }
        "shutdown" => {
            engine.close();
            Ok(json!(true))
        }
        _ => Err("Unknown request method".into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_none_or(|s| s == "--help" || s == "help") {
        println!(
            "Omarsket\n  omarsket session [--dry-run] [--directory PATH] [--control-socket PATH]\n  omarsket run TASK [--directory PATH]\n  omarsket validate FILE\n\nIndividual schedules: PATH/schedules/*.omscht. Legacy --library FILE migrates on open.\nSession accepts JSON-lines on stdin and returns replies/events on stdout. No TCP listener."
        );
        return Ok(());
    }
    if args[0] == "validate" {
        let path = args.get(1).ok_or("Expected task JSON file")?;
        Task::import("validation", read_json(path)?)?;
        println!("valid");
        return Ok(());
    }
    if !["session", "run"].contains(&args[0].as_str()) {
        return Err("Expected session, run, or validate".into());
    }
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGTERM, interrupted as *const () as usize);
        libc::signal(libc::SIGINT, interrupted as *const () as usize);
    }
    let output = Arc::new(Mutex::new(io::stdout()));
    let emit = {
        let output = output.clone();
        Arc::new(move |v: Value| {
            if let Ok(mut out) = output.lock() {
                let _ = writeln!(out, "{v}");
                let _ = out.flush();
            }
        })
    };
    let path = if let Some(index) = args
        .iter()
        .position(|a| a == "--directory" || a == "--library")
    {
        PathBuf::from(args.get(index + 1).ok_or("Storage option needs a path")?)
    } else {
        data_path()?
    };
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    let engine = Engine::new(path, args.iter().any(|a| a == "--dry-run"), emit.clone())?;
    #[cfg(unix)]
    let _control = if let Some(index) = args.iter().position(|a| a == "--control-socket") {
        Some(omarsket::control::ControlServer::start(
            PathBuf::from(args.get(index + 1).ok_or("--control-socket needs a path")?),
            engine.clone(),
            request,
        )?)
    } else {
        None
    };
    #[cfg(not(unix))]
    if args.iter().any(|a| a == "--control-socket") {
        return Err("Control sockets require Unix".into());
    }
    if args[0] == "run" {
        let id = args.get(1).ok_or("Expected saved task name")?;
        engine.queue(id, None, None)?;
        let worker = engine.start();
        while !TERMINATE.load(Ordering::Relaxed)
            && engine
                .jobs
                .lock()
                .unwrap()
                .iter()
                .any(|j| ["scheduled", "running"].contains(&j.state.as_str()))
        {
            thread::sleep(Duration::from_millis(20));
        }
        engine.close();
        let _ = worker.join();
        if engine
            .jobs
            .lock()
            .unwrap()
            .iter()
            .any(|j| j.state == "failed")
        {
            return Err("Task failed; see job error".into());
        }
        return Ok(());
    }
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = io::stdin().lock();
        loop {
            let mut line = String::new();
            match reader
                .by_ref()
                .take(2 * 1024 * 1024 + 1)
                .read_line(&mut line)
            {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if line.len() > 2 * 1024 * 1024 {
                        let _ = sender.send(Err("Request exceeds 2 MiB".into()));
                        break;
                    }
                    if sender
                        .send(serde_json::from_str::<Value>(&line).map_err(|e| e.to_string()))
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    });
    emit(json!({"event":"ready","snapshot":engine.snapshot()}));
    let worker = engine.start();
    engine.startup();
    while !TERMINATE.load(Ordering::Relaxed) && !engine.shutdown.load(Ordering::Relaxed) {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(r)) => {
                let result = request(&engine, &r);
                emit(match result {
                    Ok(value) => json!({"id":r["id"],"ok":true,"result":value}),
                    Err(error) => json!({"id":r["id"],"ok":false,"error":error}),
                });
            }
            Ok(Err(error)) => emit(json!({"ok":false,"error":error})),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(_) => break,
        }
    }
    engine.close();
    let _ = worker.join();
    Ok(())
}
