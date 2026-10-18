//! Explicit, read-only discovery through already-running native vendor providers.
use crate::model::Result;
use serde_json::{Value, json};
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn query(program: &str, args: &[&str], budget: Instant) -> Result<Value> {
    if Instant::now() >= budget {
        return Err("Native discovery time limit reached".into());
    }
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Native provider is unavailable")?;
    let stdout = child.stdout.take().ok_or("No native provider output")?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(256 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = (Instant::now() + Duration::from_secs(2)).min(budget);
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "Native provider output stopped")?
        .map_err(|_| "Cannot read native provider")?;
    if !success || bytes.len() > 256 * 1024 {
        return Err("Native provider is unavailable or exceeded the read limit".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid native provider JSON".into())
}
fn bus(
    user: bool,
    dest: &str,
    path: &str,
    interface: &str,
    method: &str,
    arguments: &[&str],
    budget: Instant,
) -> Result<Value> {
    let mut args = vec![
        if user { "--user" } else { "--system" },
        "--auto-start=no",
        "--allow-interactive-authorization=no",
        "--timeout=1",
        "--json=short",
        "call",
        dest,
        path,
        interface,
        method,
    ];
    args.extend_from_slice(arguments);
    query("busctl", &args, budget)
}
fn strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => {
            for v in a {
                strings(v, out)
            }
        }
        Value::Object(o) => {
            for (k, v) in o {
                if k != "type" {
                    strings(v, out)
                }
            }
        }
        _ => (),
    }
}
fn paths(value: &Value, prefix: &str) -> Vec<String> {
    let mut values = Vec::new();
    strings(value, &mut values);
    values.retain(|s| {
        s.starts_with(prefix)
            && s.len() < 256
            && s.bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'/' || c == b'_')
    });
    values.sort();
    values.dedup();
    values
}
fn owner(user: bool, dest: &str, budget: Instant) -> bool {
    bus(
        user,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        &["s", dest],
        budget,
    )
    .is_ok_and(|v| v["data"][0] == true)
}
fn properties(dest: &str, path: &str, interface: &str, budget: Instant) -> Result<Value> {
    bus(
        false,
        dest,
        path,
        "org.freedesktop.DBus.Properties",
        "GetAll",
        &["s", interface],
        budget,
    )
}
fn razer(budget: Instant) -> Value {
    if !owner(true, "org.razer", budget) {
        return json!({"available":false,"provider":"OpenRazer","devices":[]});
    }
    let result = (|| -> Result<Value> {
        let list = bus(
            true,
            "org.razer",
            "/org/razer",
            "razer.devices",
            "getDevices",
            &[],
            budget,
        )?;
        let mut serials = Vec::new();
        strings(&list, &mut serials);
        serials.retain(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        });
        serials.sort();
        serials.dedup();
        let mut devices = Vec::new();
        for serial in serials.iter().take(8) {
            let path = format!("/org/razer/device/{serial}");
            let name = bus(
                true,
                "org.razer",
                &path,
                "razer.device.misc",
                "getDeviceName",
                &[],
                budget,
            )?;
            let interfaces = bus(
                true,
                "org.razer",
                &path,
                "org.freedesktop.DBus.Introspectable",
                "Introspect",
                &[],
                budget,
            )?;
            devices.push(json!({"serial":serial,"name":name,"interfaces":interfaces}));
        }
        Ok(
            json!({"available":true,"provider":"OpenRazer","devices":devices,"truncated":serials.len()>8,"onboardWrite":false}),
        )
    })();
    result.unwrap_or_else(
        |e| json!({"available":false,"provider":"OpenRazer","error":e,"devices":[]}),
    )
}
fn ratbag(vendors: &[String], budget: Instant) -> Value {
    const DEST: &str = "org.freedesktop.ratbag1";
    if !owner(false, DEST, budget) {
        return json!({"available":false,"provider":"ratbagd","devices":[]});
    }
    let result = (|| -> Result<Value> {
        let list = bus(
            false,
            DEST,
            "/org/freedesktop/ratbag1",
            "org.freedesktop.DBus.Properties",
            "Get",
            &["ss", "org.freedesktop.ratbag1.Manager", "Devices"],
            budget,
        )?;
        let device_paths = paths(&list, "/org/freedesktop/ratbag1/device/");
        let mut devices = Vec::new();
        for path in device_paths.iter().take(8) {
            let data = properties(DEST, path, "org.freedesktop.ratbag1.Device", budget)?;
            let mut values = Vec::new();
            strings(&data, &mut values);
            if !vendors.iter().any(|v| {
                values.iter().any(|s| {
                    s.to_lowercase()
                        .contains(if v == "logitech" { ":046d:" } else { ":1038:" })
                })
            }) {
                continue;
            }
            let mut profiles = Vec::new();
            for profile in paths(&data, "/org/freedesktop/ratbag1/profile/")
                .iter()
                .take(8)
            {
                profiles.push(json!({"path":profile,"state":properties(DEST,profile,"org.freedesktop.ratbag1.Profile",budget)?}));
            }
            devices.push(json!({"path":path,"state":data,"profiles":profiles}));
        }
        Ok(
            json!({"available":true,"provider":"ratbagd","devices":devices,"truncated":device_paths.len()>8,"onboardWrite":false}),
        )
    })();
    result
        .unwrap_or_else(|e| json!({"available":false,"provider":"ratbagd","error":e,"devices":[]}))
}
pub fn infer(request: &Value) -> Result<Value> {
    let budget = Instant::now() + Duration::from_secs(8);
    let vendors = request["vendors"]
        .as_array()
        .ok_or("Choose a hardware provider")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("Invalid hardware provider")
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if vendors.is_empty()
        || vendors.len() > 3
        || vendors
            .iter()
            .any(|s| !["razer", "steelseries", "logitech"].contains(&s.as_str()))
    {
        return Err("Choose Razer, SteelSeries or Logitech".into());
    }
    let mut providers = Vec::new();
    if vendors.iter().any(|s| s == "razer") {
        providers.push(razer(budget));
    }
    if vendors
        .iter()
        .any(|s| s == "steelseries" || s == "logitech")
    {
        providers.push(ratbag(&vendors, budget));
    }
    if vendors.iter().any(|s| s == "steelseries") {
        let native = query(
            "omarchy-shell",
            &["-q", "steelseries.keyboard", "status"],
            budget,
        );
        providers.push(match native{Ok(v)=>json!({"available":true,"provider":"OmasteelRGB","savedState":v,"onboardRead":false,"onboardWrite":false}),Err(_)=>json!({"available":false,"provider":"OmasteelRGB"})});
    }
    Ok(json!({"providers":providers,"readOnly":true}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_payload_paths_are_bounded_and_not_shell_arguments() {
        let value = json!({"type":"v","data":[{"type":"ao","data":["/org/freedesktop/ratbag1/device/hidraw0","/org/freedesktop/ratbag1/device/hidraw0","/org/freedesktop/ratbag1/device/evil;sh","/etc/shadow"]}]});
        assert_eq!(
            paths(&value, "/org/freedesktop/ratbag1/device/"),
            vec!["/org/freedesktop/ratbag1/device/hidraw0"]
        );
        assert!(infer(&json!({"vendors":["arbitrary command"]})).is_err());
        assert!(infer(&json!({"vendors":[]})).is_err());
    }
}
