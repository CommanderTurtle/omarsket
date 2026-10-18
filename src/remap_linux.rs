//! A narrow, opt-in libevdev/uinput worker. No commands or Python run as root.
use super::{DeviceMap, HELPER, Keys, Profile};
use crate::model::Result;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    ffi::{CStr, CString},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    ptr,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
type Dev = libc::c_void;
type Ui = libc::c_void;
static STOP: AtomicBool = AtomicBool::new(false);
static CLIENT: Mutex<Option<UnixStream>> = Mutex::new(None);
static NONCE: Mutex<Option<String>> = Mutex::new(None);
static ROUTING: Mutex<Option<Value>> = Mutex::new(None);
extern "C" fn stop(_: i32) {
    STOP.store(true, Ordering::Relaxed);
}

struct Api {
    handle: *mut libc::c_void,
    new: unsafe extern "C" fn(i32, *mut *mut Dev) -> i32,
    free: unsafe extern "C" fn(*mut Dev),
    name: unsafe extern "C" fn(*const Dev) -> *const libc::c_char,
    set_name: unsafe extern "C" fn(*mut Dev, *const libc::c_char),
    vendor: unsafe extern "C" fn(*const Dev) -> i32,
    product: unsafe extern "C" fn(*const Dev) -> i32,
    has_type: unsafe extern "C" fn(*const Dev, u32) -> i32,
    has_code: unsafe extern "C" fn(*const Dev, u32, u32) -> i32,
    value: unsafe extern "C" fn(*const Dev, u32, u32) -> i32,
    enable: unsafe extern "C" fn(*mut Dev, u32, u32, *const libc::c_void) -> i32,
    disable: unsafe extern "C" fn(*mut Dev, u32) -> i32,
    grab: unsafe extern "C" fn(*mut Dev, i32) -> i32,
    next: unsafe extern "C" fn(*mut Dev, u32, *mut libc::input_event) -> i32,
    create: unsafe extern "C" fn(*const Dev, i32, *mut *mut Ui) -> i32,
    destroy: unsafe extern "C" fn(*mut Ui),
    ui_fd: unsafe extern "C" fn(*const Ui) -> i32,
    write: unsafe extern "C" fn(*const Ui, u32, u32, i32) -> i32,
}
impl Api {
    fn load() -> Result<Self> {
        // Absolute native soname: no user-selected shared library in an elevated process.
        let handle = unsafe {
            libc::dlopen(
                c"/usr/lib/libevdev.so.2".as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL,
            )
        };
        if handle.is_null() {
            return Err("Native libevdev is unavailable".into());
        }
        macro_rules! sym {
            ($s:literal,$t:ty) => {{
                let p = unsafe { libc::dlsym(handle, concat!($s, "\0").as_ptr().cast()) };
                if p.is_null() {
                    unsafe { libc::dlclose(handle) };
                    return Err("Native libevdev lacks a required interface".into());
                }
                unsafe { std::mem::transmute::<*mut libc::c_void, $t>(p) }
            }};
        }
        Ok(Self {
            handle,
            new: sym!(
                "libevdev_new_from_fd",
                unsafe extern "C" fn(i32, *mut *mut Dev) -> i32
            ),
            free: sym!("libevdev_free", unsafe extern "C" fn(*mut Dev)),
            name: sym!(
                "libevdev_get_name",
                unsafe extern "C" fn(*const Dev) -> *const libc::c_char
            ),
            set_name: sym!(
                "libevdev_set_name",
                unsafe extern "C" fn(*mut Dev, *const libc::c_char)
            ),
            vendor: sym!(
                "libevdev_get_id_vendor",
                unsafe extern "C" fn(*const Dev) -> i32
            ),
            product: sym!(
                "libevdev_get_id_product",
                unsafe extern "C" fn(*const Dev) -> i32
            ),
            has_type: sym!(
                "libevdev_has_event_type",
                unsafe extern "C" fn(*const Dev, u32) -> i32
            ),
            has_code: sym!(
                "libevdev_has_event_code",
                unsafe extern "C" fn(*const Dev, u32, u32) -> i32
            ),
            value: sym!(
                "libevdev_get_event_value",
                unsafe extern "C" fn(*const Dev, u32, u32) -> i32
            ),
            enable: sym!(
                "libevdev_enable_event_code",
                unsafe extern "C" fn(*mut Dev, u32, u32, *const libc::c_void) -> i32
            ),
            disable: sym!(
                "libevdev_disable_event_type",
                unsafe extern "C" fn(*mut Dev, u32) -> i32
            ),
            grab: sym!("libevdev_grab", unsafe extern "C" fn(*mut Dev, i32) -> i32),
            next: sym!(
                "libevdev_next_event",
                unsafe extern "C" fn(*mut Dev, u32, *mut libc::input_event) -> i32
            ),
            create: sym!(
                "libevdev_uinput_create_from_device",
                unsafe extern "C" fn(*const Dev, i32, *mut *mut Ui) -> i32
            ),
            destroy: sym!("libevdev_uinput_destroy", unsafe extern "C" fn(*mut Ui)),
            ui_fd: sym!(
                "libevdev_uinput_get_fd",
                unsafe extern "C" fn(*const Ui) -> i32
            ),
            write: sym!(
                "libevdev_uinput_write_event",
                unsafe extern "C" fn(*const Ui, u32, u32, i32) -> i32
            ),
        })
    }
}
impl Drop for Api {
    fn drop(&mut self) {
        unsafe { libc::dlclose(self.handle) };
    }
}
fn check(code: i32) -> Result<()> {
    if code < 0 {
        Err(format!(
            "Native input operation failed: {}",
            std::io::Error::from_raw_os_error(-code)
        ))
    } else {
        Ok(())
    }
}
fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
fn runtime(who: u32) -> Result<PathBuf> {
    let path = PathBuf::from(format!("/run/user/{who}"));
    let m = fs::symlink_metadata(&path).map_err(|_| "No active user runtime directory")?;
    if !m.is_dir() || m.uid() != who || m.mode() & 0o077 != 0 {
        return Err("User runtime directory is not private".into());
    }
    Ok(path.join("omarsket-input"))
}
pub fn trusted_helper() -> bool {
    ["/usr", "/usr/local", "/usr/local/libexec", HELPER]
        .iter()
        .all(|p| {
            fs::symlink_metadata(p)
                .is_ok_and(|m| m.uid() == 0 && m.mode() & 0o022 == 0 && !m.file_type().is_symlink())
        })
        && fs::metadata(HELPER)
            .is_ok_and(|m| m.is_file() && m.mode() & 0o111 != 0 && m.mode() & 0o6000 == 0)
}
fn physical(path: &Path) -> Result<PathBuf> {
    let p = fs::canonicalize(path).map_err(|_| "Selected input device is disconnected")?;
    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid event device")?;
    if p.parent() != Some(Path::new("/dev/input"))
        || !name
            .strip_prefix("event")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err("Select a physical event device".into());
    }
    let sys = fs::canonicalize(Path::new("/sys/class/input").join(name).join("device"))
        .map_err(|_| "No input device identity")?;
    if sys.starts_with("/sys/devices/virtual/input") {
        return Err("Virtual input devices cannot be routed again".into());
    }
    Ok(p)
}
pub fn devices() -> Value {
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    for dir in ["/dev/input/by-id", "/dev/input/by-path"] {
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                let path = e.path();
                let Ok(real) = physical(&path) else { continue };
                if !seen.insert(real.clone()) {
                    continue;
                }
                let sys = Path::new("/sys/class/input")
                    .join(real.file_name().unwrap())
                    .join("device");
                let hex = |s: &str| {
                    fs::read_to_string(sys.join(s))
                        .ok()
                        .and_then(|v| u16::from_str_radix(v.trim(), 16).ok())
                        .unwrap_or(0)
                };
                items.push(json!({"path":path,"name":fs::read_to_string(sys.join("name")).unwrap_or_default().trim(),"vendor":hex("id/vendor"),"product":hex("id/product")}));
            }
        }
    }
    items.sort_by_key(|v| v["path"].as_str().unwrap_or("").to_owned());
    json!(items)
}
fn line(stream: &mut UnixStream) -> Result<Value> {
    // One request is outstanding at a time, so no later reply can be buffered here.
    let mut bytes = Vec::new();
    BufReader::new(stream)
        .take(256 * 1024 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 256 * 1024 || bytes.last() != Some(&b'\n') {
        return Err("Incomplete or oversized input-router reply".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid input-router reply".into())
}
pub fn client(action: &str) -> Result<Value> {
    let mut guard = CLIENT.lock().map_err(|_| "Input router closed")?;
    if action == "stop" && guard.is_none() {
        let nonce = NONCE.lock().map_err(|_| "Input router closed")?.take();
        if let Some(nonce) = nonce
            && let Ok(mut stream) = UnixStream::connect(runtime(uid())?.join("control.sock"))
        {
            stream
                .set_write_timeout(Some(Duration::from_millis(100)))
                .map_err(|e| e.to_string())?;
            let _ = stream.write_all(format!("{}\n", json!({"cancel":nonce})).as_bytes());
        }
        return Ok(json!({"stopped":true}));
    }
    if guard.is_none() {
        let nonce = NONCE
            .lock()
            .map_err(|_| "Input router closed")?
            .clone()
            .ok_or("Input router is off")?;
        let mut stream = UnixStream::connect(runtime(uid())?.join("control.sock"))
            .map_err(|_| "Input router is off")?;
        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(format!("{}\n", json!({"attach":nonce})).as_bytes())
            .map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        let attached = line(&mut stream)?;
        if attached["active"] != true {
            return Err("Input router did not activate".into());
        }
        *ROUTING.lock().map_err(|_| "Input router closed")? = Some(attached["routing"].clone());
        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| e.to_string())?;
        *guard = Some(stream);
    }
    let result = (|| {
        let stream = guard.as_mut().unwrap();
        stream
            .write_all(format!("{action}\n").as_bytes())
            .map_err(|e| e.to_string())?;
        line(stream)
    })();
    if result.is_err() || action == "stop" {
        *guard = None;
        *ROUTING.lock().map_err(|_| "Input router closed")? = None;
    }
    if action == "stop" {
        *NONCE.lock().map_err(|_| "Input router closed")? = None;
    }
    result
}
pub fn active_routing() -> Value {
    if client("status").is_err() {
        return Value::Null;
    }
    ROUTING
        .lock()
        .ok()
        .and_then(|v| v.clone())
        .unwrap_or(Value::Null)
}
pub fn activation_error() -> Option<String> {
    let nonce = NONCE.lock().ok()?.clone()?;
    let path = runtime(uid()).ok()?.join(format!("error-{nonce}.json"));
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .ok()?;
    let m = file.metadata().ok()?;
    if !m.is_file() || m.uid() != uid() || m.len() > 4096 {
        return None;
    }
    let value: Value = serde_json::from_reader(file).ok()?;
    value["error"].as_str().map(str::to_owned)
}
pub fn start(profile: &Path) -> Result<()> {
    if !trusted_helper() {
        return Err(
            "Initialize the optional root-owned input helper first (quick-start.md)".into(),
        );
    }
    Api::load()?;
    if client("status").is_ok() {
        return Err("Stop the active input profile first".into());
    }
    let mut random = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut random))
        .map_err(|e| e.to_string())?;
    let nonce = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Command::new("omarchy-launch-terminal")
        .args(["--", "sudo", "--", HELPER, "--profile"])
        .arg(fs::canonicalize(profile).map_err(|e| e.to_string())?)
        .arg("--owner")
        .arg(&nonce)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    *NONCE.lock().map_err(|_| "Input router closed")? = Some(nonce);
    Ok(())
}

