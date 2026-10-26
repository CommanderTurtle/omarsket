import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import qs.Commons

Item {
  id: root
  property var shell: null
  property var manifest: null
  property var settings: ({})
  property bool ready: false
  property var tasks: []
  property var data: ({})
  property var jobs: []
  property string directory: ""
  property var notices: []
  property var preferences: ({})
  property string error: ""
  property int serial: 0
  property int operationEpoch: 0
  property var callbacks: ({})
  property var nativeBindings: []
  property var nativeKeymap: ({})
  property var nativeRouting: null
  property bool nativeRefreshing: false
  property bool nativeRefreshPending: false
  property int nativeEpoch: 0
  property int nativeRevision: 0
  property string nativeError: ""
  property var nativeLayouts: ({})
  property bool nativeKeymapFresh: true
  function ensureNative(){if(nativeRevision===0&&!nativeRefreshing)refreshNative(false)}
  function inputConfigCommand(request,callback){request.method="input-config";call(request,function(v,e){if(!e&&request.action==="save")root.refreshNative();if(callback)callback(v,e)})}
  function refreshNative(fresh){
    if(!ready)return
    if(fresh!==false)nativeKeymapFresh=true
    if(nativeRefreshing){nativeRefreshPending=true;return}
    nativeRefreshing=true;nativeRefreshPending=false
    var token=nativeEpoch,force=nativeKeymapFresh;nativeKeymapFresh=false
    call({method:"input-config",action:"describe",fresh:force},function(v,e){
      if(token!==root.nativeEpoch)return
      root.nativeRefreshing=false
      var again=root.nativeRefreshPending;root.nativeRefreshPending=false
      var failure=e||(!v||!Array.isArray(v.bindings)?"Invalid native keyboard metadata":"")
      root.nativeError=failure||((v&&v.keymap&&v.keymap.error)||"")
      if(!failure){
        var changed=false,keymap=v.keymap||{},routing=v.routing||null
        if(JSON.stringify(root.nativeBindings)!==JSON.stringify(v.bindings)){root.nativeBindings=v.bindings;changed=true}
        if(JSON.stringify(root.nativeKeymap)!==JSON.stringify(keymap)){root.nativeKeymap=keymap;changed=true}
        if(JSON.stringify(root.nativeRouting)!==JSON.stringify(routing)){root.nativeRouting=routing;changed=true}
        if(changed||root.nativeRevision===0)root.nativeRevision++
      }
      if(again)nativeRefreshDelay.restart()
    })
  }
  function nativeEvent(name,data){
    if(name==="configreloaded"){nativeLayouts=({});nativeKeymapFresh=true;nativeRefreshDelay.restart();return}
    if(name!=="activelayout"||typeof data!=="string")return
    var at=data.indexOf(",");if(at<1)return
    var keyboard="$"+data.slice(0,at),layout=data.slice(at+1)
    if(nativeLayouts[keyboard]===layout)return
    // Focus changes can repeat a device's current layout. Only a new value
    // invalidates legends; pointer/window events never query native metadata.
    if(Object.keys(nativeLayouts).length>=32)nativeLayouts=({})
    nativeLayouts[keyboard]=layout;nativeRefreshDelay.restart()
  }
  Timer {id:nativeRefreshDelay;interval:150;onTriggered:root.refreshNative(false)}
  Connections {target:Hyprland;function onRawEvent(event){root.nativeEvent(event.name,event.data)}}
  signal stopping()
  signal hidePanel()
  property var shortcutQueue:[]
  function runSchedule(name){
    if(!ready){error="Omarsket runtime is not ready";return}
    name=String(name).replace(/\.omscht$/i,"")
    if(!tasks.some(function(t){return t.id===name})){error="Schedule not found: "+name;return}
    if(shortcutQueue.length>=16){error="Schedule shortcut queue is full";return}
    shortcutQueue.push({name:name,epoch:operationEpoch});hidePanel();shortcutDelay.restart()
  }
  Timer {id:shortcutDelay;interval:300;onTriggered:{var queued=root.shortcutQueue;root.shortcutQueue=[];queued.forEach(function(item){if(root.ready&&item.epoch===root.operationEpoch)root.change({method:"run",name:item.name})})}}
  Connections {target:root;function onStopping(){shortcutDelay.stop();root.shortcutQueue=[]}}
  readonly property string helper: (Quickshell.env("XDG_DATA_HOME") || (Quickshell.env("HOME") + "/.local/share")) + "/omarsket/runtime/bin/omarsket"
  readonly property string controlSocket: settings && settings.controlSocket === false ? "" : String((settings && settings.controlSocket) || (Quickshell.env("XDG_RUNTIME_DIR") ? Quickshell.env("XDG_RUNTIME_DIR") + "/omarsket/control.sock" : ""))
  readonly property var runtimeCommand: [helper, "session"].concat(settings && settings.directory ? ["--directory",String(settings.directory)] : []).concat(controlSocket ? ["--control-socket",controlSocket] : [])
  readonly property int activeCount: jobs.filter(function(j) { return j.state === "running" || j.state === "scheduled" }).length
  function apply(snapshot) {
    tasks = snapshot.tasks || []; data = snapshot.data || ({}); jobs = snapshot.jobs || []
    directory = snapshot.directory || ""; notices = snapshot.notices || []; preferences = snapshot.preferences || ({})
  }
  function refresh() { call({method: "snapshot"}, function(s, error) { if(!error)root.apply(s) }) }
  function call(request, callback) {
    if (!ready) { error = "Run bash build-plugin.sh in the plugin checkout, then enable Omarsket."; if(callback)callback(null,error); return }
    var id = ++serial
    request.id = id
    if (callback) callbacks[id] = callback
    runtime.write(JSON.stringify(request) + "\n")
  }
  function change(request, callback) {
    call(request, function(result, failure) {
      if (!failure) root.refresh()
      if (callback) callback(result, failure)
    })
  }
  function stop() { operationEpoch++;stopping();call({method: "stop"}) }
  function restart() { if (!runtime.running) { error = ""; runtime.running = true } }
  property var pickerCallback: null
  function chooseFile(save, callback, name, folder) {
    if (picker.running) return
    pickerCallback = callback
    var command = ["zenity", "--file-selection", "--title=" + (save ? "Export" : "Import")]
    if (save) command.push("--save")
    if (folder) command.push("--directory")
    else if (!save) command.push("--file-filter=Schedules and JSON | *.omscht *.scht *.json", "--file-filter=All files | *")
    if (name) command.push("--filename=" + name)
    picker.command = command
    picker.running = true
  }
  Process {
    id: picker
    stdout: StdioCollector { id: picked; waitForEnd: true }
    onExited: function(code) {
      var callback = root.pickerCallback; root.pickerCallback = null
      if (code === 0 && callback) callback(String(picked.text).trim())
    }
  }
  Process {
    id: runtime
    command: root.runtimeCommand
    stdinEnabled: true
    running: true
    stdout: SplitParser {
      onRead: function(line) {
        var reply
        try { reply = JSON.parse(line) } catch (e) { root.error = "Invalid runtime response"; return }
        if (reply.event === "stopping") { root.operationEpoch++;root.stopping();return }
        if (reply.event === "ready") { root.ready = true; root.apply(reply.snapshot); root.refreshNative(); return }
        if (reply.event === "store") { root.apply(reply.snapshot); return }
        if (reply.event === "jobs") { root.jobs = reply.jobs; return }
        if (reply.event === "error") { root.error = reply.error; return }
        var callback = root.callbacks[reply.id]
        delete root.callbacks[reply.id]
        if (!reply.ok) { root.error = reply.error; if(callback)callback(null,reply.error); return }
        root.error = ""
        if (callback) callback(reply.result)
      }
    }
    stderr: SplitParser { onRead: function(line) { root.error = line } }
    onExited: function(code) {
      root.ready = false
      root.nativeEpoch++;root.nativeRefreshing=false;root.nativeRefreshPending=false;root.nativeLayouts=({});root.nativeKeymapFresh=true;nativeRefreshDelay.stop()
      root.nativeError="Runtime stopped; hotkey map is stale"
      root.operationEpoch++;root.stopping()
      var callbacks = root.callbacks; root.callbacks = ({})
      for (var id in callbacks) callbacks[id](null,"Runtime stopped")
      if (code !== 0 && root.error === "") root.error = "Runtime exited (" + code + "). Rebuild or start it again."
    }
  }
}
