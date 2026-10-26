import {test,expect} from "bun:test"
import {readFileSync} from "node:fs"
import {runInNewContext} from "node:vm"
const read=file=>readFileSync(new URL("../qml/"+file,import.meta.url),"utf8")
const B=runInNewContext(read("model/Bindings.js")+"\n({chord,layer,rows,presentation,physical,replacement})")
const K=runInNewContext(read("model/Keyboard.js")+"\n({layout,mouse})")
const T=runInNewContext(read("model/Tasks.js").replace(/^\.pragma library\s*/,"")+"\n({filename,uniqueName})")
test("layers distinguish native actions, application commands and macro schedules",()=>{
  const bindings=[
    {key:"f",modmask:64,dispatcher:"exec",arg:"omarchy-launch-file-manager",description:"Files"},
    {key:"g",modmask:64,dispatcher:"exec",arg:"omarchy-shell -q io.github.commanderturtle.omarsket run 'Work'",description:"Work"},
    {key:"h",modmask:64,dispatcher:"workspace",arg:"1",description:"Workspace"},
    {key:"j",modmask:64,dispatcher:"exec",arg:"omarchy-lock-screen",description:"Lock"},
    {key:"k",modmask:64,dispatcher:"exec",arg:"printf %s text | wtype -",description:"Text"},
  ]
  expect(bindings.map(B.layer)).toEqual(["software","macro","os","os","software"])
  expect(B.rows(bindings,"",{},"macro").map(b=>b.description)).toEqual(["Work"])
  expect(B.rows(bindings,"",{},"os").map(b=>b.description)).toEqual(["Workspace","Lock"])
  expect(B.layer({dispatcher:"exec_cmd",arg:"editor"})).toBe("software")
  expect(B.rows([{key:"f",submap:"resize",dispatcher:"exec",arg:"editor"}],"",{},"software")[0].scope).toBe("resize")
})
test("maps distinguish assigned, other-layer, inactive-default and free keys for the selected modifiers",()=>{
  const map={codes:{f:[41],g:[42],h:[43]}},keys=K.layout(),bindings=[
    {key:"F",modmask:64,dispatcher:"exec",arg:"editor",description:"Editor"},
    {key:"G",modmask:64,dispatcher:"exec",arg:"omarchy-shell -q io.github.commanderturtle.omarsket run 'Work'",description:"Work"},
  ]
  const defaults='-- o.bind("SUPER + A", "comment", "ignored")\no.bind("SUPER + H", "Old editor", "old-editor")\no.bind("SUPER + F", "Editor", "editor")'
  const p=B.presentation(keys,bindings,defaults,map,["SUPER"],"software")
  expect(p.states[33]).toBe("assigned");expect(p.states[34]).toBe("other");expect(p.states[35]).toBe("default");expect(p.states[36]).toBe("available")
  expect(p.hints[35]).toContain("Old editor");expect(p.hints[34]).toContain("Work")
  expect(B.rows(bindings,defaults,map,"software").filter(b=>!b.active).map(b=>b.description)).toEqual(["Old editor"])
  expect(B.presentation(keys,bindings,defaults,map,[],"software").states[33]).toBe("available")
  expect(B.presentation(keys,bindings,defaults,{error:"unknown layout"},["SUPER"],"software").states[36]).toBe("unknown")
})
function editor(){
  const pending=[],s={nativeState:{text:"-- user file",defaults:""},draft:"-- user file",loading:false,editing:false,defaults:false,showSource:false,configFile:"bindings.lua",error:"",keyId:"",chord:"",modifiers:["SUPER","SHIFT"],alive:true,generation:0,kind:"schedule",target:"",description:"",collisions:[]}
  s.root=s;s.service={tasks:[{id:"Work"}],chooseFile(save,callback){pending.push({picker:callback})},change(request,callback){pending.push({request,callback})},inputConfigCommand(){throw Error("No automatic configuration write")}}
  const source=read("Bindings.qml"),code=source.slice(source.indexOf("  function load()"),source.indexOf("  onBindingLayerChanged:"))
  runInNewContext(code,Object.assign(s,{Bindings:B,Tasks:T}))
  return {s,pending}
}
test("right-click assignment preserves modifiers and .omscht import stages a unique name without running or saving",()=>{
  const {s,pending}=editor()
  s.beginAssignment(K.layout().find(k=>k.code===33));expect(s.editing).toBe(true);expect(s.chord).toBe("SHIFT + SUPER + code:41")
  s.chooseSchedule();pending[0].picker("/tasks/Work.omscht")
  expect(pending[1].request).toEqual({method:"import",path:"/tasks/Work.omscht",name:"Work 2"})
  pending[1].callback(true,null)
  expect(s.target).toBe("Work 2");expect(s.draft).toBe("-- user file")
  s.appendBinding();expect(s.draft).toContain("io.github.commanderturtle.omarsket run")
  expect(pending).toHaveLength(2)
})
test("file picker cancellation races and bad files cannot bind a different chord or discard a draft",()=>{
  const {s,pending}=editor();s.chord="SUPER + F9";s.editing=true;s.chooseSchedule()
  s.chord="SUPER + F10";pending[0].picker("/tasks/Work.omscht")
  expect(pending).toHaveLength(1);expect(s.error).toContain("chord changed");expect(s.draft).toBe("-- user file")
  s.chooseSchedule();pending[1].picker("/tasks/Work.json")
  expect(pending).toHaveLength(2);expect(s.error).toContain(".omscht")
  s.chooseSchedule();s.generation++;pending[2].picker("/tasks/Work.omscht");expect(pending).toHaveLength(3)
  s.configFile="input.lua";s.beginAssignment(null);expect(s.error).toContain("Switch to bindings.lua")
  s.chooseSchedule();expect(pending).toHaveLength(3)
})
test("keyboard has discoverable right click, pretty inventories and distinct read-only/remap roles",()=>{
  expect(read("KeyboardMap.qml")).toContain("Qt.LeftButton|Qt.RightButton")
  expect(read("Bindings.qml")).toContain('onKeyContextRequested:function(key){root.beginAssignment(key)}')
  expect(read("Bindings.qml")).toContain('["Assignments","Inactive defaults","Unassigned keys"]')
  expect(read("Bindings.qml")).toContain('text:"Choose .omscht…"')
  expect(read("Bindings.qml")).not.toMatch(/onBindingLayerChanged:.*load\(\)/)
  expect(read("InputRouter.qml")).toContain("states:root.remapPresentation.states")
  expect(read("Hardware.qml")).toContain("device-reported state (read-only)")
  expect(read("InputLayers.qml")).toContain('panelVisible&&inputLayer==="heatmap"')
})
test("leaving an unsaved binding editor asks before destroying its draft",()=>{
  const source=read("InputLayers.qml"),s={inputLayer:"macro",pendingLayer:"",bindingsView:{item:{editing:true}}}
  const code=source.slice(source.indexOf("  function selectLayer("),source.indexOf("  function updateVisibility("))
  runInNewContext(code,s)
  s.selectLayer("heatmap");expect(s.inputLayer).toBe("macro");expect(s.pendingLayer).toBe("heatmap")
  s.selectLayer("software");expect(s.inputLayer).toBe("software");expect(s.pendingLayer).toBe("")
  s.bindingsView.item.editing=false;s.selectLayer("heatmap");expect(s.inputLayer).toBe("heatmap")
})
test("schedule IPC closes the editor and waits before queueing, and Stop invalidates pending shortcuts",()=>{
  const source=read("Service.qml"),s={ready:true,tasks:[{id:"Work"}],operationEpoch:0,shortcutQueue:[],error:"",hidden:0,posts:[],shortcutDelay:{restart(){}}}
  s.root=s;s.hidePanel=()=>s.hidden++;s.change=req=>s.posts.push(req)
  const code=source.slice(source.indexOf("  function runSchedule("),source.indexOf("  Timer {id:shortcutDelay"))
  runInNewContext(code,s)
  s.runSchedule("Work.omscht");expect(s.hidden).toBe(1);expect(s.posts).toHaveLength(0)
  const line=source.split("\n").find(l=>l.includes("id:shortcutDelay")),handler=line.slice(line.indexOf("onTriggered:{")+13,-2)
  s.operationEpoch++;runInNewContext(handler,s);expect(s.posts).toHaveLength(0)
  s.runSchedule("Work");runInNewContext(handler,s);expect(s.posts).toEqual([{method:"run",name:"Work"}])
  s.runSchedule("missing");expect(s.error).toContain("not found")
})
