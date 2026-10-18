.pragma library
function copy(value) { return JSON.parse(JSON.stringify(value)) }
function blank(name) { return {id:name, description:"", delay_seconds:2, loop_times:1, auto_start:false, allow_system:false, actions:[]} }
function action(type) {
  switch (type) {
  case "paste": return {type:type, content:"", contentId:"", loop:1}
  case "wait": return {type:type, duration:1}
  case "keyssequence": return {type:type, keysmap:{"0":[100,["CONTROL_LEFT","C"]]}, keysSeqId:"", loop:1}
  case "cursormovements": return {type:type, cursormovsmap:[[0,100,0,0]], cursorMovsId:"", optionalkeysstroke:[], loop:1}
  case "systemcommand": return {type:type, sysCommandType:"openurl", sysCommandParam1:"", sysCommandParam2:""}
  case "runningothertask": return {type:type, otherTaskName:"", otherTaskDelay:0, otherTaskLoops:1}
  case "move": return {type:type, x:0, y:0, duration_ms:100}
  case "click": return {type:type, x:0, y:0, button:"left", count:1}
  case "scroll": return {type:type, vertical:1, horizontal:0}
  case "hotkey": return {type:type, keys:["ctrl","s"]}
  case "type": return {type:type, text:"", interval_ms:0}
  default: throw Error("Unknown action: " + type)
  }
}
function duplicate(actions, times) {
  if (times < 1 || times > 100 || times % 1 !== 0 || actions.length * (times + 1) > 10000) throw Error("Duplication exceeds the action limit")
  var result = copy(actions)
  for (var i=0; i<times; i++) result = result.concat(copy(actions))
  return result
}
function move(actions, index, delta) {
  var result = copy(actions), next = index + delta
  if (next < 0 || next >= result.length) return result
  var removed = result.splice(index,1)[0]; result.splice(next,0,removed)
  return result
}
function title(a) {
  var labels = {paste:"Paste", wait:"Wait", keyssequence:"Key sequence", cursormovements:"Cursor path", systemcommand:"System command", runningothertask:"Run task", move:"Move", click:"Click", scroll:"Scroll", hotkey:"Hotkey", type:"Type"}
  return labels[a.type] || a.type
}
function uniqueName(names, base) {
  var name=base, n=2
  while (names.indexOf(name)!==-1) name=base+" "+n++
  return name
}
function filename(path) { return path.split(/[\\/]/).pop().replace(/\.(omscht|scht|json)$/i, "") }
function jobText(job, now) {
  if (job.state==="scheduled") {
    var seconds=Math.max(0,Math.ceil((job.due_ms-now)/1000))
    return seconds ? "In "+seconds+"s" : "Waiting for input queue"
  }
  if (job.state==="running") return "Action "+(job.action+1)+"/"+job.total_actions+" · loop "+(job.completed_loops+1)+(job.loop_times<0?"/∞":"/"+job.loop_times)
  return job.state+" · "+job.completed_loops+" loops"
}
function key(event) {
  var names = {16777216:"Escape",16777217:"Tab",16777219:"BackSpace",16777220:"Return",16777221:"Return",16777222:"Insert",16777223:"Delete",16777225:"Print",16777232:"Home",16777233:"End",16777234:"Left",16777235:"Up",16777236:"Right",16777237:"Down",16777238:"Page_Up",16777239:"Page_Down",16777248:"shift",16777249:"ctrl",16777250:"super",16777251:"alt",32:"space"}
  if (names[event.key]) return names[event.key]
  if (event.key >= 16777264 && event.key <= 16777298) return "F" + (event.key-16777263)
  return String(event.key>=33&&event.key<=126?String.fromCharCode(event.key):event.text||"").toLowerCase()
}
