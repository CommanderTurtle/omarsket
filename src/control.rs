//! Private local IPC for controllers sharing the plugin's existing engine.
#[cfg(unix)]
mod unix {
    use crate::{engine::Engine, model::Result, store::private_dir};
    use serde_json::{Value, json};
    use std::{
        fs,
        io::{BufRead, BufReader, Read, Write},
        os::unix::{
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };
    type Handler = fn(&Arc<Engine>, &Value) -> Result<Value>;
    pub struct ControlServer {
        path: PathBuf,
        stopped: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
    }
    impl ControlServer {
        pub fn start(path: PathBuf, engine: Arc<Engine>, handler: Handler) -> Result<Self> {
            if !path.is_absolute() {
                return Err("Control socket path must be absolute".into());
            }
            private_dir(
                path.parent()
                    .ok_or("Control socket needs a private parent directory")?,
            )?;
            if let Ok(meta) = fs::symlink_metadata(&path) {
                if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } {
                    return Err("Refusing to replace a non-socket or another user's socket".into());
                }
                match UnixStream::connect(&path) {
                    Ok(_) => return Err("An Omarsket control socket is already active".into()),
                    Err(e)
                        if [
                            std::io::ErrorKind::ConnectionRefused,
                            std::io::ErrorKind::NotFound,
                        ]
                        .contains(&e.kind()) =>
                    {
                        fs::remove_file(&path).map_err(|e| e.to_string())?
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
            listener.set_nonblocking(true).map_err(|e| e.to_string())?;
            let stopped = Arc::new(AtomicBool::new(false));
            let stop = stopped.clone();
            let clients = Arc::new(AtomicUsize::new(0));
            let worker = thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) && !engine.shutdown.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if clients.fetch_add(1, Ordering::Relaxed) >= 16 {
                                clients.fetch_sub(1, Ordering::Relaxed);
                                continue;
                            }
                            let engine = engine.clone();
                            let clients = clients.clone();
                            thread::spawn(move || {
                                serve(stream, &engine, handler);
                                clients.fetch_sub(1, Ordering::Relaxed);
                            });
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(_) => break,
                    }
                }
            });
            Ok(Self {
                path,
                stopped,
                worker: Some(worker),
            })
        }
    }
    fn serve(mut stream: UnixStream, engine: &Arc<Engine>, handler: Handler) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
        let mut line = String::new();
        let reply = (|| -> Result<Value> {
            BufReader::new(&mut stream)
                .take(2 * 1024 * 1024 + 1)
                .read_line(&mut line)
                .map_err(|e| e.to_string())?;
            if line.len() > 2 * 1024 * 1024 || !line.ends_with('\n') {
                return Err("Control request exceeds 2 MiB or is incomplete".into());
            }
            let r: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if engine.shutdown.load(Ordering::Relaxed) {
                return Err("Engine is shutting down".into());
            }
            Ok(match handler(engine, &r) {
                Ok(result) => json!({"id":r["id"],"ok":true,"result":result}),
                Err(error) => json!({"id":r["id"],"ok":false,"error":error}),
            })
        })()
        .unwrap_or_else(|error| json!({"ok":false,"error":error}));
        let _ = writeln!(stream, "{reply}");
    }
    impl Drop for ControlServer {
        fn drop(&mut self) {
            self.stopped.store(true, Ordering::Relaxed);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
            let _ = fs::remove_file(&self.path);
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn handle(engine: &Arc<Engine>, r: &Value) -> Result<Value> {
            match r["method"].as_str() {
                Some("snapshot") => Ok(engine.snapshot()),
                _ => Err("Unknown request".into()),
            }
        }
        #[test]
        fn private_socket_lifecycle_and_shared_engine() {
            let dir = std::env::temp_dir().join(format!(
                "oms-ipc-{}-{}",
                std::process::id(),
                crate::engine::now_ms()
            ));
            let engine = Engine::new(dir.join("state"), true, Arc::new(|_| {})).unwrap();
            let path = dir.join("socket/control.sock");
            let server = ControlServer::start(path.clone(), engine.clone(), handle).unwrap();
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert!(ControlServer::start(path.clone(), engine.clone(), handle).is_err());
            let mut socket = UnixStream::connect(&path).unwrap();
            writeln!(socket, "{}", json!({"id":4,"method":"snapshot"})).unwrap();
            let mut line = String::new();
            BufReader::new(socket).read_line(&mut line).unwrap();
            let response: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(response["id"], 4);
            assert_eq!(response["result"]["session"], engine.session);
            drop(server);
            assert!(!path.exists());
            fs::write(&path, "not a socket").unwrap();
            assert!(ControlServer::start(path.clone(), engine, handle).is_err());
            assert_eq!(fs::read_to_string(path).unwrap(), "not a socket");
            fs::remove_dir_all(dir).unwrap();
        }
    }
}
#[cfg(unix)]
pub use unix::ControlServer;
