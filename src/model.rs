use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, String>;
fn one() -> u32 {
    1
}
fn one_loop() -> i64 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Action {
    #[serde(rename = "paste")]
    Paste {
        #[serde(default)]
        content: String,
        #[serde(default, rename = "contentId")]
        data_id: String,
        #[serde(default = "one", rename = "loop")]
        repeat: u32,
    },
    #[serde(rename = "wait")]
    Wait { duration: f64 },
    #[serde(rename = "keyssequence")]
    Keys {
        #[serde(default)]
        keysmap: BTreeMap<String, (u64, Vec<String>)>,
        #[serde(default, rename = "keysSeqId")]
        data_id: String,
        #[serde(default = "one", rename = "loop")]
        repeat: u32,
    },
    #[serde(rename = "cursormovements")]
    Cursor {
        #[serde(default, rename = "cursormovsmap")]
        movements: Vec<[i32; 4]>,
        #[serde(default, rename = "cursorMovsId")]
        data_id: String,
        #[serde(default = "one", rename = "loop")]
        repeat: u32,
        #[serde(default, rename = "optionalkeysstroke")]
        keys: Vec<String>,
    },
    #[serde(rename = "systemcommand")]
    System {
        #[serde(rename = "sysCommandType")]
        command: String,
        #[serde(default, rename = "sysCommandParam1")]
        param1: String,
        #[serde(default, rename = "sysCommandParam2")]
        param2: String,
    },
    #[serde(rename = "runningothertask")]
    Task {
        #[serde(rename = "otherTaskName")]
        name: String,
        #[serde(default, rename = "otherTaskDelay")]
        delay: u64,
        #[serde(default = "one_loop", rename = "otherTaskLoops")]
        repeat: i64,
    },
    #[serde(rename = "move")]
    Move {
        x: i32,
        y: i32,
        #[serde(default)]
        duration_ms: u64,
    },
    #[serde(rename = "click")]
    Click {
        x: i32,
        y: i32,
        button: String,
        #[serde(default = "one")]
        count: u32,
    },
    #[serde(rename = "scroll")]
    Scroll {
        #[serde(default)]
        horizontal: i32,
        #[serde(default)]
        vertical: i32,
    },
    #[serde(rename = "hotkey")]
    Hotkey { keys: Vec<String> },
    #[serde(rename = "type")]
    Type {
        text: String,
        #[serde(default)]
        interval_ms: u64,
    },
}

pub fn loops(value: i64) -> Result<()> {
    if value == -1 || (1..=100_000).contains(&value) {
        Ok(())
    } else {
        Err("Loop count must be 1–100000, or -1 for infinite".into())
    }
}

