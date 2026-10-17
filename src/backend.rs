use crate::model::Result;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

pub struct Backend {
    child: Option<Child>,
    input: Option<ChildStdin>,
    replies: Option<mpsc::Receiver<String>>,
    dry_run: bool,
}
impl Backend {
    pub fn new(dry_run: bool) -> Result<Self> {
        if dry_run {
            return Ok(Self {
                child: None,
                input: None,
                replies: None,
                dry_run,
            });
        }
        let root = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .and_then(|p| p.parent())
            .ok_or("Cannot locate plugin root")?
            .to_path_buf();
        let worker = std::env::var_os("OMARSKET_WORKER")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("backend/worker.py"));
        let python = std::env::var_os("OMARSKET_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("backend/.venv/bin/python"));
        if !worker.is_file() || !python.is_file() {
            return Err("Python backend is not initialized. Run bash build-plugin.sh in the plugin checkout (or set OMARSKET_WORKER and OMARSKET_PYTHON).".into());
        }
        let mut command = Command::new(python);
        command
            .arg("-u")
            .arg(worker)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let input = child.stdin.take();
        let output = child.stdout.take().ok_or("Worker stdout unavailable")?;
        let (sender, replies) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                match line {
                    Ok(line) => {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            child: Some(child),
            input,
            replies: Some(replies),
            dry_run,
        })
    }
    pub fn call(&mut self, action: &Value, cancelled: &AtomicBool) -> Result<Value> {
        if cancelled.load(Ordering::Relaxed) {
            return Err("Stopped".into());
        }
        if self.dry_run {
            return Ok(serde_json::json!({"dry_run":true}));
        }
        let input = self.input.as_mut().ok_or("Worker stdin unavailable")?;
        writeln!(input, "{action}").map_err(|e| e.to_string())?;
        input.flush().map_err(|e| e.to_string())?;
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Stopped".into());
            }
            match self
                .replies
                .as_ref()
                .unwrap()
                .recv_timeout(Duration::from_millis(20))
            {
                Ok(line) => {
                    let reply: Value = serde_json::from_str(&line)
                        .map_err(|e| format!("Invalid backend response: {e}"))?;
                    if reply["ok"] == true {
                        return Ok(reply["result"].clone());
                    }
                    return Err(reply["error"]
                        .as_str()
                        .unwrap_or("Input backend failed")
                        .into());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(status) = self
                        .child
                        .as_mut()
                        .unwrap()
                        .try_wait()
                        .map_err(|e| e.to_string())?
                    {
                        return Err(format!("Input backend exited: {status}"));
                    }
                }
                Err(_) => return Err("Input backend closed its output".into()),
            }
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            self.input.take();
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGTERM);
            }
            let until = Instant::now() + Duration::from_millis(1000);
            while Instant::now() < until {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
