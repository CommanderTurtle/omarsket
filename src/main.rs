use omarsket::{
    backend::Backend,
    engine::Engine,
    model::{Result, Snippet, Task, valid_id},
    store::data_path,
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
    let f = fs::File::open(path).map_err(|e| e.to_string())?;
    if f.metadata().map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
        return Err("Import exceeds 16 MiB".into());
    }
    serde_json::from_reader(f).map_err(|e| e.to_string())
}
fn export_file(path: &str, value: &Value) -> Result<Value> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(value).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(json!({"path":path}))
}
fn request(engine: &Arc<Engine>, r: &Value) -> Result<Value> {
    match string(r, "method")? {
        "snapshot" => Ok(engine.snapshot()),
        "save" => {
            let t: Task = serde_json::from_value(r["task"].clone()).map_err(|e| e.to_string())?;
            t.validate()?;
            engine.edit(|s| {
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
        "stop" => {
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
                serde_json::to_value(t).unwrap()
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
                "[Desktop Entry]\nType=Application\nName=Omarsket — {name}\nExec={} run {}\nIcon=input-keyboard\nTerminal=false\n",
                quote(&binary.to_string_lossy()),
                quote(name)
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
            "Omarsket\n  omarsket session [--dry-run] [--library PATH]\n  omarsket run TASK [--library PATH]\n  omarsket validate FILE\n\nSession accepts JSON-lines on stdin and returns replies/events on stdout. No network listener."
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
    let path = if let Some(index) = args.iter().position(|a| a == "--library") {
        PathBuf::from(args.get(index + 1).ok_or("--library needs a path")?)
    } else {
        data_path()?
    };
    let engine = Engine::new(path, args.iter().any(|a| a == "--dry-run"), emit.clone())?;
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
