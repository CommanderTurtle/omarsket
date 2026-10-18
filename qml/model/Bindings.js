function chord(value) {
  var mods=[],keys=[]
  String(value).split("+").forEach(function(part){
    var token=part.trim(),upper=token.toUpperCase(),mod=({CONTROL:"CTRL",CTRL:"CTRL",SHIFT:"SHIFT",ALT:"ALT",WIN:"SUPER",META:"SUPER",SUPER:"SUPER",CAPS:"CAPS",MOD2:"MOD2",MOD3:"MOD3",MOD5:"MOD5"})[upper]
    if(mod){if(mods.indexOf(mod)<0)mods.push(mod)}else if(token)keys.push(token)
  })
  return mods.sort().concat(keys).join(" + ")
}
function keysym(key) { return ({"/":"slash","\\":"backslash",";":"semicolon","'":"apostrophe","`":"grave","[":"bracketleft","]":"bracketright",",":"comma",".":"period","-":"minus","=":"equal","+":"plus"})[key]||key }
function nativeChord(binding) {
  var mods=[];[[1,"SHIFT"],[2,"CAPS"],[4,"CTRL"],[8,"ALT"],[16,"MOD2"],[32,"MOD3"],[64,"SUPER"],[128,"MOD5"]].forEach(function(m){if(binding.modmask&m[0])mods.push(m[1])})
  var key=binding.key||((binding.keycode>0)?"code:"+binding.keycode:"")
  return key?chord(mods.concat([key]).join("+")):""
}
function identities(value,keymap) {
  var parts=chord(value).split(" + "),key=parts.pop(),prefix=parts.join("+")+"|"
  var codes=/^code:(\d+)$/.exec(key),mouse=/^mouse:\d+$/.test(key)
  if(codes)return [prefix+"code:"+Number(codes[1])]
  if(mouse)return [prefix+key]
  var aliases=keymap&&keymap.codes&&keymap.codes[key.toLowerCase()]||[]
  return [prefix+key.toLowerCase()].concat(aliases.map(function(code){return prefix+"code:"+code}))
}
function overlaps(a,b,keymap) {var keys=identities(a,keymap);return identities(b,keymap).some(function(key){return keys.indexOf(key)>=0})}
function conflicts(bindings,nativeBindings,keymap) {
  return nativeBindings.filter(function(b){return (!b.submap||b.submap_universal===true||b.submap_universal==="true")&&bindings.some(function(d){return overlaps(d.chord,nativeChord(b),keymap)})})
}
function physical(key,mods) { return chord((mods||[]).concat([key.code===null?"":key.code>=272&&key.code<=276?"mouse:"+key.code:"code:"+(key.code+8)]).filter(Boolean).join(" + ")) }
function lua(text){return '"'+String(text).replace(/[\\"\x00-\x1f]/g,function(c){return c==='\\'?'\\\\':c==='"'?'\\"':'\\'+String(c.charCodeAt(0)).padStart(3,"0")})+'"'}
function shell(text){return "'"+String(text).replace(/'/g,"'\\''")+"'"}
function line(keys,kind,target,description){
  if(/[\x00-\x1f]/.test(keys)||!keys.trim())throw Error("Choose a chord")
  var command=String(target)
  if(kind==="schedule")command="omarchy-shell -q io.github.commanderturtle.omarsket run "+shell(target)
  else if(kind==="profile")command="omarchy-shell -q io.github.commanderturtle.macrohelp profile "+shell(target)
  else if(kind==="hotkey"){if(!/^[0-9]$/.test(target))throw Error("Choose 0–9");command="omarchy-shell -q io.github.commanderturtle.macrohelp hotkey "+target}
  else if(kind==="text")command="printf %s "+shell(target)+" | wtype -"
  else if(kind!=="command")throw Error("Choose an action")
  return "o.bind("+lua(keys.trim())+", "+lua(description||target)+", "+lua(command)+")"
}
