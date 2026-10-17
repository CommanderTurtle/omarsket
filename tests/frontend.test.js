import { test, expect } from "bun:test";
import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";

const root=resolve(import.meta.dir,"..");
const source=readFileSync(resolve(root,"qml/model/Tasks.js"),"utf8").replace(/^\.pragma library\s*/,"");
const api=new Function(source+";return {blank,action,duplicate,move,title,key}")();
test("every editor template has a valid independent action",()=>{
  for(const type of ["paste","wait","keyssequence","cursormovements","systemcommand","runningothertask","move","click","scroll","hotkey","type"]){
    expect(api.action(type).type).toBe(type);
    expect(api.action(type)).not.toBe(api.action(type));
  }
});
test("duplication and reorder do not mutate source actions",()=>{
  const a=[{type:"paste",content:"a"},{type:"wait",duration:1}];
  const doubled=api.duplicate(a,1);doubled[2].content="b";
  expect(a[0].content).toBe("a");expect(doubled.length).toBe(4);
  expect(api.move(a,0,1)[1].content).toBe("a");
  expect(()=>api.duplicate(a,101)).toThrow();
});
test("new tasks cannot silently enable startup or commands",()=>{
  const task=api.blank("new");expect(task.auto_start).toBe(false);expect(task.allow_system).toBe(false);
});
test("recorded keys preserve keysyms and modifiers",()=>{
  expect(api.key({key:16777249,text:""})).toBe("ctrl");
  expect(api.key({key:16777275,text:""})).toBe("F12");
  expect(api.key({key:65,text:"A"})).toBe("a");
});
test("native manifest points to real source components",()=>{
  const manifest=JSON.parse(readFileSync(resolve(root,"manifest.json"),"utf8"));
  expect(manifest.kinds).toEqual(["service","bar-widget"]);
  for(const file of Object.values(manifest.entryPoints))expect(existsSync(resolve(root,file))).toBe(true);
  const panel=readFileSync(resolve(root,"qml/Panel.qml"),"utf8");
  expect(panel).toContain("Ui.KeyboardPanel");expect(panel).toContain("Commons.Color");
  expect(panel).not.toMatch(/#[0-9a-f]{6}/i);
});
