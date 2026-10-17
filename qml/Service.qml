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
  property var callbacks: ({})
  readonly property string helper: decodeURIComponent(Qt.resolvedUrl("../bin/omarsket").toString().replace(/^file:\/\//, ""))
  readonly property int activeCount: jobs.filter(function(j) { return j.state === "running" || j.state === "scheduled" }).length
  function apply(snapshot) {
    tasks = snapshot.tasks || []; data = snapshot.data || ({}); jobs = snapshot.jobs || []
    directory = snapshot.directory || ""; notices = snapshot.notices || []; preferences = snapshot.preferences || ({})
  }
  function refresh() { call({method: "snapshot"}, function(s, error) { if(!error)root.apply(s) }) }
  function call(request, callback) {
    if (!ready) { error = "Build the Linux runtime, then start Omarsket."; if(callback)callback(null,error); return }
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
  function stop() { call({method: "stop"}) }
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
    command: root.settings && root.settings.directory ? [root.helper, "session", "--directory", String(root.settings.directory)] : [root.helper, "session"]
    stdinEnabled: true
    running: true
    stdout: SplitParser {
      onRead: function(line) {
        var reply
        try { reply = JSON.parse(line) } catch (e) { root.error = "Invalid runtime response"; return }
        if (reply.event === "ready") { root.ready = true; root.apply(reply.snapshot); return }
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
      var callbacks = root.callbacks; root.callbacks = ({})
      for (var id in callbacks) callbacks[id](null,"Runtime stopped")
      if (code !== 0 && root.error === "") root.error = "Runtime exited (" + code + "). Rebuild or start it again."
    }
  }
  ShellIpc {
    target: "io.github.commanderturtle.omarsket"
    function stop(): void { root.stop() }
    function run(name: string): void { root.call({method:"run", name:name}) }
  }
}
