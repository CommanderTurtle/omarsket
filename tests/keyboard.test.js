import { test, expect } from "bun:test"
import { readFileSync } from "node:fs"
import { runInNewContext } from "node:vm"
const base=new URL("../qml/",import.meta.url)
const K=runInNewContext(readFileSync(new URL("model/Keyboard.js",base),"utf8")+"\n({layout,mouse,maximum,normalized})")
const B=runInNewContext(readFileSync(new URL("model/Bindings.js",base),"utf8")+"\n({chord,nativeChord,conflicts,physical,line,lua,routingWarnings})")
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
test("one HTTP health check, then native aggregate IPC only while the panel is visible",()=>{
  const source=readFileSync(new URL("Service.qml",base),"utf8")
  expect(source).toContain("if(heatmapChecked)return")
  expect(source).toContain('[macrohelp,"health"]')
  expect(source).toContain('"io.github.commanderturtle.macrohelp","heatmap"')
  expect(source).toContain("running:root.heatmapConnected&&root.heatmapVisible")
  expect(source).not.toMatch(/serviceFor\(["']io.github.commanderturtle.macrohelp/)
  expect(source).toContain("failed to connect to macrohelp")
  const panel=readFileSync(new URL("Panel.qml",base),"utf8")
  expect((panel.match(/onOpenedChanged:/g)||[])).toHaveLength(1)
})
