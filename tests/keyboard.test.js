import { test, expect } from "bun:test"
import { readFileSync } from "node:fs"
import { runInNewContext } from "node:vm"
const base=new URL("../qml/",import.meta.url)
const K=runInNewContext(readFileSync(new URL("model/Keyboard.js",base),"utf8")+"\n({layout,mouse,maximum,normalized,label,geometry})")
const B=runInNewContext(readFileSync(new URL("model/Bindings.js",base),"utf8")+"\n({chord,nativeChord,conflicts,physical,line,lua,routingWarnings,sourceChords,sourceBindings,replacement,draftConflicts,profileWarnings,keyHints,layer,rows,presentation})")

test("keymap legends change without changing physical identities or geometry",()=>{
  const a=K.layout().find(k=>k.code===30),map={labels:{30:"q",99:"Print",70:"Scroll_Lock",119:"Pause"}}
  expect(K.label(a,map)).toBe("Q");expect(B.physical(a,[])).toBe("code:38")
  expect(K.label(a,{labels:{30:"й"}})).toBe("Й")
  expect(K.label(a,{})).toBe("A")
  for(const code of [99,70,119])expect(K.layout().some(k=>k.code===code)).toBe(true)
  for(const width of [0,120,650,1200])for(const mouse of [false,true]){
    const g=K.geometry(width,mouse)
    expect(g.unit).toBeGreaterThanOrEqual(0)
    for(const k of K.layout())expect((k.y+k.h)*g.unit).toBeLessThanOrEqual(g.height+1e-9)
    if(mouse)for(const k of K.mouse())expect((k.y+k.h+6.8)*g.unit).toBeLessThanOrEqual(g.height+1e-9)
  }
  expect(K.geometry(-10,false).height).toBe(0)
})

test("keyboard binding indicators respect modifiers, physical keys, mouse and submaps",()=>{
  const keys=K.layout().concat(K.mouse()),map={codes:{f:[41]}}
  const native=[{key:"f",modmask:64,description:"Files"},{key:"mouse:272",modmask:64,description:"Pointer"},{key:"f",modmask:4,description:"Control"},{key:"f",modmask:64,submap:"resize",description:"Inactive"}]
  const hints=B.keyHints(keys,native,map,["SUPER"])
  expect(hints[33]).toContain("Files");expect(hints[33]).not.toContain("Control");expect(hints[33]).not.toContain("Inactive")
  expect(hints[272]).toContain("Pointer");expect(hints.copilot).toBeUndefined()
  expect(B.keyHints(keys,native,map,[])).toEqual({})
})

function nativeService(){
  const source=readFileSync(new URL("Service.qml",base),"utf8")
  const code=source.slice(source.indexOf("  function refreshNative()"),source.indexOf("  Timer {id:nativeRefreshDelay"))
  const requests=[],state={ready:true,nativeRefreshing:false,nativeRefreshPending:false,nativeEpoch:0,nativeRevision:0,nativeError:"",nativeBindings:[{key:"old"}],nativeKeymap:{},nativeRouting:null}
  const delay={count:0,restart(){this.count++}}
  state.call=(request,callback)=>requests.push({request,callback})
  Object.assign(state,new Function("root","nativeRefreshDelay",`with(root){${code};return {refreshNative,nativeEvent}}`)(state,delay))
  return {state,requests,delay}
}

test("keyboard startup and refresh are event-driven, with no pointer or key-event polling",()=>{
  const service=readFileSync(new URL("Service.qml",base),"utf8"),panel=readFileSync(new URL("Panel.qml",base),"utf8"),layers=readFileSync(new URL("InputLayers.qml",base),"utf8")
  expect(service).toContain('root.apply(reply.snapshot); root.refreshNative();')
  expect(service).toContain('function onRawEvent(event){root.nativeEvent(event.name)}')
  expect(layers).toContain('Component.onCompleted:{service.refreshNative();updateVisibility()}')
  expect(layers).toContain('onClicked:root.service.refreshNative()')
  expect(panel).toContain('scroll.contentItem.contentY=0')
  expect(layers).toContain('onInputLayerChanged:{updateVisibility();resetScroll()}')
  const refresh=service.slice(service.indexOf('function refreshNative()'),service.indexOf('signal stopping()'))
  for(const forbidden of ['repeat:true','cursorpos','repl','reload"','dispatch','keyPressed'])expect(refresh).not.toContain(forbidden)
})