impl Action {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Wait { duration }
                if !duration.is_finite() || !(0.0..=86400.0).contains(duration) =>
            {
                return Err("Wait duration must be between 0 and 86400 seconds".into());
            }
            Self::Paste { repeat, .. }
            | Self::Keys { repeat, .. }
            | Self::Cursor { repeat, .. }
                if !(1..=100_000).contains(repeat) =>
            {
                return Err("Invalid action repeat count".into());
            }
            Self::Keys { keysmap, .. } => {
                for (at, (hold, keys)) in keysmap {
                    if at.parse::<u64>().is_err()
                        || at.parse::<u64>().unwrap() > 86400000
                        || *hold > 86400000
                        || keys.is_empty()
                    {
                        return Err("Invalid key sequence timing or empty chord".into());
                    }
                }
            }
            Self::Cursor { movements, .. } if movements.iter().any(|m| m[0] < 0 || m[1] < 0) => {
                return Err("Cursor timing cannot be negative".into());
            }
            Self::Task {
                name,
                delay,
                repeat,
            } => {
                valid_id(name.trim_end_matches(".scht"))?;
                loops(*repeat)?;
                if *delay > 86400 {
                    return Err("Task delay exceeds 24 hours".into());
                }
            }
            Self::Click { button, count, .. }
                if !["left", "middle", "right"].contains(&button.as_str())
                    || !(1..=100000).contains(count) =>
            {
                return Err("Invalid mouse button or click count".into());
            }
            Self::Hotkey { keys } if keys.is_empty() => {
                return Err("A hotkey needs at least one key".into());
            }
            Self::Move { duration_ms, .. } if *duration_ms > 86400000 => {
                return Err("Movement duration exceeds 24 hours".into());
            }
            Self::System { command, .. }
                if ![
                    "shutdown",
                    "restart",
                    "logoff",
                    "changevolume",
                    "changeaudiodevice",
                    "killprocess",
                    "quitselfprogram",
                    "createfolder",
                    "deletefolder",
                    "createfile",
                    "deletefile",
                    "screenshot",
                    "printscreen",
                    "openfile",
                    "executeprogram",
                    "openurl",
                    "openfolder",
                    "copyfile",
                ]
                .contains(&command.as_str()) =>
            {
                return Err(format!("Unsupported system command: {command}"));
            }
            _ => (),
        }
        Ok(())
    }

    pub fn needs_permission(&self) -> bool {
        matches!(self, Self::System { .. })
    }

    pub fn resolve(&self, data: &BTreeMap<String, Snippet>) -> Result<Self> {
        let mut action = self.clone();
        let (id, kind) = match &action {
            Self::Paste { data_id, .. } => (data_id.clone(), "paste"),
            Self::Keys { data_id, .. } => (data_id.clone(), "keyssequence"),
            Self::Cursor { data_id, .. } => (data_id.clone(), "cursormovements"),
            _ => return Ok(action),
        };
        // Tasket embeds the saved data alongside its reference. A missing external
        // library entry must not discard the embedded, portable copy.
        if let Some(item) = data.get(&id) {
            if item.kind != kind {
                return Err(format!("Data {id} is not {kind}"));
            }
            match &mut action {
                Self::Paste { content, .. } => {
                    *content =
                        serde_json::from_value(item.value.clone()).map_err(|e| e.to_string())?
                }
                Self::Keys { keysmap, .. } => {
                    *keysmap =
                        serde_json::from_value(item.value.clone()).map_err(|e| e.to_string())?
                }
                Self::Cursor { movements, .. } => {
                    *movements =
                        serde_json::from_value(item.value.clone()).map_err(|e| e.to_string())?
                }
                _ => unreachable!(),
            }
        }
        action.validate()?;
        Ok(action)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub delay_seconds: u64,
    #[serde(default = "one_loop")]
    pub loop_times: i64,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub allow_system: bool,
    pub actions: Vec<Action>,
}

