import { test, expect } from "bun:test"
import { readFileSync } from "node:fs"
import { runInNewContext } from "node:vm"
const base=new URL("../qml/",import.meta.url)
const K=runInNewContext(readFileSync(new URL("model/Keyboard.js",base),"utf8")+"\n({layout,mouse,maximum,normalized})")
const B=runInNewContext(readFileSync(new URL("model/Bindings.js",base),"utf8")+"\n({chord,nativeChord,conflicts,physical,line,lua,routingWarnings,sourceChords,replacement,draftConflicts,profileWarnings})")

test("virtual input warnings cover source, target and changed modifier bindings",()=>{
  const native=[{key:"f",modmask:64,description:"Files"},{key:"g",modmask:4,description:"Other"}]
  const map={codes:{f:[41],g:[42]}}
  expect(B.profileWarnings({devices:[{path:"kbd",keys:{33:[34]}}]},native,map)).toHaveLength(2)
  expect(B.profileWarnings({devices:[{path:"kbd",keys:{125:[]}}]},native,map)[0]).toContain("Files")
  expect(B.profileWarnings({devices:[]},native,map)).toEqual([])
  const ui=readFileSync(new URL("InputRouter.qml",base),"utf8")
  expect(ui).toContain('if(root.savedWarnings.length)root.pendingStart=root.selected')
  expect(ui).toContain('Native bindings will not be removed.')
})

test("conflict replacement uses original native chord order, not a recreated callback",()=>{
  const source='o.bind("SUPER + SHIFT + E", "Browser", function() launch_browser() end, { repeating = true })'
  const active=[{key:"E",modmask:65,description:"Browser"}]
  const text=B.replacement("SHIFT + SUPER + code:26","command","terminal","Shell",active,source)
  expect(text).toStartWith('hl.unbind("SUPER + SHIFT + E")\n')
  expect(text).toContain('o.bind("SHIFT + SUPER + code:26", "Shell", "terminal")')
  expect(text).not.toContain("launch_browser")
  expect(()=>B.replacement("SUPER+E","command","x","",active,"")).toThrow("Exact native chord unavailable")
  expect(B.draftConflicts("SUPER+code:26",'o.bind("SUPER + e", "New", "x")',{codes:{e:[26]}})).toHaveLength(1)
})

test("literal chord extraction skips comments, strings and dynamic Lua expressions",()=>{
  const source=`-- o.bind("SUPER + A", "comment", "x")
--[=[ o.bind("SUPER + B", "comment", "x") ]=]
local text = 'o.bind("SUPER + C", "text", "x")'
local other = [[o.bind("SUPER + D", "text", "x")]]
o.bind(prefix .. "E", "dynamic", "x")
o.bind_hold('CTRL + SUPER + F', "Press", down, "Release", up)
hl.bind("SUPER + code:42", callback)
o.rebind("SUPER + G", "G", { launch="thing" })`
  expect(B.sourceChords(source)).toEqual(["CTRL + SUPER + F","SUPER + code:42","SUPER + G"])
})

test("collision confirmation stages edits and keeps native authorization and theme",()=>{
  const panel=readFileSync(new URL("Bindings.qml",base),"utf8")
  expect(panel).toContain("Already bound:")
  expect(panel).toContain('text:"Replace bindings"')
  expect(panel).toContain('text:"Keep existing"')
  expect(panel).toContain('JSON.stringify(collisions)!==pending.signature')
  expect(panel).toContain("Commons.Color.urgent")
  expect(panel).toContain("root.nativeState.text")
})

test("replacement needs confirmation and rejects edits made after the prompt",()=>{
  const qml=readFileSync(new URL("Bindings.qml",base),"utf8")
  const end=qml.indexOf("  onBindingLayerChanged:")
  expect(end).toBeGreaterThan(qml.indexOf("  function appendBinding()"))
  const methods=qml.slice(qml.indexOf("  function appendBinding()"),end)
  const state={draft:"-- user config",editing:true,chord:"SUPER+SHIFT+E",kind:"command",target:"terminal",description:"Shell",error:"",pendingReplacement:null,nativeState:{defaults:'o.bind("SUPER + SHIFT + E", "Browser", browser_callback)'},collisions:[{key:"E",modmask:65,description:"Browser"}]}
  const fn=new Function("root","Bindings",`with(root){${methods};return {appendBinding,confirmReplacement}}`)(state,B)
  fn.appendBinding()
  expect(state.draft).toBe("-- user config")
  expect(state.pendingReplacement.labels).toContain("Browser")
  state.target="changed after prompt";fn.confirmReplacement()
  expect(state.draft).toBe("-- user config")
  expect(state.error).toContain("review the conflict again")
  fn.appendBinding();fn.confirmReplacement()
  expect(state.draft).toContain('hl.unbind("SUPER + SHIFT + E")')
  expect(state.draft).toContain('"changed after prompt"')
  expect(state.pendingReplacement).toBeNull()
})
test("full-size physical layout includes numpad, media, Fn, Copilot and side buttons",()=>{
  const keys=K.layout()
  expect(keys.length).toBeGreaterThanOrEqual(104)
  expect(new Set(keys.map(k=>k.id)).size).toBe(keys.length)
  for(const key of ["Fn","Copilot","Vol+","Mute","Num"]){expect(keys.some(k=>k.label===key)).toBe(true)}
  expect(keys.find(k=>k.code===96).h).toBe(2)
  expect(keys.find(k=>k.code===82).w).toBe(2)
  for(const k of keys){expect(k.x+k.w).toBeLessThanOrEqual(22.8);expect(k.y+k.h).toBeLessThanOrEqual(6.4)}
  expect(K.mouse().map(k=>k.code)).toEqual([272,274,273,275,276])
  expect(keys.find(k=>k.id==="copilot").code).toBeNull()
})

