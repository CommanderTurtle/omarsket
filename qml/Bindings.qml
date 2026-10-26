import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui
import "model/Bindings.js" as Bindings
import "model/Tasks.js" as Tasks
import "model/Keyboard.js" as Keyboard

Column {
  id:root
  required property var service
  required property string bindingLayer
  property string configFile:"bindings.lua"
  property var nativeState: ({})
  property string draft:""
  property bool editing:false
  property bool defaults:false
  property bool showSource:false
  property bool loading:false
  property int generation:0
  property string error:""
  property string notice:""
  property bool showMouse:false
  property var modifiers:[]
  property string keyId:""
  property string chord:""
  property string kind:bindingLayer==="macro"?"schedule":"command"
  property string target:""
  property string description:""
  property string search:""
  property string bindingView:"Assignments"
  property bool recording:false
  readonly property var actionKinds:{var choices=bindingLayer==="macro"?["schedule","hotkey","profile"]:["command","text"];return choices.indexOf(kind)<0?[kind].concat(choices):choices}
  property var pendingReplacement:null
  property bool alive:true
  property bool sourceChanged:false
  readonly property var boardKeys:Keyboard.layout().concat(showMouse?Keyboard.mouse():[])
  readonly property var presentation:Bindings.presentation(boardKeys,service.nativeBindings,nativeState.defaults||"",service.nativeKeymap,modifiers,bindingLayer)
  readonly property var keyHints:presentation.hints
  readonly property string layerTitle:bindingLayer==="macro"?"Macros · schedules and Macrohelp":bindingLayer==="software"?"Software · applications, commands and text":"Omarchy · desktop actions and native defaults"
  readonly property var bindingRows:Bindings.rows(service.nativeBindings,nativeState.defaults||"",service.nativeKeymap,bindingLayer)
  readonly property var visibleRows:{
    var rows=bindingView==="Unassigned keys"?boardKeys.filter(function(k){return presentation.states[k.id]==="available"}).map(function(k){return {chord:Bindings.physical(k,root.modifiers),description:Keyboard.label(k,root.service.nativeKeymap),action:"Unassigned for these modifiers",active:false}}):bindingRows.filter(function(row){return root.bindingView==="Assignments"?row.active:!row.active})
    return rows.filter(function(row){return (!root.keyId||Bindings.overlaps(row.chord,root.chord,root.service.nativeKeymap))&&(!root.search||(row.chord+" "+row.description+" "+row.action).toLowerCase().indexOf(root.search.toLowerCase())>=0)})
  }
  readonly property var collisions:chord?Bindings.conflicts([{chord:chord}],service.nativeBindings,service.nativeKeymap).concat(Bindings.draftConflicts(chord,draft,service.nativeKeymap)):[]
  readonly property var routingWarnings:Bindings.routingWarnings(chord,service.nativeRouting,service.nativeKeymap)
  spacing:Style.space(8)
  function load(){
    var token=++generation;loading=true;editing=false;defaults=false;recording=false;pendingReplacement=null;sourceChanged=false
    service.inputConfigCommand({action:"snapshot",file:configFile},function(v,e){if(!root.alive||token!==root.generation)return;root.loading=false;root.error=e||"";if(!e){root.nativeState=v;root.draft=v.text}})
  }
  function find(text){
    var at=draft.indexOf(text);defaults=at<0
    if(at<0)at=String(nativeState.defaults||"").indexOf(text)
    if(at<0){error="Literal source location unavailable; inspect the native configuration";return}
    showSource=true;Qt.callLater(function(){editor.input.forceActiveFocus();editor.input.select(at,at+text.length)})
  }
  function begin(){if(nativeState.privileged){service.inputConfigCommand({action:"admin-edit",file:configFile},function(v,e){root.error=e||""});return}defaults=false;editing=true}
  function selectKey(key){keyId=key.id;chord=key.code===null?"":Bindings.physical(key,modifiers)}
  function beginAssignment(key){
    if(loading||nativeState.text===undefined)return false
    if(configFile!=="bindings.lua"){error="Switch to bindings.lua before assigning a key; the current file is unchanged";return false}
    if(key)selectKey(key)
    if(!chord){error="This key has no emitted physical identity";return false}
    begin()
    if(!editing)error="This configuration requires the native administrator editor"
    return editing
  }
  function chooseSchedule(){
    notice=""
    if(!beginAssignment(null))return
    var token=generation,selectedChord=chord
    service.chooseFile(false,function(path){
      if(!root.alive||token!==root.generation||!root.editing)return
      if(root.chord!==selectedChord){root.error="The selected chord changed; choose the schedule again";return}
      if(!/\.omscht$/i.test(path)){root.error="Choose an .omscht schedule";return}
      var name=Tasks.uniqueName(root.service.tasks.map(function(t){return t.id}),Tasks.filename(path))
      root.loading=true
      root.service.change({method:"import",path:path,name:name},function(v,e){
        if(!root.alive||token!==root.generation)return
        root.loading=false;root.error=e||""
        if(!e){root.kind="schedule";root.target=name;root.description=name;root.notice="Imported "+name+". Review and append the binding, then Save; nothing has run."}
      })
    })
  }
  function save(){
    loading=true;var token=++generation
    service.inputConfigCommand({action:"save",file:configFile,expected:nativeState.text,text:draft},function(v,e){if(!root.alive||token!==root.generation)return;root.loading=false;root.error=e||"";if(!e){root.nativeState=v;root.draft=v.text;root.editing=false;root.sourceChanged=false}})
  }
  function appendBinding(){
    try {
      var text=Bindings.replacement(chord,kind,target,description,collisions,draft+"\n"+(nativeState.defaults||""))
      error=""
      if(collisions.length)pendingReplacement={draft:draft,text:text,request:JSON.stringify([chord,kind,target,description]),signature:JSON.stringify(collisions),labels:collisions.map(function(b){return Bindings.nativeChord(b)+" · "+(b.description||b.dispatcher)}).join("\n")}
      else draft+="\n"+text+"\n"
    }catch(e){error=e.message}
  }
  function confirmReplacement(){
    var pending=pendingReplacement;pendingReplacement=null
    if(!pending||!editing)return
    if(draft!==pending.draft||JSON.stringify(collisions)!==pending.signature||JSON.stringify([chord,kind,target,description])!==pending.request){error="Bindings changed; review the conflict again";return}
    draft+="\n"+pending.text+"\n"
  }
  onBindingLayerChanged:{if(!editing){kind=bindingLayer==="macro"?"schedule":"command";target=""};pendingReplacement=null}
  onModifiersChanged:{var key=Keyboard.layout().concat(Keyboard.mouse()).find(function(k){return k.id===root.keyId});if(key&&key.code!==null)chord=Bindings.physical(key,modifiers)}
  Connections {
    target:root.service
    function onNativeRevisionChanged(){if(root.editing){root.sourceChanged=true;root.pendingReplacement=null}}
  }
  Component.onCompleted:load()
  Component.onDestruction:{alive=false;generation++}
  Keys.onPressed:function(event){
    if(!recording)return;event.accepted=true
    if(event.key===Qt.Key_Escape){recording=false;return}
    if([Qt.Key_Control,Qt.Key_Shift,Qt.Key_Alt,Qt.Key_Meta,Qt.Key_AltGr].indexOf(event.key)>=0)return
    var key=event.nativeScanCode>0?"code:"+event.nativeScanCode:Bindings.keysym(Tasks.key(event));if(!key)return
    var mods=[];if(event.modifiers&Qt.ControlModifier)mods.push("CTRL");if(event.modifiers&Qt.ShiftModifier)mods.push("SHIFT");if(event.modifiers&Qt.AltModifier)mods.push("ALT");if(event.modifiers&Qt.MetaModifier)mods.push("SUPER")
    keyId="";chord=Bindings.chord(mods.concat([key]).join(" + "));recording=false
  }
  Text {width:parent.width;wrapMode:Text.Wrap;text:root.layerTitle;textFormat:Text.PlainText;color:Commons.Color.accent;font.family:Style.font.family;font.pixelSize:Style.font.body}
  KeyboardMap {width:parent.width;keymap:root.service.nativeKeymap;hints:root.keyHints;states:root.service.nativeRevision>0?root.presentation.states:({});contextEnabled:root.service.nativeRevision>0;selected:root.keyId;showMouse:root.showMouse;onKeySelected:function(key){root.selectKey(key)};onKeyContextRequested:function(key){root.beginAssignment(key)}}
  Text {width:parent.width;wrapMode:Text.Wrap;text:"Global map · accent: assigned here · pale dot: another layer · warning: inactive default. Select modifiers and a key to inspect; right-click to assign. Submap assignments are listed below. No binding changes until Save.";textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  Flow {
    width:parent.width;spacing:Style.space(8)
    Ui.Button {text:root.chord?"Assign "+root.chord:"Select a key";enabled:root.chord!==""&&!root.loading;onClicked:root.beginAssignment(null)}
    Ui.Button {text:"All keys";visible:root.keyId!=="";onClicked:{root.keyId="";root.chord=""}}
    Ui.Button {text:"Choose .omscht…";visible:root.bindingLayer==="macro";enabled:root.chord!==""&&!root.loading;onClicked:root.chooseSchedule()}
  }
  Flow {
    width:parent.width;spacing:Style.space(8)
    Ui.Toggle {label:"Show mouse";checked:root.showMouse;onClicked:root.showMouse=!checked}
    Repeater {model:["CTRL","SHIFT","ALT","SUPER"];Ui.Toggle {required property string modelData;label:modelData;checked:root.modifiers.indexOf(modelData)>=0;onClicked:root.modifiers=checked?root.modifiers.filter(function(v){return v!==modelData}):root.modifiers.concat([modelData])}}
  }
  Flow {
    width:parent.width;spacing:Style.space(8)
    Ui.Dropdown {label:"File";options:["bindings.lua","input.lua"];value:root.configFile;enabled:!root.editing&&!root.loading;onChanged:function(v){root.configFile=v;root.load()}}
    Ui.Button {text:"Edit";iconText:"󰏫";enabled:!root.editing&&!root.loading;onClicked:{root.showSource=true;root.begin()}}
    Ui.Button {text:"Save";iconText:"󰆓";enabled:root.editing&&!root.loading&&!root.pendingReplacement;onClicked:root.save()}
    Ui.Button {text:"Cancel";visible:root.editing;onClicked:root.load()}
    Ui.Button {text:"Reload file";enabled:!root.editing&&!root.loading;onClicked:root.load()}
    Ui.Button {text:"Default";enabled:!root.editing;onClicked:{root.defaults=!root.defaults;root.showSource=true}}
    Ui.Button {text:root.showSource?"Hide source":"Show source";onClicked:root.showSource=!root.showSource}
  }
  Text {width:parent.width;wrapMode:Text.Wrap;text:root.nativeState.path||"";textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  Text {visible:root.sourceChanged; width:parent.width;wrapMode:Text.Wrap;text:"Active hotkeys were refreshed. Your draft is unchanged; Save still checks for external file edits.";textFormat:Text.PlainText;color:Commons.Color.accent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  CodeArea {id:editor;visible:root.showSource;width:parent.width;implicitHeight:Style.space(260);readOnly:!root.editing||root.defaults;text:root.defaults?String(root.nativeState.defaults||""):root.draft;onEdited:function(v){root.draft=v}}
  Column {
    visible:root.editing&&root.configFile==="bindings.lua";enabled:!root.loading;width:parent.width;spacing:Style.space(8)
    Ui.TextField {width:parent.width;placeholderText:"Chord";text:root.chord;onTextEdited:{root.keyId="";root.chord=text}}
    Ui.Button {text:root.recording?"Press a chord (Escape cancels)":"Record chord";onClicked:{root.recording=!root.recording;if(root.recording)root.forceActiveFocus()}}
    Ui.Dropdown {label:"Action";options:root.actionKinds;value:root.kind;onChanged:function(v){root.kind=v;root.target=""}}
    Ui.Dropdown {visible:root.kind==="schedule";label:"Schedule";options:root.service.tasks.map(function(t){return t.id});value:root.target;onChanged:function(v){root.target=v}}
    Ui.Dropdown {visible:root.kind==="hotkey";label:"Macrohelp 0–9";options:["0","1","2","3","4","5","6","7","8","9"];value:root.target;onChanged:function(v){root.target=v}}
    Ui.TextField {visible:["profile","command","text"].indexOf(root.kind)>=0;width:parent.width;placeholderText:root.kind==="profile"?"Macrohelp preset name":root.kind==="text"?"Text / character":"Command";text:root.target;onTextEdited:root.target=text}
    Ui.TextField {width:parent.width;placeholderText:"Description";text:root.description;onTextEdited:root.description=text}
    Text {visible:root.collisions.length>0;width:parent.width;wrapMode:Text.Wrap;text:root.collisions.map(function(b){return Bindings.nativeChord(b)+" · "+(b.description||b.dispatcher)}).join("\n");textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
    Text {visible:root.routingWarnings.length>0;width:parent.width;wrapMode:Text.Wrap;text:root.routingWarnings.join("\n");textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
    Ui.Button {text:root.collisions.length?"Review replacement…":"Append binding";enabled:root.chord.trim()!==""&&root.target.trim()!==""&&!root.pendingReplacement;onClicked:root.appendBinding()}
    Column {
      width:parent.width;visible:!!root.pendingReplacement;spacing:Style.space(6)
      Text {width:parent.width;wrapMode:Text.Wrap;text:root.pendingReplacement?"Already bound:\n"+root.pendingReplacement.labels+"\nReplace these bindings? This unbinds their exact native chords, including press/release and submap variants. Changes apply only on Save.":"";textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.body}
      Row {spacing:Style.space(8)
        Ui.Button {text:"Replace bindings";onClicked:root.confirmReplacement()}
        Ui.Button {text:"Keep existing";onClicked:root.pendingReplacement=null}
      }
    }
  }
  Text {visible:root.error!=="";width:parent.width;wrapMode:Text.Wrap;text:root.error;textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  Text {visible:root.notice!=="";width:parent.width;wrapMode:Text.Wrap;text:root.notice;textFormat:Text.PlainText;color:Commons.Color.accent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  Ui.Dropdown {label:"View";options:["Assignments","Inactive defaults","Unassigned keys"];value:root.bindingView;onChanged:function(v){root.bindingView=v}}
  Ui.TextField {width:parent.width;placeholderText:"Find a chord, description or action";text:root.search;onTextEdited:root.search=text}
  Text {width:parent.width;wrapMode:Text.Wrap;text:root.visibleRows.length+" · "+root.bindingView+(root.keyId?" · "+root.chord:"");textFormat:Text.PlainText;color:Commons.Color.accent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  Repeater {
    model:root.service.nativeRevision>0?root.visibleRows:[]
    Column {
      required property var modelData
      width:parent.width;spacing:Style.space(4)
      Text {width:parent.width;wrapMode:Text.Wrap;text:parent.modelData.chord+" · "+parent.modelData.description+(parent.modelData.scope&&parent.modelData.scope!=="global"?" · submap: "+parent.modelData.scope:"");textFormat:Text.PlainText;color:parent.modelData.active?Commons.Color.accent:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.body}
      Text {width:parent.width;wrapMode:Text.Wrap;text:parent.modelData.action;textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.caption}
      Row {spacing:Style.space(6)
        Ui.Button {text:"Find in source";enabled:root.bindingView!=="Unassigned keys";onClicked:root.find(parent.parent.modelData.description)}
        Ui.Button {text:"Use chord";onClicked:{root.keyId="";root.chord=parent.parent.modelData.chord;root.beginAssignment(null)}}
      }
    }
  }
}
