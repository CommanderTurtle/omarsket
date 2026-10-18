// Full-size Dell-style ANSI geometry, in key units. Fn legends are physical labels.
function layout() {
  var keys=[]
  function put(code,label,x,y,w,h,fn){keys.push({id:String(code),code:typeof code==="number"?code:null,label:label,x:x,y:y,w:w||1,h:h||1,fn:fn||""})}
  put(1,"Esc",0,0,1,1,"Lock")
  var functions=["Sleep","","","","","","","","Search","Previous","Play","Next"]
  for(var i=0;i<12;i++)put(i<10?59+i:87+i-10,"F"+(i+1),1.5+i+(i>=4?.35:0)+(i>=8?.35:0),0,1,1,functions[i])
  put(113,"Mute",15.3,0);put(114,"Vol−",16.3,0);put(115,"Vol+",17.3,0)
  var rows=[
    [[41,"`"],[2,"1"],[3,"2"],[4,"3"],[5,"4"],[6,"5"],[7,"6"],[8,"7"],[9,"8"],[10,"9"],[11,"0"],[12,"−"],[13,"="],[14,"Backspace",2]],
    [[15,"Tab",1.5],[16,"Q"],[17,"W"],[18,"E"],[19,"R"],[20,"T"],[21,"Y"],[22,"U"],[23,"I"],[24,"O"],[25,"P"],[26,"["],[27,"]"],[43,"\\",1.5]],
    [[58,"Caps",1.75],[30,"A"],[31,"S"],[32,"D"],[33,"F"],[34,"G"],[35,"H"],[36,"J"],[37,"K"],[38,"L"],[39,";"],[40,"'"],[28,"Enter",2.25]],
    [[42,"Shift",2.25],[44,"Z"],[45,"X"],[46,"C"],[47,"V"],[48,"B"],[49,"N"],[50,"M"],[51,","],[52,"."],[53,"/"],[54,"Shift",2.75]],
    [[29,"Ctrl",1.25],[464,"Fn"],[125,"Super",1.25],[56,"Alt",1.25],[57,"Space",5.25],[100,"AltGr",1.25],[127,"Menu"],["copilot","Copilot",1.5],[97,"Ctrl",1.25]]
  ]
  rows.forEach(function(row,y){var x=0;row.forEach(function(k){put(k[0],k[1],x,y+1.4,k[2]||1,1,k[0]===127?"Fn":"");x+=k[2]||1})})
  put(110,"Insert",15.3,1.4,1,1,"Print");put(102,"Home",16.3,1.4,1,1,"Scroll");put(104,"PgUp",17.3,1.4,1,1,"Pause")
  put(111,"Delete",15.3,2.4);put(107,"End",16.3,2.4);put(109,"PgDn",17.3,2.4)
  put(103,"↑",16.3,4.4);put(105,"←",15.3,5.4);put(108,"↓",16.3,5.4);put(106,"→",17.3,5.4)
  ;[[69,"Num"],[98,"/"],[55,"*"],[74,"−"]].forEach(function(k,i){put(k[0],k[1],18.8+i,1.4)})
  ;[[71,"7"],[72,"8"],[73,"9"],[75,"4"],[76,"5"],[77,"6"],[79,"1"],[80,"2"],[81,"3"]].forEach(function(k,i){put(k[0],k[1],18.8+i%3,2.4+Math.floor(i/3))})
  put(78,"+",21.8,2.4,1,2);put(96,"Enter",21.8,4.4,1,2);put(82,"0",18.8,5.4,2);put(83,".",20.8,5.4)
  return keys
}
function mouse() { return [{id:"272",code:272,label:"Left",x:0,y:0,w:2,h:2},{id:"274",code:274,label:"Middle",x:2,y:0,w:1.5,h:2},{id:"273",code:273,label:"Right",x:3.5,y:0,w:2,h:2},{id:"275",code:275,label:"Side 1",x:0,y:2,w:2,h:1},{id:"276",code:276,label:"Side 2",x:0,y:3,w:2,h:1}] }
function normalized(session) { return session&&session.values?session.values:{} }
function maximum(values) { return Math.max.apply(Math,[0.000001].concat(Object.keys(values).map(function(k){return Number(values[k])||0}))) }