test("native key identity preserves exact case and physical/symbol collisions",()=>{
  const bindings=[{key:"F",modmask:64,description:"File manager"},{keycode:104,modmask:4,description:"Numpad"}]
  const map={codes:{f:[41]}}
  expect(B.nativeChord(bindings[0])).toBe("SUPER + F")
  expect(B.conflicts([{chord:"SUPER+code:41"}],bindings,map)).toHaveLength(1)
  expect(B.conflicts([{chord:"SUPER+code:42"}],bindings,map)).toHaveLength(0)
  expect(B.nativeChord({key:"F9",modmask:128})).toBe("MOD5 + F9")
  expect(B.physical({code:275},["CTRL"])).toBe("CTRL + mouse:275")
  expect(B.line("SUPER+F9","profile","1 - Test","Preset")).toContain("macrohelp profile")
  expect(B.line("SUPER+F9","text","—","Dash")).toContain("| wtype -")
  expect(B.lua("\"\n\\")).toBe('"\\"\\010\\\\"')
})
test("bindings live only in native configuration and privileged edits use the system editor",()=>{
  const source=readFileSync(new URL("../src/input.rs",base),"utf8")
  expect(source).not.toContain("input-profiles.json")
  expect(source).not.toContain("omarsket-bindings.lua")
  expect(source).toContain('"sudoedit"')
  expect(source).toContain('r["expected"].as_str()')
  expect(source).toContain('ipc("configerrors")')
  const panel=readFileSync(new URL("Bindings.qml",base),"utf8")
  expect(panel).toContain('iconText:"󰏫"');expect(panel).toContain('iconText:"󰆓"')
})
test("heatmap colors are normalized independently of stored weighted averages",()=>{
  const session={values:{30:.05,48:.25}}
  expect(K.normalized(session)).toEqual(session.values)
  expect(K.maximum(session.values)).toBe(.25)
  expect(K.maximum({})).toBeGreaterThan(0)
})
test("native binding warnings account for active physical-input remaps",()=>{
  const routing={devices:[{path:"keyboard",keys:{33:[34]}}]},keymap={codes:{f:[41],g:[42]}}
  expect(B.routingWarnings("SUPER+F",routing,keymap)).toEqual(["keyboard: 33 → 34"])
  expect(B.routingWarnings("SUPER+code:42",routing,keymap)).toHaveLength(1)
  expect(B.routingWarnings("SUPER+code:43",routing,keymap)).toHaveLength(0)
  expect(B.routingWarnings("SUPER+F",null,keymap)).toHaveLength(0)
})
test("failed heatmap health checks can retry without overlapping requests or polling a connected daemon",()=>{
  const source=readFileSync(new URL("Service.qml",base),"utf8")
  const start=source.slice(source.indexOf("  function openHeatmap()"),source.indexOf("  function heatmapCommand("))
  const state={heatmapConnected:false,heatmapError:"failed to connect to macrohelp"}
  const health={running:false,command:[]},settings={macrohelpEnvFile:"/private/overlay.env"}
  const open=new Function("root","health","macrohelp","settings",start+";return openHeatmap")(state,health,"/bin/macrohelpctl",settings)
  open()
  expect(health.command).toEqual(["/bin/macrohelpctl","health","--env","/private/overlay.env"])
  expect(health.running).toBe(true)
  expect(state.heatmapError).toBe("")
  health.command=["in flight"];open();expect(health.command).toEqual(["in flight"])
  health.running=false;state.heatmapError="failed to connect to macrohelp"
  open();expect(health.command[1]).toBe("health")
  health.running=false;state.heatmapConnected=true;health.command=["connected"]
  open();expect(health.running).toBe(false);expect(health.command).toEqual(["connected"])
})
test("heatmap transport failure disconnects and discards queued mutations before a retry",()=>{
  const source=readFileSync(new URL("Service.qml",base),"utf8")
  const process=source.slice(source.indexOf("    id:bridge"),source.indexOf("  readonly property string helper:"))
  const handler=process.slice(process.indexOf("    onExited:function(code){")+"    onExited:function(code){".length,process.lastIndexOf("\n    }"))
  const state={heatmapConnected:true,heatmapAvailable:true,heatmapQueue:['{"action":"start"}'],heatmapError:"",nextHeatmap(){this.advanced=true}}
  const finish=new Function("root","bridgeOutput","code",handler)
  finish(state,{text:""},1)
  expect(state.heatmapConnected).toBe(false)
  expect(state.heatmapAvailable).toBe(false)
  expect(state.heatmapQueue).toEqual([])
  expect(state.heatmapError).toContain("failed to connect")
  expect(state.advanced).toBe(true)
  state.heatmapConnected=true;state.heatmapQueue=['{"action":"snapshot"}']
  finish(state,{text:'{"error":"Session name is required"}'},0)
  expect(state.heatmapConnected).toBe(true)
  expect(state.heatmapQueue).toHaveLength(1)
  expect(state.heatmapError).toBe("Session name is required")
})
test("heatmap uses native aggregate IPC only while the panel is visible",()=>{
  const source=readFileSync(new URL("Service.qml",base),"utf8")
  expect(source).toContain('[macrohelp,"health"]')
  expect(source).toContain('"io.github.commanderturtle.macrohelp","heatmap"')
  expect(source).toContain("running:root.heatmapConnected&&root.heatmapVisible")
  expect(source).not.toMatch(/serviceFor\(["']io.github.commanderturtle.macrohelp/)
  expect(source).toContain("failed to connect to macrohelp")
  const panel=readFileSync(new URL("Panel.qml",base),"utf8")
  expect((panel.match(/onOpenedChanged:/g)||[])).toHaveLength(1)
})
