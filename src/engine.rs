use crate::{
    backend::Backend,
    model::{Action, Result, Task, loops, task_name},
    store::{Store, root_path},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Job {
    pub session: String,
    pub number: u64,
    pub task: String,
    pub state: String,
    pub due_ms: u64,
    pub action: usize,
    pub completed_loops: u64,
    pub error: String,
    pub temporary: bool,
    pub total_actions: usize,
    pub loop_times: i64,
    pub created_ms: u64,
    pub started_ms: Option<u64>,
    pub finished_ms: Option<u64>,
    pub delay_seconds: u64,
    pub description: String,
    #[serde(skip)]
    pub cancel: Arc<AtomicBool>,
    #[serde(skip)]
    plan: Option<Plan>,
}
#[derive(Clone)]
struct Plan {
    task: Task,
    children: BTreeMap<String, Plan>,
}
impl Plan {
    fn make(
        task: &Task,
        store: &Store,
        chain: &mut BTreeSet<String>,
        depth: usize,
        budget: &mut usize,
    ) -> Result<Self> {
        if depth >= 16 || !chain.insert(task.id.clone()) {
            return Err("Nested task cycle or depth greater than 16".into());
        }
        task.validate()?;
        *budget = budget
            .checked_add(task.actions.len())
            .ok_or("Task graph too large")?;
        if *budget > 100000 {
            return Err("Expanded task graph exceeds 100000 actions".into());
        }
        if task.actions.is_empty() {
            return Err(format!("{} has no actions", task.id));
        }
        let mut task = task.clone();
        let mut children = BTreeMap::new();
        for action in &mut task.actions {
            *action = action.resolve(&store.data)?;
            if action.needs_permission() && !task.allow_system {
                return Err(format!(
                    "{}: enable system/file command permission before running",
                    task.id
                ));
            }
            if let Action::Task { name, .. } = action {
                let id = task_name(name).to_string();
                if children.contains_key(&id) {
                    continue;
                }
                let child = store
                    .tasks
                    .get(&id)
                    .ok_or_else(|| format!("Task not found: {id}"))?;
                children.insert(id, Self::make(child, store, chain, depth + 1, budget)?);
            }
        }
        chain.remove(&task.id);
        Ok(Self { task, children })
    }
}
type Emitter = Arc<dyn Fn(Value) + Send + Sync>;
pub struct Engine {
    pub session: String,
    pub store: Mutex<Store>,
    pub jobs: Mutex<Vec<Job>>,
    path: PathBuf,
    next: AtomicU64,
    pub shutdown: AtomicBool,
    dry_run: bool,
    emit: Emitter,
    wake: Condvar,
}
impl Engine {
    pub fn new(path: PathBuf, dry_run: bool, emit: Emitter) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            session: format!(
                "{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
            store: Mutex::new(Store::load(&path)?),
            jobs: Mutex::new(vec![]),
            path,
            next: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
            dry_run,
            emit,
            wake: Condvar::new(),
        }))
    }
    pub fn snapshot(&self) -> Value {
        let store = self.store.lock().unwrap();
        json!({"session":self.session,"tasks":store.tasks.values().collect::<Vec<_>>(),"data":store.data,"jobs":*self.jobs.lock().unwrap(),"dry_run":self.dry_run,"directory":root_path(&self.path),"notices":store.notices,"preferences":store.preferences})
    }
    fn changed(&self) {
        (self.emit)(json!({"event":"jobs","jobs":*self.jobs.lock().unwrap()}));
    }
    pub fn edit<F>(&self, f: F) -> Result<Value>
    where
        F: FnOnce(&mut Store) -> Result<Value>,
    {
        let mut guard = self.store.lock().unwrap();
        let mut next = guard.clone();
        let result = f(&mut next)?;
        next.save_changes(&self.path, &guard)?;
        *guard = next;
        drop(guard);
        (self.emit)(json!({"event":"store","snapshot":self.snapshot()}));
        Ok(result)
    }
    pub fn queue(&self, id: &str, delay: Option<u64>, repeat: Option<i64>) -> Result<Value> {
        let store = self.store.lock().unwrap();
        let mut task = store.tasks.get(id).ok_or("Task not found")?.clone();
        drop(store);
        if let Some(repeat) = repeat {
            loops(repeat)?;
            task.loop_times = repeat;
        }
        if let Some(delay) = delay {
            task.delay_seconds = delay;
        }
        self.queue_task(task, false)
    }
    pub fn reload(&self) -> Result<Value> {
        let mut guard = self.store.lock().unwrap();
        *guard = Store::load(&self.path)?;
        drop(guard);
        let snapshot = self.snapshot();
        (self.emit)(json!({"event":"store","snapshot":snapshot}));
        Ok(snapshot)
    }
    pub fn queue_task(&self, task: Task, temporary: bool) -> Result<Value> {
        let store = self.store.lock().unwrap();
        let plan = Plan::make(&task, &store, &mut BTreeSet::new(), 0, &mut 0)?;
        drop(store);
        let mut jobs = self.jobs.lock().unwrap();
        if jobs
            .iter()
            .filter(|j| ["scheduled", "running"].contains(&j.state.as_str()))
            .count()
            >= 1024
        {
            return Err("1024 tasks already queued".into());
        }
        if jobs.len() >= 2048 {
            jobs.retain(|j| ["scheduled", "running"].contains(&j.state.as_str()));
        }
        let job = Job {
            session: self.session.clone(),
            number: self.next.fetch_add(1, Ordering::Relaxed),
            task: task.id.clone(),
            state: "scheduled".into(),
            due_ms: now_ms() + task.delay_seconds * 1000,
            action: 0,
            completed_loops: 0,
            error: String::new(),
            temporary,
            total_actions: task.actions.len(),
            loop_times: task.loop_times,
            created_ms: now_ms(),
            started_ms: None,
            finished_ms: None,
            delay_seconds: task.delay_seconds,
            description: task.description.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            plan: Some(plan),
        };
        let result = json!(&job);
        jobs.push(job);
        drop(jobs);
        self.wake.notify_all();
        self.changed();
        Ok(result)
    }
    pub fn stop(&self, number: Option<u64>) {
        for job in self.jobs.lock().unwrap().iter_mut() {
            if number.is_none_or(|id| job.number == id)
                && ["scheduled", "running"].contains(&job.state.as_str())
            {
                job.cancel.store(true, Ordering::Relaxed);
                job.state = "stopped".into();
                job.finished_ms = Some(now_ms());
                job.plan = None;
            }
        }
        if number.is_none() {
            (self.emit)(json!({"event":"stopping"}));
        }
        self.wake.notify_all();
        self.changed();
    }
    pub fn close(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.stop(None);
    }
    fn update<F: FnOnce(&mut Job)>(&self, id: u64, f: F) {
        if let Some(job) = self
            .jobs
            .lock()
            .unwrap()
            .iter_mut()
            .find(|j| j.number == id)
        {
            f(job)
        }
        self.changed();
    }
    fn wait(&self, seconds: f64, cancel: &AtomicBool) -> Result<()> {
        let deadline = std::time::Instant::now() + Duration::from_secs_f64(seconds);
        loop {
            if cancel.load(Ordering::Relaxed) || self.shutdown.load(Ordering::Relaxed) {
                return Err("Stopped".into());
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Ok(());
            }
            thread::sleep(left.min(Duration::from_millis(10)));
        }
    }
    fn execute(
        &self,
        plan: &Plan,
        repeat: i64,
        backend: &mut Backend,
        job: &Job,
        depth: usize,
    ) -> Result<()> {
        let mut completed = 0;
        while repeat < 0 || completed < repeat as u64 {
            for (index, action) in plan.task.actions.iter().enumerate() {
                self.wait(0.0, &job.cancel)?;
                if depth == 0 {
                    self.update(job.number, |j| j.action = index);
                }
                match action {
                    Action::Wait { duration } => self.wait(*duration, &job.cancel)?,
                    Action::Task {
                        name,
                        delay,
                        repeat,
                    } => {
                        self.wait(*delay as f64, &job.cancel)?;
                        self.execute(
                            &plan.children[task_name(name)],
                            *repeat,
                            backend,
                            job,
                            depth + 1,
                        )?;
                    }
                    _ => {
                        let result = backend.call(
                            &serde_json::to_value(action).map_err(|e| e.to_string())?,
                            &job.cancel,
                        )?;
                        if result["quit"] == true {
                            self.close();
                            return Err("Stopped".into());
                        }
                    }
                }
            }
            completed += 1;
            if depth == 0 {
                self.update(job.number, |j| j.completed_loops = completed);
            }
            self.wait(0.002, &job.cancel)?;
        }
        Ok(())
    }
    pub fn run_next(&self) -> bool {
        let job = {
            let mut jobs = self.jobs.lock().unwrap();
            let next = jobs
                .iter_mut()
                .filter(|j| j.state == "scheduled" && j.due_ms <= now_ms())
                .min_by_key(|j| (j.due_ms, j.number));
            match next {
                Some(job) => {
                    job.state = "running".into();
                    job.started_ms = Some(now_ms());
                    Some(job.clone())
                }
                None => None,
            }
        };
        let Some(job) = job else { return false };
        self.changed();
        let plan = job.plan.as_ref().unwrap();
        let result = Backend::new(self.dry_run).and_then(|mut backend| {
            self.execute(plan, plan.task.loop_times, &mut backend, &job, 0)
        });
        self.update(job.number, |j| {
            j.plan = None;
            j.finished_ms = Some(now_ms());
            if j.cancel.load(Ordering::Relaxed) || self.shutdown.load(Ordering::Relaxed) {
                j.state = "stopped".into();
            } else {
                match result {
                    Ok(()) => j.state = "finished".into(),
                    Err(e) => {
                        j.state = "failed".into();
                        j.error = e;
                    }
                }
            }
        });
        true
    }
    pub fn start(self: &Arc<Self>) -> thread::JoinHandle<()> {
        let engine = self.clone();
        thread::spawn(move || {
            while !engine.shutdown.load(Ordering::Relaxed) {
                if !engine.run_next() {
                    let jobs = engine.jobs.lock().unwrap();
                    if engine.shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    let next = jobs
                        .iter()
                        .filter(|j| j.state == "scheduled")
                        .map(|j| j.due_ms)
                        .min();
                    let wait = next
                        .map(|due| Duration::from_millis(due.saturating_sub(now_ms()).max(1)))
                        .unwrap_or(Duration::from_secs(60));
                    let _guard = engine
                        .wake
                        .wait_timeout(jobs, wait.min(Duration::from_secs(60)))
                        .unwrap();
                }
            }
        })
    }
    pub fn startup(&self) {
        let ids: Vec<_> = self
            .store
            .lock()
            .unwrap()
            .tasks
            .values()
            .filter(|t| t.auto_start)
            .map(|t| t.id.clone())
            .collect();
        for id in ids {
            if let Err(error) = self.queue(&id, None, None) {
                (self.emit)(json!({"event":"error","error":error}));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine() -> Arc<Engine> {
        Engine::new(
            std::env::temp_dir().join(format!("omarsket-test-{}-{}", std::process::id(), now_ms())),
            true,
            Arc::new(|_| {}),
        )
        .unwrap()
    }
    fn task(id: &str) -> Task {
        Task::import(
            id,
            json!({"docType":"ScheduleTask File","actions":[{"type":"wait","duration":0}]}),
        )
        .unwrap()
    }
    #[test]
    fn fifo_loops_and_stop() {
        let e = engine();
        e.store.lock().unwrap().tasks.insert("a".into(), task("a"));
        e.queue("a", Some(0), Some(3)).unwrap();
        e.queue("a", Some(0), None).unwrap();
        assert!(e.run_next());
        let jobs = e.jobs.lock().unwrap();
        assert_eq!(jobs[0].completed_loops, 3);
        assert_eq!(jobs[1].state, "scheduled");
        drop(jobs);
        e.stop(None);
        assert!(!e.run_next());
    }
    #[test]
    fn permission_checked_before_queue() {
        let e = engine();
        let mut t = task("a");
        t.actions.push(Action::System {
            command: "executeprogram".into(),
            param1: "/bin/true".into(),
            param2: String::new(),
        });
        e.store.lock().unwrap().tasks.insert("a".into(), t);
        assert!(e.queue("a", None, None).is_err());
        assert!(e.jobs.lock().unwrap().is_empty());
    }
    #[test]
    fn rejects_cycles_before_input() {
        let e = engine();
        let mut t = task("a");
        t.actions.push(Action::Task {
            name: "a".into(),
            delay: 0,
            repeat: 1,
        });
        e.store.lock().unwrap().tasks.insert("a".into(), t);
        assert!(e.queue("a", None, None).is_err());
    }
    #[test]
    fn stop_interrupts_wait() {
        let e = engine();
        let mut t = task("a");
        t.actions = vec![Action::Wait { duration: 60.0 }];
        e.store.lock().unwrap().tasks.insert("a".into(), t);
        e.queue("a", None, None).unwrap();
        let runner = {
            let e = e.clone();
            thread::spawn(move || e.run_next())
        };
        thread::sleep(Duration::from_millis(30));
        e.stop(None);
        runner.join().unwrap();
        assert_eq!(e.jobs.lock().unwrap()[0].state, "stopped");
    }
    #[test]
    fn external_stop_notifies_the_panel_to_cancel_delayed_runs() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let e = Engine::new(
            std::env::temp_dir().join(format!(
                "omarsket-stop-event-{}-{}",
                std::process::id(),
                now_ms()
            )),
            true,
            Arc::new(move |event| sink.lock().unwrap().push(event)),
        )
        .unwrap();
        e.stop(Some(1));
        assert!(
            !events
                .lock()
                .unwrap()
                .iter()
                .any(|e| e["event"] == "stopping")
        );
        events.lock().unwrap().clear();
        e.stop(None);
        assert_eq!(events.lock().unwrap()[0]["event"], "stopping");
    }
}
