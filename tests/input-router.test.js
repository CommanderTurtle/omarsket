import {test,expect} from "bun:test"
import {readFileSync} from "node:fs"
const read=path=>readFileSync(new URL("../"+path,import.meta.url),"utf8")
test("physical input is opt-in and cannot start from an ordinary snapshot",()=>{
  const q=read("qml/InputRouter.qml"),source=read("src/remap.rs")
  expect(q).toContain("property bool optIn:false")
  expect(q).toContain('Component.onCompleted:load(true)')
  expect(q).not.toMatch(/Component.onCompleted:.*action:"start"/)
  expect(source).toContain('"snapshot" => profiles(root)')
  expect(q).toContain('iconText:"󰏫"');expect(q).toContain('iconText:"󰆓"')
  expect(q).toContain('root.selected!=="ts_uinput"')
})
function router(){
  const model={alive:true,optIn:false,settingTouched:false,loading:false,error:"",book:{profiles:[]},profile:{name:"",devices:[]},previous:null,selected:"ts_uinput",deviceIndex:0,pending:[],service:{}}
  model.service.call=(request,callback)=>model.pending.push({request,callback})
  const functions=[...read("qml/InputRouter.qml").matchAll(/^  function (\w+)\(([^)]*)\)\{(.*)\}$/gm)]
  for(const [,name,args,body] of functions)model[name]=new Function("root","Tasks",`with(root){return function(${args}){${body}}}`)(model,{copy:v=>JSON.parse(JSON.stringify(v))})
  return model
}
test("late snapshots cannot re-enable an explicitly disabled layer or touch an unloaded view",()=>{
  const m=router();m.load(true);m.settingTouched=true;m.optIn=false
  m.pending.shift().callback({active:{profile:"Desk"},profiles:[]},null)
  expect(m.optIn).toBe(false);expect(m.loading).toBe(false)
  m.load();m.alive=false;m.pending.shift().callback(null,"late")
  expect(m.error).toBe("")
})
test("save snapshots its target, rejects double saves and retains failed drafts",()=>{
  const m=router();m.profile={name:"Desk",devices:[{path:"keyboard",keys:{33:[34]}}]};m.save();m.save()
  expect(m.pending).toHaveLength(1);expect(m.pending[0].request.profile.name).toBe("Desk")
  m.pending.shift().callback(null,"outside edit");expect(m.profile.name).toBe("Desk");expect(m.loading).toBe(false);expect(m.error).toBe("outside edit")
})
test("authorization uses an installed root-owned helper, never user scripts or device permissions",()=>{
  const s=read("src/remap_linux.rs"),build=read("scripts/build.sh")
  expect(s).toContain('"sudo", "--", HELPER, "--profile"')
  expect(s).toContain('m.uid() == 0')
  expect(s).toContain("libc::setresuid(who, who, who)")
  expect(s).toContain("libc::PR_SET_DUMPABLE")
  expect(s).not.toContain('Command::new("sh")')
  expect(s).not.toContain('Command::new("python')
  expect(build).not.toMatch(/sudo|usermod|chmod.*dev\/|systemctl.*enable/)
})
test("routing covers kernel resync, fair draining, device release and controller disappearance",()=>{
  const s=read("src/remap_linux.rs")
  for(const value of ["libevdev_next_event","libevdev_uinput_create_from_device","libevdev_grab","libc::POLLHUP","self.keys.release()","self.pending = true","if self.sync { 1 } else { 2 }","/sys/devices/virtual/input"])expect(s).toContain(value)
  expect(s).toContain('value["cancel"] == args[3]')
  expect(s).toContain('if action == "stop" && guard.is_none()')
  expect(s).not.toContain('uaccess')
})
test("onboard discovery is explicit, read-only, and never activates a vendor service",()=>{
  const s=read("src/hardware.rs"),q=read("qml/Hardware.qml")
  expect(q).toContain("property bool optIn:false")
  expect(q).not.toContain("Component.onCompleted")
  expect(s).toContain('"--auto-start=no"')
  expect(s).toContain('"--allow-interactive-authorization=no"')
  expect(s).toContain('"NameHasOwner"')
  expect(s).not.toContain('"SetActive"');expect(s).not.toContain('"Commit"')
  expect(s).toContain('"onboardWrite":false')
})