impl Task {
    pub fn validate(&self) -> Result<()> {
        valid_id(&self.id)?;
        loops(self.loop_times)?;
        if self.delay_seconds > 86400 {
            return Err("Task delay exceeds 24 hours".into());
        }
        if self.actions.len() > 10000 {
            return Err("A task may contain at most 10000 actions".into());
        }
        for action in &self.actions {
            action.validate()?;
        }
        Ok(())
    }
    pub fn import(id: &str, value: Value) -> Result<Self> {
        valid_id(id)?;
        let mut task = if value.get("docType").and_then(Value::as_str) == Some("ScheduleTask File")
        {
            Self {
                id: id.into(),
                description: value["description"].as_str().unwrap_or("").into(),
                delay_seconds: 0,
                loop_times: 1,
                auto_start: false,
                allow_system: false,
                actions: serde_json::from_value(value["actions"].clone())
                    .map_err(|e| e.to_string())?,
            }
        } else {
            serde_json::from_value(value).map_err(|e| e.to_string())?
        };
        task.id = id.into();
        task.auto_start = false;
        task.allow_system = false;
        task.validate()?;
        Ok(task)
    }
    pub fn tasket_json(&self, data: &BTreeMap<String, Snippet>) -> Result<Value> {
        if self.actions.iter().any(|a| {
            !matches!(
                a,
                Action::Paste { .. }
                    | Action::Wait { .. }
                    | Action::Keys { .. }
                    | Action::Cursor { .. }
                    | Action::System { .. }
                    | Action::Task { .. }
            )
        }) {
            return Err(
                "Omarsket-only actions cannot be exported to Windows Tasket; use Omarsket JSON"
                    .into(),
            );
        }
        let actions: Result<Vec<_>> = self.actions.iter().map(|a| a.resolve(data)).collect();
        Ok(
            json!({"docType":"ScheduleTask File", "description":self.description, "actions":actions?}),
        )
    }
    pub fn rescale(
        &mut self,
        old: [u32; 2],
        new: [u32; 2],
        data: &BTreeMap<String, Snippet>,
    ) -> Result<()> {
        if old.contains(&0) || new.contains(&0) {
            return Err("Screen dimensions must be positive".into());
        }
        let scale = |v: i32, axis: usize| -> Result<i32> {
            let n = (v as f64 * new[axis] as f64 / old[axis] as f64).round();
            if n < i32::MIN as f64 || n > i32::MAX as f64 {
                Err("Scaled coordinate overflow".into())
            } else {
                Ok(n as i32)
            }
        };
        let mut updated = self.actions.clone();
        for action in &mut updated {
            *action = action.resolve(data)?;
            match action {
                Action::Cursor {
                    movements, data_id, ..
                } => {
                    for m in movements {
                        m[2] = scale(m[2], 0)?;
                        m[3] = scale(m[3], 1)?;
                    }
                    data_id.clear();
                }
                Action::Click { x, y, .. } | Action::Move { x, y, .. } => {
                    *x = scale(*x, 0)?;
                    *y = scale(*y, 1)?;
                }
                _ => (),
            }
        }
        self.actions = updated;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snippet {
    pub kind: String,
    pub value: Value,
}
impl Snippet {
    pub fn validate(&self) -> Result<()> {
        let mut value = json!({"type":self.kind});
        let field = match self.kind.as_str() {
            "paste" => "content",
            "keyssequence" => "keysmap",
            "cursormovements" => "cursormovsmap",
            _ => return Err("Data kind must be paste, keyssequence or cursormovements".into()),
        };
        value[field] = self.value.clone();
        serde_json::from_value::<Action>(value)
            .map_err(|e| e.to_string())?
            .validate()
    }
}
pub fn valid_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 120
        || id == "."
        || id == ".."
        || id
            .chars()
            .any(|c| c.is_control() || "/\\:\"<>|?*".contains(c))
    {
        Err("Use a name of 1–120 characters without path separators or control characters".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_all_native_actions() {
        let task = Task::import("native", json!({"docType":"ScheduleTask File","actions":[
            {"type":"paste","content":"hello"},{"type":"wait","duration":0.1},
            {"type":"keyssequence","keysmap":{"0":[100,["CONTROL_LEFT","C"]]}},
            {"type":"cursormovements","cursormovsmap":[[0,100,200,300]]},
            {"type":"systemcommand","sysCommandType":"shutdown","sysCommandParam1":"Don't force"},
            {"type":"runningothertask","otherTaskName":"other","otherTaskLoops":1}
        ]})).unwrap();
        assert_eq!(task.actions.len(), 6);
        assert!(!task.allow_system);
        assert_eq!(
            task.tasket_json(&BTreeMap::new()).unwrap()["actions"][4]["sysCommandParam1"],
            "Don't force"
        );
    }
    #[test]
    fn rescale_detaches_data_and_keeps_timings() {
        let mut task = Task::import("path", json!({"docType":"ScheduleTask File","actions":[{"type":"cursormovements","cursorMovsId":"walk","cursormovsmap":[[5,20,-100,80]]}]})).unwrap();
        task.rescale([100, 100], [200, 150], &BTreeMap::new())
            .unwrap();
        let value = task.tasket_json(&BTreeMap::new()).unwrap();
        assert_eq!(
            value["actions"][0]["cursormovsmap"][0],
            json!([5, 20, -200, 120])
        );
        assert_eq!(value["actions"][0]["cursorMovsId"], "");
        assert!(task.rescale([0, 100], [1, 1], &BTreeMap::new()).is_err());
    }
    #[test]
    fn rejects_bad_names_and_timing() {
        assert!(valid_id("../bad").is_err());
        assert!(Action::Wait { duration: f64::NAN }.validate().is_err());
        assert!(loops(0).is_err());
        assert!(serde_json::from_value::<Action>(json!({"type":"unknown"})).is_err());
    }
}
