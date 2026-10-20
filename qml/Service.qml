import QtQuick
import Quickshell
import Quickshell.Io
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
  property bool heatmapChecked: false
  property bool heatmapConnected: false
  property bool heatmapVisible: false
  property bool heatmapAvailable: false
  property string heatmapError: ""
  property var heatmapState: ({version:1,active:null,sessions:[]})
  property var heatmapQueue: []
  property var nativeBindings: []
  property var nativeKeymap: ({})
  property var nativeRouting: null
  function inputConfigCommand(request,callback){request.method="input-config";call(request,function(v,e){if(!e&&v.bindings){root.nativeBindings=v.bindings;root.nativeKeymap=v.keymap||{};root.nativeRouting=v.routing||null}if(callback)callback(v,e)})}
  readonly property string macrohelp: (Quickshell.env("XDG_DATA_HOME")||Quickshell.env("HOME")+"/.local/share")+"/macrohelp/overlay/bin/macrohelpctl"
  signal stopping()
  function openHeatmap() {
    if(heatmapChecked)return
    heatmapChecked=true
    health.command=[macrohelp,"health"].concat(settings&&settings.macrohelpEnvFile?["--env",String(settings.macrohelpEnvFile)]:[])
    health.running=true
  }
  function heatmapCommand(request) {
    heatmapQueue.push(JSON.stringify(request))
    nextHeatmap()
  }
  function nextHeatmap() {
    if(bridge.running||!heatmapConnected||!heatmapQueue.length)return
    bridge.command=["omarchy-shell","-q","io.github.commanderturtle.macrohelp","heatmap",heatmapQueue.shift()]
    bridge.running=true
  }
  Timer {id:heatmapRefresh;interval:1000;repeat:true;running:root.heatmapConnected&&root.heatmapVisible;onTriggered:{if(!bridge.running&&!root.heatmapQueue.length)root.heatmapCommand({action:"snapshot"})}}
  Process {
    id:health
    stdout:StdioCollector {id:healthOutput;waitForEnd:true}
    onExited:function(code){
      var ok=false;try{ok=code===0&&!!JSON.parse(String(healthOutput.text))}catch(e){}
      root.heatmapConnected=ok
      root.heatmapError=ok?"":"failed to connect to macrohelp"
      if(ok)root.heatmapCommand({action:"snapshot"})
    }
  }
  Process {
    id:bridge
    stdout:StdioCollector {id:bridgeOutput;waitForEnd:true}
    onExited:function(code){
      try{
        if(code!==0)throw Error("failed to connect to macrohelp")
        var reply=JSON.parse(String(bridgeOutput.text))
        if(reply.state){root.heatmapState=reply.state;root.heatmapAvailable=!!reply.available;root.heatmapError=reply.error||""}
        else if(reply.error)root.heatmapError=reply.error
      }catch(e){root.heatmapError="failed to connect to macrohelp"}
      root.nextHeatmap()
    }
  }
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
        if (reply.event === "ready") { root.ready = true; root.apply(reply.snapshot); return }
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
      root.operationEpoch++;root.stopping()
      var callbacks = root.callbacks; root.callbacks = ({})
      for (var id in callbacks) callbacks[id](null,"Runtime stopped")
      if (code !== 0 && root.error === "") root.error = "Runtime exited (" + code + "). Rebuild or start it again."
    }
  }
}
