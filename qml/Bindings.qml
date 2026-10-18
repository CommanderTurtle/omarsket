import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui
import "model/Bindings.js" as Bindings
import "model/Tasks.js" as Tasks

Column {
  id:root
  required property var service
  required property string layer
  property string configFile:"bindings.lua"
  property var nativeState: ({})
  property string draft:""
  property bool editing:false
  property bool defaults:false
  property bool loading:false
  property int generation:0
  property string error:""
  property bool showMouse:false
  property var modifiers:[]
  property string keyId:""
  property string chord:""
  property string kind:layer==="macro"?"schedule":"command"
  property string target:""
  property string description:""
  property string search:""
  property bool recording:false
  property var pendingReplacement:null
  property bool alive:true
  readonly property var collisions:chord?Bindings.conflicts([{chord:chord}],service.nativeBindings,service.nativeKeymap).concat(Bindings.draftConflicts(chord,draft,service.nativeKeymap)):[]
  readonly property var routingWarnings:Bindings.routingWarnings(chord,service.nativeRouting,service.nativeKeymap)
  spacing:Style.space(8)
  function load(){
    var token=++generation;loading=true;editing=false;defaults=false;recording=false;pendingReplacement=null
    service.inputConfigCommand({action:"snapshot",file:configFile},function(v,e){if(!root.alive||token!==root.generation)return;root.loading=false;root.error=e||"";if(!e){root.nativeState=v;root.draft=v.text}})
  }
  function find(text){var at=draft.indexOf(text);if(at>=0){defaults=false;editor.input.forceActiveFocus();editor.input.select(at,at+text.length)}else error="Defined in Omarchy's defaults; edit the native override below."}
  function begin(){if(nativeState.privileged){service.inputConfigCommand({action:"admin-edit",file:configFile},function(v,e){root.error=e||""});return}defaults=false;editing=true}
  function save(){
    loading=true;var token=++generation
    service.inputConfigCommand({action:"save",file:configFile,expected:nativeState.text,text:draft},function(v,e){if(!root.alive||token!==root.generation)return;root.loading=false;root.error=e||"";if(!e){root.nativeState=v;root.draft=v.text;root.editing=false}})
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
  onLayerChanged:{kind=layer==="macro"?"schedule":"command";target="";if(service)load()}
  Component.onCompleted:load()
  Component.onDestruction:{alive=false;generation++}
  Keys.onPressed:function(event){
    if(!recording)return;event.accepted=true
    if(event.key===Qt.Key_Escape){recording=false;return}
    if([Qt.Key_Control,Qt.Key_Shift,Qt.Key_Alt,Qt.Key_Meta,Qt.Key_AltGr].indexOf(event.key)>=0)return
    var key=event.nativeScanCode>0?"code:"+event.nativeScanCode:Bindings.keysym(Tasks.key(event));if(!key)return
    var mods=[];if(event.modifiers&Qt.ControlModifier)mods.push("CTRL");if(event.modifiers&Qt.ShiftModifier)mods.push("SHIFT");if(event.modifiers&Qt.AltModifier)mods.push("ALT");if(event.modifiers&Qt.MetaModifier)mods.push("SUPER")
    chord=Bindings.chord(mods.concat([key]).join(" + "));recording=false
  }
  KeyboardMap {width:parent.width;selected:root.keyId;showMouse:root.showMouse;onKeySelected:function(key){root.keyId=key.id;if(key.code!==null)root.chord=Bindings.physical(key,root.modifiers)}}
  Flow {
    width:parent.width;spacing:Style.space(8)
    Ui.Toggle {label:"Show mouse";checked:root.showMouse;onClicked:root.showMouse=!checked}
    Repeater {model:["CTRL","SHIFT","ALT","SUPER"];Ui.Toggle {required property string modelData;label:modelData;checked:root.modifiers.indexOf(modelData)>=0;onClicked:root.modifiers=checked?root.modifiers.filter(function(v){return v!==modelData}):root.modifiers.concat([modelData])}}
  }
  Flow {
    width:parent.width;spacing:Style.space(8)
    Ui.Dropdown {label:"File";options:["bindings.lua","input.lua"];value:root.configFile;enabled:!root.editing&&!root.loading;onChanged:function(v){root.configFile=v;root.load()}}
    Ui.Button {text:"Edit";iconText:"󰏫";enabled:!root.editing&&!root.loading;onClicked:root.begin()}
    Ui.Button {text:"Save";iconText:"󰆓";enabled:root.editing&&!root.loading&&!root.pendingReplacement;onClicked:root.save()}
    Ui.Button {text:"Cancel";visible:root.editing;onClicked:{root.pendingReplacement=null;root.draft=root.nativeState.text;root.editing=false}}
    Ui.Button {text:"Reload";enabled:!root.editing&&!root.loading;onClicked:root.load()}
    Ui.Button {text:"Default";enabled:!root.editing;onClicked:root.defaults=!root.defaults}
  }
  Text {width:parent.width;wrapMode:Text.Wrap;text:root.nativeState.path||"";textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  CodeArea {id:editor;width:parent.width;implicitHeight:Style.space(260);readOnly:!root.editing||root.defaults;text:root.defaults?String(root.nativeState.defaults||""):root.draft;onEdited:function(v){root.draft=v}}
  Column {
    visible:root.editing&&root.configFile==="bindings.lua";enabled:!root.loading;width:parent.width;spacing:Style.space(8)
    Ui.TextField {width:parent.width;placeholderText:"Chord";text:root.chord;onTextEdited:root.chord=text}
    Ui.Button {text:root.recording?"Press a chord (Escape cancels)":"Record chord";onClicked:{root.recording=!root.recording;if(root.recording)root.forceActiveFocus()}}
    Ui.Dropdown {label:"Action";options:root.layer==="macro"?["schedule","hotkey","profile"]:["command","text"];value:root.kind;onChanged:function(v){root.kind=v;root.target=""}}
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
  Ui.TextField {width:parent.width;placeholderText:"Find active bindings";text:root.search;onTextEdited:root.search=text}
  Repeater {
    model:root.service.nativeBindings.filter(function(b){return !root.search||(Bindings.nativeChord(b)+" "+b.description).toLowerCase().indexOf(root.search.toLowerCase())>=0})
    Row {
      required property var modelData
      width:parent.width;spacing:Style.space(6)
      Ui.Button {text:"Find";onClicked:root.find(parent.modelData.description||Bindings.nativeChord(parent.modelData))}
      Text {width:parent.width-Style.space(65);wrapMode:Text.Wrap;text:Bindings.nativeChord(parent.modelData)+" · "+(parent.modelData.description||parent.modelData.dispatcher);textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.caption}
    }
  }
}