struct Device<'a> {
    api: &'a Api,
    fd: File,
    dev: *mut Dev,
    ui: *mut Ui,
    map: DeviceMap,
    keys: Keys,
    sync: bool,
    grabbed: bool,
    pending: bool,
}
impl<'a> Device<'a> {
    fn prepare(api: &'a Api, map: DeviceMap) -> Result<Self> {
        let real = physical(Path::new(&map.path))?;
        let fd = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(real)
            .map_err(|_| "Cannot open selected input device")?;
        let meta = fd.metadata().map_err(|e| e.to_string())?;
        if !meta.file_type().is_char_device() || libc::major(meta.rdev()) != 13 {
            return Err("Not a kernel input device".into());
        }
        let mut d = Self {
            api,
            fd,
            dev: ptr::null_mut(),
            ui: ptr::null_mut(),
            map,
            keys: Keys::default(),
            sync: false,
            grabbed: false,
            pending: false,
        };
        check(unsafe { (api.new)(d.fd.as_raw_fd(), &mut d.dev) })?;
        if unsafe { (api.vendor)(d.dev) } != i32::from(d.map.vendor)
            || unsafe { (api.product)(d.dev) } != i32::from(d.map.product)
        {
            return Err("Device identity changed; select it again".into());
        }
        if unsafe { (api.has_type)(d.dev, 1) } == 0
            || [3, 21]
                .iter()
                .any(|t| unsafe { (api.has_type)(d.dev, *t) } != 0)
        {
            return Err("This router supports keyboards and relative mice, not touch/absolute/force-feedback devices".into());
        }
        for source in d.map.keys.keys() {
            if unsafe { (api.has_code)(d.dev, 1, u32::from(*source)) } == 0 {
                return Err(format!("Physical device does not emit key code {source}"));
            }
        }
        for key in d.map.keys.values().flatten() {
            check(unsafe { (api.enable)(d.dev, 1, u32::from(*key), ptr::null()) })?;
        }
        // Forward physical repeat notifications; don't create a second kernel repeat source.
        check(unsafe { (api.disable)(d.dev, 20) })?;
        let raw = unsafe { (api.name)(d.dev) };
        let name = if raw.is_null() {
            "input".into()
        } else {
            unsafe { CStr::from_ptr(raw) }
                .to_string_lossy()
                .into_owned()
        };
        let name = CString::new(format!("Omarsket {name}")).map_err(|_| "Invalid device name")?;
        unsafe { (api.set_name)(d.dev, name.as_ptr()) };
        check(unsafe { (api.create)(d.dev, -2, &mut d.ui) })?;
        let out = unsafe { (api.ui_fd)(d.ui) };
        let flags = unsafe { libc::fcntl(out, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(out, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err("Cannot initialize input output descriptor".into());
        }
        Ok(d)
    }
    fn emit(&self, kind: u32, code: u32, value: i32) -> Result<()> {
        check(unsafe { (self.api.write)(self.ui, kind, code, value) })
    }
    fn grab(&mut self) -> Result<()> {
        // Drain events before the grab; never swallow a key/button already held.
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            if STOP.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return Err("Release all keys/buttons before enabling input routing".into());
            }
            let mut e = unsafe { std::mem::zeroed::<libc::input_event>() };
            let r = unsafe { (self.api.next)(self.dev, if self.sync { 1 } else { 2 }, &mut e) };
            if r == 1 {
                self.sync = true;
                continue;
            }
            if r == -libc::EAGAIN {
                self.sync = false;
                if (1..768).all(|k| unsafe { (self.api.value)(self.dev, 1, k) } == 0) {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("Release all keys/buttons before enabling input routing".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            } else {
                check(r)?;
            }
        }
        check(unsafe { (self.api.grab)(self.dev, 3) })?;
        self.grabbed = true;
        Ok(())
    }
    fn forward(&mut self) -> Result<bool> {
        self.pending = false;
        // Bound one visit so a busy mouse cannot starve the stop/control descriptor.
        for _ in 0..256 {
            let mut e = unsafe { std::mem::zeroed::<libc::input_event>() };
            let r = unsafe { (self.api.next)(self.dev, if self.sync { 1 } else { 2 }, &mut e) };
            if r == -libc::EAGAIN {
                if self.sync {
                    self.sync = false;
                    continue;
                }
                return Ok(true);
            }
            check(r)?;
            if r == 1 && !self.sync {
                self.sync = true;
                continue;
            }
            if e.type_ == 1 {
                let events = self.keys.event(&self.map.keys, e.code, e.value);
                if self.keys.emergency() {
                    return Ok(false);
                }
                for (code, value) in events {
                    self.emit(1, u32::from(code), value)?;
                }
            } else if !(e.type_ == 4 && e.code == 4 || e.type_ == 20) {
                self.emit(u32::from(e.type_), u32::from(e.code), e.value)?;
            }
        }
        self.pending = true;
        Ok(true)
    }
    fn leds(&self) -> Result<()> {
        let out = unsafe { (self.api.ui_fd)(self.ui) };
        for _ in 0..32 {
            let mut e = unsafe { std::mem::zeroed::<libc::input_event>() };
            let n = unsafe {
                libc::read(
                    out,
                    (&mut e as *mut libc::input_event).cast(),
                    std::mem::size_of_val(&e),
                )
            };
            if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock {
                break;
            }
            if n <= 0 {
                return Err("Virtual input device disconnected".into());
            }
            if n as usize != std::mem::size_of_val(&e) {
                return Err("Incomplete virtual input event".into());
            }
            if e.type_ == 17 || e.type_ == 0 {
                let n = unsafe {
                    libc::write(
                        self.fd.as_raw_fd(),
                        (&e as *const libc::input_event).cast(),
                        std::mem::size_of_val(&e),
                    )
                };
                if n as usize != std::mem::size_of_val(&e) {
                    return Err("Cannot forward input indicator state".into());
                }
            }
        }
        Ok(())
    }
}
impl Drop for Device<'_> {
    fn drop(&mut self) {
        if !self.ui.is_null() {
            for (code, value) in self.keys.release() {
                let _ = self.emit(1, u32::from(code), value);
            }
            let _ = self.emit(0, 0, 0);
        }
        if self.grabbed {
            unsafe { (self.api.grab)(self.dev, 4) };
        }
        if !self.ui.is_null() {
            unsafe { (self.api.destroy)(self.ui) };
        }
        if !self.dev.is_null() {
            unsafe { (self.api.free)(self.dev) };
        }
    }
}
struct Socket {
    path: PathBuf,
    device: u64,
    inode: u64,
}
impl Drop for Socket {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.dev() == self.device && m.ino() == self.inode)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}
fn authorize_profile(path: &Path, who: u32) -> Result<Profile> {
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "Cannot open input profile")?;
    let m = f.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() || m.uid() != who || m.mode() & 0o077 != 0 || m.len() > 128 * 1024 {
        return Err(
            "Input profile must be a private user-owned regular file (at most 128 KiB)".into(),
        );
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut f)
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let profile: Profile = serde_json::from_slice(&bytes).map_err(|_| "Invalid input profile")?;
    profile.validate()?;
    Ok(profile)
}
/// A bounded activation failure, not an input-event log. Writes only after root
/// has been discarded; the file lives in the requesting user's runtime directory.
pub fn report_error(error: &str) {
    let _ = (|| -> Result<()> {
        if !trusted_helper() || std::env::current_exe().ok().as_deref() != Some(Path::new(HELPER)) {
            return Ok(());
        }
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        let Some(nonce) = args
            .get(3)
            .filter(|s| s.len() == 32 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        else {
            return Ok(());
        };
        let who = if uid() == 0 {
            std::env::var("SUDO_UID")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|n| *n != 0)
                .ok_or("No requesting user")?
        } else {
            uid()
        };
        let root = runtime(who)?;
        if uid() == 0 {
            let account = unsafe { libc::getpwuid(who) };
            if account.is_null() {
                return Ok(());
            }
            let group = unsafe { (*account).pw_gid };
            if unsafe { libc::setgroups(0, ptr::null()) } < 0
                || unsafe { libc::setresgid(group, group, group) } < 0
                || unsafe { libc::setresuid(who, who, who) } < 0
            {
                return Ok(());
            }
        }
        crate::store::private_dir(&root)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join(format!("error-{nonce}.json")))
            .map_err(|e| e.to_string())?;
        let message = error.chars().take(512).collect::<String>();
        file.write_all(json!({"error":message}).to_string().as_bytes())
            .map_err(|e| e.to_string())
    })();
}
fn peer_uid(stream: &UnixStream) -> Result<u32> {
    let mut cred = unsafe { std::mem::zeroed::<libc::ucred>() };
    let mut len = std::mem::size_of_val(&cred) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    } < 0
    {
        return Err("Cannot verify controller identity".into());
    }
    Ok(cred.uid)
}
pub fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--help"] {
        println!(
            "omarsket-input --profile FILE --owner TOKEN\nOptional authorized physical-input worker. Escape+Backspace+Enter stops routing."
        );
        return Ok(());
    }
    if args.len() != 4
        || args[0] != "--profile"
        || args[2] != "--owner"
        || args[3].len() != 32
        || !args[3].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err("Start input routing from Omarsket".into());
    }
    if uid() != 0
        || !trusted_helper()
        || std::env::current_exe().ok().as_deref() != Some(Path::new(HELPER))
    {
        return Err("Use the root-owned optional helper through sudo".into());
    }
    let who = std::env::var("SUDO_UID")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n != 0)
        .ok_or("Run from the logged-in user through sudo")?;
    let account = unsafe { libc::getpwuid(who) };
    if account.is_null() {
        return Err("Unknown requesting user".into());
    }
    let group = unsafe { (*account).pw_gid };
    let root = runtime(who)?;
    let profile = authorize_profile(Path::new(&args[1]), who)?;
    let api = Api::load()?;
    let mut devices = Vec::new();
    let mut seen = BTreeSet::new();
    for map in &profile.devices {
        let real = physical(Path::new(&map.path))?;
        if !seen.insert(real) {
            return Err("The same physical device was selected twice".into());
        }
        devices.push(Device::prepare(&api, map.clone())?);
    }
    // File descriptors, not broad group membership, carry the necessary access.
    // Drop saved IDs too, prohibit regaining privileges and prevent fd extraction.
    if unsafe { libc::setgroups(0, ptr::null()) } < 0
        || unsafe { libc::setresgid(group, group, group) } < 0
        || unsafe { libc::setresuid(who, who, who) } < 0
        || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } < 0
        || unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } < 0
    {
        return Err("Cannot discard input-worker privileges".into());
    }
    crate::store::private_dir(&root)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join("worker.lock"))
        .map_err(|e| e.to_string())?;
    let lock_meta = lock.metadata().map_err(|e| e.to_string())?;
    if !lock_meta.is_file() || lock_meta.uid() != who || lock_meta.mode() & 0o077 != 0 {
        return Err("Input worker lock must be a private regular file".into());
    }
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        return Err("Input routing is already active".into());
    }
    let path = root.join("control.sock");
    if let Ok(m) = fs::symlink_metadata(&path) {
        if !m.file_type().is_socket() || m.uid() != who {
            return Err("Refusing to replace an unrelated control path".into());
        }
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    let _socket = Socket {
        path,
        device: meta.dev(),
        inode: meta.ino(),
    };
    fs::set_permissions(&_socket.path, fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    unsafe {
        libc::signal(libc::SIGINT, stop as *const () as usize);
        libc::signal(libc::SIGTERM, stop as *const () as usize);
        libc::signal(libc::SIGHUP, stop as *const () as usize);
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut owner = loop {
        if STOP.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err("Input controller did not attach; devices left unchanged".into());
        }
        if let Ok((mut stream, _)) = listener.accept() {
            if peer_uid(&stream)? != who {
                continue;
            }
            stream
                .set_read_timeout(Some(Duration::from_millis(100)))
                .map_err(|e| e.to_string())?;
            if let Ok(value) = line(&mut stream) {
                if value["cancel"] == args[3] {
                    return Ok(());
                }
                if value["attach"] == args[3] {
                    break stream;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    for d in &mut devices {
        d.grab()?;
    }
    let status = json!({"profile":profile.name,"devices":devices.len(),"active":true});
    owner
        .write_all(format!("{}\n",json!({"profile":profile.name,"devices":devices.len(),"active":true,"routing":profile})).as_bytes())
        .map_err(|e| e.to_string())?;
    owner.set_nonblocking(true).map_err(|e| e.to_string())?;
    println!("Input routing active. Escape + Backspace + Enter or Ctrl+C stops it.");
    let mut input = Vec::new();
    let mut polls = vec![libc::pollfd {
        fd: owner.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];
    for d in &devices {
        polls.push(libc::pollfd {
            fd: d.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });
        polls.push(libc::pollfd {
            fd: unsafe { (api.ui_fd)(d.ui) },
            events: libc::POLLIN,
            revents: 0,
        });
    }
    while !STOP.load(Ordering::Relaxed) {
        let n = unsafe {
            libc::poll(
                polls.as_mut_ptr(),
                polls.len() as libc::nfds_t,
                if devices.iter().any(|d| d.pending) {
                    0
                } else {
                    100
                },
            )
        };
        if n < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err("Input poll failed".into());
        }
        if polls
            .iter()
            .any(|p| p.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0)
        {
            break;
        }
        if polls[0].revents & libc::POLLIN != 0 {
            let mut bytes = [0u8; 128];
            let n = owner.read(&mut bytes).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            input.extend_from_slice(&bytes[..n]);
            if input.len() > 512 {
                return Err("Invalid input control request".into());
            }
            while let Some(end) = input.iter().position(|b| *b == b'\n') {
                let command = input.drain(..=end).collect::<Vec<_>>();
                if command == b"stop\n" {
                    owner
                        .write_all(b"{\"stopped\":true}\n")
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
                if command != b"status\n" {
                    return Err("Unknown input control request".into());
                }
                owner
                    .write_all(format!("{status}\n").as_bytes())
                    .map_err(|e| e.to_string())?;
            }
        }
        for (i, d) in devices.iter_mut().enumerate() {
            if (d.pending || polls[1 + i * 2].revents & libc::POLLIN != 0) && !d.forward()? {
                return Ok(());
            }
            if polls[2 + i * 2].revents & libc::POLLIN != 0 {
                d.leds()?;
            }
        }
    }
    Ok(())
}