test("changing modifiers updates a selected physical chord without replacing a manually typed chord",()=>{
  const source=readFileSync(new URL("Bindings.qml",base),"utf8")
  const line=source.split('\n').find(l=>l.includes('onModifiersChanged:'))
  const fn=new Function('root','Keyboard','Bindings',`with(root)${line.slice(line.indexOf(':')+1)}`)
  const s={keyId:"33",modifiers:["CTRL"],chord:"code:41"}
  fn(s,K,B);expect(s.chord).toBe("CTRL + code:41")
  s.keyId="";s.chord="SUPER + F9";s.modifiers=["ALT"]
  fn(s,K,B);expect(s.chord).toBe("SUPER + F9")
})

test("native refresh coalesces requests, rejects invalidated results and retains good data on failure",()=>{
  const {state:s,requests:r,delay}=nativeService()
  s.refreshNative();s.refreshNative()
  expect(r).toHaveLength(1);expect(r[0].request).toEqual({method:"input-config",action:"describe"})
  r[0].callback({bindings:[{key:"stale"}],keymap:{}})
  expect(s.nativeBindings[0].key).toBe("old");expect(r).toHaveLength(2)
  r[1].callback({bindings:[{key:"new"}],keymap:{layout:"French"},routing:null})
  expect(s.nativeBindings[0].key).toBe("new");expect(s.nativeRevision).toBe(1);expect(s.nativeRefreshing).toBe(false)
  s.refreshNative();r[2].callback(null,"offline")
  expect(s.nativeBindings[0].key).toBe("new");expect(s.nativeError).toBe("offline")
  s.refreshNative();r[3].callback({bindings:{}})
  expect(s.nativeError).toContain("Invalid native");expect(s.nativeBindings[0].key).toBe("new")
  s.refreshNative();s.nativeEpoch++;r[4].callback({bindings:[]})
  expect(s.nativeBindings[0].key).toBe("new")
  for(const name of ["configreloaded","activelayout","activewindow","mousemove"])s.nativeEvent(name)
  expect(delay.count).toBe(2)
  s.ready=false;s.nativeRefreshing=false;s.refreshNative();expect(r).toHaveLength(5)
})

