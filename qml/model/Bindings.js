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
  if(binding.chord)return chord(binding.chord)
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
function routingWarnings(value,routing,keymap) {
  if(!value||!routing)return []
  var codes=identities(value,keymap).map(function(v){return v.split("|").pop()}),warnings=[]
  ;(routing.devices||[]).forEach(function(d){Object.keys(d.keys).forEach(function(source){
    var target=d.keys[source]
    if(codes.indexOf("code:"+(Number(source)+8))>=0||target.some(function(code){return codes.indexOf("code:"+(code+8))>=0}))warnings.push(d.path+": "+source+" → "+(target.length?target.join(" + "):"disabled"))
  })})
  return warnings
}
function profileWarnings(profile,bindings,keymap){
  var warnings=[]
  ;(profile.devices||[]).forEach(function(d){Object.keys(d.keys||{}).forEach(function(source){
    var targets=d.keys[source],codes=[Number(source)].concat(targets),mods={29:4,97:4,42:1,54:1,56:8,100:128,125:64,126:64}
    ;(bindings||[]).forEach(function(b){
      var ids=identities(nativeChord(b),keymap).map(function(k){return k.split("|").pop()})
      if(codes.some(function(code){return ids.indexOf("code:"+(code+8))>=0||((mods[code]||0)&b.modmask)!==0}))warnings.push(d.path+": "+source+" → "+(targets.join(" + ")||"disabled")+" affects "+nativeChord(b)+" · "+(b.description||b.dispatcher||"binding"))
    })
  })})
  return warnings
}
function conflicts(bindings,nativeBindings,keymap) {
  return nativeBindings.filter(function(b){return (!b.submap||b.submap_universal===true||b.submap_universal==="true")&&bindings.some(function(d){return overlaps(d.chord,nativeChord(b),keymap)})})
}
// Extract literal chords only; never evaluate Lua or reconstruct native callbacks.
function sourceChords(source) {
  var tokens=[],i=0
  function longEnd(at){var m=source.slice(at).match(/^\[(=*)\[/);if(!m)return -1;var end=source.indexOf("]"+m[1]+"]",at+m[0].length);return end<0?source.length:end+m[1].length+2}
  while(i<source.length){
    var c=source[i]
    if(/\s/.test(c)){i++;continue}
    if(source.slice(i,i+2)==="--"){var end=longEnd(i+2);i=end>=0?end:source.indexOf("\n",i+2);if(i<0)break;continue}
    var end=longEnd(i);if(end>=0){tokens.push({value:"",string:true});i=end;continue}
    if(c==='"'||c==="'"){
      var quote=c,value="",valid=true,closed=false;i++
      while(i<source.length){c=source[i++];if(c===quote){closed=true;break}if(c==="\\"){var e=source[i++];if(e===quote||e==="\\")value+=e;else{valid=false;value+=e||""}}else value+=c}
      tokens.push({value:valid&&closed?value:"",string:true});continue
    }
    var ident=source.slice(i).match(/^[A-Za-z_][A-Za-z_0-9]*/)
    if(ident){tokens.push({value:ident[0]});i+=ident[0].length}else{tokens.push({value:c});i++}
  }
  var out=[]
  for(var n=0;n+5<tokens.length;n++){
    var a=tokens.slice(n,n+6)
    if(a[0].string||["o","hl"].indexOf(a[0].value)<0||a[1].value!=="."||["bind","rebind","bind_hold"].indexOf(a[2].value)<0||a[3].value!=="("||!a[4].string||a[5].value!==","||!a[4].value)continue
    if(out.indexOf(a[4].value)<0)out.push(a[4].value)
  }
  return out
}
function draftConflicts(keys,source,keymap){return sourceChords(source).filter(function(k){return overlaps(keys,k,keymap)}).map(function(k){return {chord:k,description:"Binding in current draft"}})}
function replacement(keys,kind,target,description,collisions,source){
  var known=sourceChords(source),remove=[]
  collisions.forEach(function(b){
    var matches=known.filter(function(k){return chord(k).toLowerCase()===nativeChord(b).toLowerCase()})
    if(!matches.length)throw Error("Exact native chord unavailable for "+(b.description||nativeChord(b))+"; edit its native configuration instead")
    matches.forEach(function(k){if(remove.indexOf(k)<0)remove.push(k)})
  })
  // Include unsaved literal bindings too, so two appends cannot silently collide.
  return remove.map(function(k){return "hl.unbind("+lua(k)+")"}).concat([line(keys,kind,target,description)]).join("\n")
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
