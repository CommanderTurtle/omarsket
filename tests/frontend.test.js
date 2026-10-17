import { test, expect } from "bun:test";
import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";

const root=resolve(import.meta.dir,"..");
const source=readFileSync(resolve(root,"qml/model/Tasks.js"),"utf8").replace(/^\.pragma library\s*/,"");
const api=new Function(source+";return {copy,blank,action,duplicate,move,title,key,uniqueName,filename,jobText}")();
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
test("schedule names and imported extensions are handled consistently",()=>{
  expect(api.uniqueName(["Task","Task 2"],"Task")).toBe("Task 3");
  expect(api.filename("/tmp/paste.omscht")).toBe("paste");
  expect(api.filename("C:\\Tasks\\paste.scht")).toBe("paste");
});
test("independent schedule countdown and loop progress",()=>{
  expect(api.jobText({state:"scheduled",due_ms:3000},1000)).toBe("In 2s");
  expect(api.jobText({state:"scheduled",due_ms:100},1000)).toBe("Waiting for input queue");
  expect(api.jobText({state:"running",action:2,total_actions:5,completed_loops:3,loop_times:-1},0)).toBe("Action 3/5 · loop 4/∞");
});
test("Tasket menus, temporary schedules and structured editors are wired",()=>{
  const panel=readFileSync(resolve(root,"qml/Panel.qml"),"utf8");
  for(const method of ["import-folder","import-library","export-library","duplicate","run-temporary","rescale","shortcut"])
    expect(panel).toContain(`method:"${method}"`);
  for(const file of ["TaskMenu.qml","ActionCard.qml"])
    expect(existsSync(resolve(root,"qml",file))).toBe(true);
  const editor=readFileSync(resolve(root,"qml/ActionEditor.qml"),"utf8");
  expect(editor).toContain("Add key event");expect(editor).toContain("Add position");
  expect(panel).toContain('app.preferences');
});

function panelModel() {
  const qml=readFileSync(resolve(root,"qml/Panel.qml"),"utf8");
  const script=qml.slice(qml.indexOf("  function set("),qml.indexOf("  Timer {id:startRun"));
  const names=[...script.matchAll(/^  function (\w+)\(/gm)].map(m=>m[1]);
  const scope={documents:[],documentIndex:-1,draft:api.blank("Untitled"),savedName:"",dirty:false,saving:false,
    selectedAction:0,tab:"Tasks",closingIndex:-1,confirmText:"",confirmCallback:null,
    app:{tasks:[{...api.blank("saved"),description:"original"}],change:(req,cb)=>cb(null,"test failure")}};
  scope.root=scope;
  new Function("scope","Tasks",`with(scope){${script};Object.assign(scope,{${names.join(",")}})}`)(scope,api);
  return scope;
}
test("task tabs preserve independent unsaved drafts and guard closing",()=>{
  const model=panelModel();
  model.choose("");model.set("description","scratch");
  model.choose("saved");expect(model.draft.description).toBe("original");
  model.set("description","edited");model.activate(0);
  expect(model.draft.description).toBe("scratch");
  model.activate(1);expect(model.draft.description).toBe("edited");
  model.closeDocument(1,false);expect(model.closingIndex).toBe(1);expect(model.documents.length).toBe(2);
  model.closeDocument(1,true);expect(model.documents.length).toBe(1);
  expect(model.draft.description).toBe("scratch");
});
test("save errors retain the draft and release the UI saving lock",()=>{
  const model=panelModel();model.choose("");model.set("description","do not lose");
  model.save();expect(model.saving).toBe(false);expect(model.dirty).toBe(true);
  expect(model.draft.description).toBe("do not lose");
});
test("a delayed delete response cannot discard another tab",()=>{
  const model=panelModel();model.choose("saved");model.removeTask();
  let respond;model.app.change=(req,cb)=>{respond=cb};
  model.confirmCallback();model.choose("");model.set("description","new scratch");respond(true,null);
  expect(model.documents.some(d=>d.saved==="saved")).toBe(false);
  expect(model.draft.description).toBe("new scratch");
});
test("export picker keeps the intended schedule if the active tab changes",()=>{
  const model=panelModel();model.choose("saved");
  let pick,request;model.app.change=(req,cb)=>cb(true,null);
  model.app.chooseFile=(save,cb)=>{pick=cb};model.app.call=(req)=>{request=req};
  model.exportTask("omarsket");model.choose("");pick("/tmp/output");
  expect(request.name).toBe("saved");expect(request.path).toBe("/tmp/output.omscht");
});