test("refreshing hotkey metadata preserves unsaved binding drafts and cancels stale replacement prompts",()=>{
  const source=readFileSync(new URL("Bindings.qml",base),"utf8")
  const begin=source.indexOf("function onNativeRevisionChanged(){")+"function onNativeRevisionChanged(){".length
  const code=source.slice(begin,source.indexOf("\n  }",begin)).trim().replace(/}$/,"")
  const s={editing:true,sourceChanged:false,pendingReplacement:{},draft:"unsaved",load(){this.loaded=true}}
  new Function("root",code)(s)
  expect(s.draft).toBe("unsaved");expect(s.sourceChanged).toBe(true);expect(s.pendingReplacement).toBeNull();expect(s.loaded).toBeUndefined()
  s.editing=false;new Function("root",code)(s);expect(s.loaded).toBeUndefined()
  expect(source).toContain('chord=key.code===null?"":Bindings.physical(key,modifiers)')
  const input=readFileSync(new URL("../src/input.rs",base),"utf8")
  const describe=input.slice(input.indexOf("fn describe()"),input.indexOf("fn read_binding_sources"))
  expect(describe).toContain('ipc("binds")?')
  for(const forbidden of ["reload", "repl", "atomic_text", "config_path"])expect(describe).not.toContain(forbidden)
})

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
  expect(panel).toContain("expected:nativeState.text")
  expect(panel).toContain('text:"Cancel";visible:root.editing;onClicked:root.load()')
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
test("heatmap retries probe its real IPC surface without the unrelated HTTP health gate",()=>{
  const source=readFileSync(new URL("Service.qml",base),"utf8")
  const start=source.slice(source.indexOf("  function openHeatmap()"),source.indexOf("  function heatmapCommand("))
  const state={heatmapConnected:false,heatmapError:"failed to connect to macrohelp"}
  const bridge={running:false},requests=[]
  state.heatmapCommand=request=>{requests.push(request);bridge.running=true}
  const open=new Function("root","bridge",start+";return openHeatmap")(state,bridge)
  open()
  expect(requests).toEqual([{action:"snapshot"}])
  expect(state.heatmapError).toBe("")
  open();expect(requests).toHaveLength(1)
  bridge.running=false;state.heatmapError="offline"
  open();expect(requests).toHaveLength(2)
  bridge.running=false;state.heatmapConnected=true
  open();expect(bridge.running).toBe(false);expect(requests).toHaveLength(2)
})
test("heatmap transport failure disconnects and discards queued mutations before a retry",()=>{
  const source=readFileSync(new URL("Service.qml",base),"utf8")
  const process=source.slice(source.indexOf("    id:bridge"),source.indexOf("  readonly property string helper:"))
  const handler=process.slice(process.indexOf("    onExited:function(code){")+"    onExited:function(code){".length,process.lastIndexOf("\n    }"))
  const state={heatmapConnected:true,heatmapAvailable:true,heatmapQueue:['{"action":"start"}'],heatmapError:"",nextHeatmap(){this.advanced=true}}
  const finish=new Function("root","bridgeOutput","code","bridgeError",handler)
  finish(state,{text:""},1,{text:"Target not found."})
  expect(state.heatmapConnected).toBe(false)
  expect(state.heatmapAvailable).toBe(false)
  expect(state.heatmapQueue).toEqual([])
  expect(state.heatmapError).toContain("Target not found.")
  expect(state.advanced).toBe(true)
  state.heatmapConnected=true;state.heatmapQueue=['{"action":"snapshot"}']
  finish(state,{text:'{"error":"Session name is required"}'},0)
  expect(state.heatmapConnected).toBe(true)
  expect(state.heatmapQueue).toHaveLength(1)
  expect(state.heatmapError).toBe("Session name is required")
  finish(state,{text:""},0)
  expect(state.heatmapConnected).toBe(false);expect(state.heatmapError).toContain("returned no JSON")
  finish(state,{text:'{"state":{"version":1,"active":null,"sessions":[]},"connected":true,"pending":true,"available":true}'},0)
  expect(state.heatmapConnected).toBe(true);expect(state.heatmapBusy).toBe(true);expect(state.heatmapAvailable).toBe(true)
})
test("heatmap uses native aggregate IPC only while the panel is visible",()=>{
  const source=readFileSync(new URL("Service.qml",base),"utf8")
  expect(source).not.toContain('[macrohelp,"health"]')
  expect(source).toContain('["omarchy-shell","io.github.commanderturtle.macrohelp","heatmap"')
  expect(source).not.toContain('["omarchy-shell","-q","io.github.commanderturtle.macrohelp","heatmap"')
  expect(source).toContain('"io.github.commanderturtle.macrohelp","heatmap"')
  expect(source).toContain("running:root.heatmapConnected&&root.heatmapVisible")
  expect(source).not.toMatch(/serviceFor\(["']io.github.commanderturtle.macrohelp/)
  expect(source).toContain("Macrohelp heatmap IPC:")
  const panel=readFileSync(new URL("Panel.qml",base),"utf8")
  expect((panel.match(/onOpenedChanged:/g)||[])).toHaveLength(1)
})
