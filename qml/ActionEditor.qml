import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui
import "model/Tasks.js" as Tasks

Column {
  id: root
  required property var action
  property var service: null
  property string error: ""
  property string captureMode: ""
  signal changed(var value)
  spacing: Style.space(6)
  function put(key,value) { var next=Tasks.copy(action); next[key]=value; changed(next) }
  function json(key,text) { try { put(key,JSON.parse(text)); error="" } catch(e) {error=e.message} }
  function capture(mode) {captureMode=mode;captureDelay.restart()}
  Timer {
    id:captureDelay;interval:3000
    onTriggered:if(root.service)root.service.call({method:"cursor"},function(point){
      if(root.captureMode==="path"){var path=Tasks.copy(root.action.cursormovsmap || []);path.push([0,100,point.x,point.y]);root.put("cursormovsmap",path)}
      else{var next=Tasks.copy(root.action);next.x=point.x;next.y=point.y;root.changed(next)}
      root.captureMode=""
    })
  }
  Ui.TextField {
    visible: root.action.type === "paste" || root.action.type === "keyssequence" || root.action.type === "cursormovements"
    width: parent.width
    placeholderText: "Saved data name (optional)"
    readonly property string field: root.action.type === "paste" ? "contentId" : root.action.type === "keyssequence" ? "keysSeqId" : "cursorMovsId"
    text: root.action[field] || ""
    onTextEdited: root.put(field,text)
  }
  CodeArea {
    visible: root.action.type === "paste" || root.action.type === "type"
    width: parent.width
    text: root.action.type === "paste" ? (root.action.content || "") : (root.action.text || "")
    onEdited: function(value) {root.put(root.action.type === "paste" ? "content" : "text",value)}
  }
  Ui.TextField {
    visible: root.action.type === "wait"; width:parent.width
    placeholderText: "Seconds"; text: String(root.action.duration === undefined ? 1 : root.action.duration)
    onTextEdited: if (text!=="" && isFinite(Number(text))) root.put("duration",Number(text))
  }
  Row {
    visible: ["paste","keyssequence","cursormovements","type"].indexOf(root.action.type) >= 0
    spacing:Style.space(8)
    Ui.NumberField {
      label: root.action.type === "type" ? "Key interval (ms)" : "Repeat"
      from:root.action.type === "type" ? 0 : 1; to:100000
      value:root.action.type === "type" ? (root.action.interval_ms || 0) : (root.action.loop || 1)
      onModified:function(value){root.put(root.action.type === "type" ? "interval_ms" : "loop",value)}
    }
  }
  CodeArea {
    visible: root.action.type === "keyssequence" || root.action.type === "cursormovements"
    width:parent.width
    text:JSON.stringify(root.action.type === "keyssequence" ? root.action.keysmap : root.action.cursormovsmap,null,2)
    hint:root.action.type === "keyssequence" ? '{"start ms":[hold ms,["keys"]]}' : "[[delay ms,move ms,x,y]]"
    onEdited:function(value){root.json(root.action.type === "keyssequence" ? "keysmap" : "cursormovsmap",value)}
  }
  Ui.TextField {
    visible:root.action.type === "cursormovements"; width:parent.width
    placeholderText:"Held keys/buttons, separated by commas"
    text:(root.action.optionalkeysstroke || []).join(", ")
    onTextEdited:root.put("optionalkeysstroke",text.split(",").map(function(v){return v.trim()}).filter(function(v){return v!==""}))
  }
  Row {
    visible:root.action.type === "cursormovements"; spacing:Style.space(8)
    Ui.Button {
      text:captureDelay.running?"Move pointer · capturing…":"Add position in 3s"
      onClicked:root.capture("path")
    }
  }
  Rectangle {
    id: recorder
    visible:root.action.type === "keyssequence"; width:parent.width; height:Style.space(36)
    color: recording ? Qt.rgba(Commons.Color.accent.r,Commons.Color.accent.g,Commons.Color.accent.b,0.14) : "transparent"
    border.color:Commons.Color.foreground; radius:Style.cornerRadius
    property bool recording:false
    property real start:0
    property var down:({})
    property var events:({})
    function finish() {
      var end=Date.now()-start
      for(var key in down){var at=down[key];while(events[String(at)]!==undefined)at++;events[String(at)]=[Math.max(0,end-down[key]),[key]]}
      down=({});recording=false;root.put("keysmap",events)
    }
    activeFocusOnTab:true
    Text {anchors.centerIn:parent;text:recorder.recording ? "Recording · click to finish" : "Record keys in this field";color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.body}
    MouseArea {anchors.fill:parent;onClicked:{if(recorder.recording){recorder.finish()}else{recorder.events=({});recorder.down=({});recorder.start=Date.now();recorder.recording=true;recorder.forceActiveFocus()}}}
    Keys.priority:Keys.BeforeItem
    Keys.onPressed:function(event){if(!recording)return;event.accepted=true;if(event.isAutoRepeat)return;var k=Tasks.key(event);if(down[k]===undefined)down[k]=Date.now()-start}
    Keys.onReleased:function(event){if(!recording)return;event.accepted=true;var k=Tasks.key(event);if(down[k]===undefined)return;var at=down[k];while(events[String(at)]!==undefined)at++;events[String(at)]=[Math.max(0,Date.now()-start-down[k]),[k]];delete down[k]}
    onActiveFocusChanged:if(!activeFocus&&recording)finish()
  }
  Row {
    visible:root.action.type === "move" || root.action.type === "click"
    spacing:Style.space(8)
    Ui.NumberField {label:"X";from:-100000;to:100000;value:root.action.x || 0;onModified:function(v){root.put("x",v)}}
    Ui.NumberField {label:"Y";from:-100000;to:100000;value:root.action.y || 0;onModified:function(v){root.put("y",v)}}
    Ui.Button {text:captureDelay.running?"Move pointer…":"Capture in 3s";onClicked:root.capture("point")}
  }
  Row {
    visible:root.action.type === "click";spacing:Style.space(8)
    Ui.Dropdown {label:"Button";options:["left","middle","right"];value:root.action.button || "left";onChanged:function(v){root.put("button",v)}}
    Ui.NumberField {label:"Clicks";from:1;to:10000;value:root.action.count || 1;onModified:function(v){root.put("count",v)}}
  }
  Ui.NumberField {visible:root.action.type === "move";label:"Movement (ms)";from:0;to:86400000;value:root.action.duration_ms || 0;onModified:function(v){root.put("duration_ms",v)}}
  Row {
    visible:root.action.type === "scroll";spacing:Style.space(8)
    Ui.NumberField {label:"Vertical steps";from:-10000;to:10000;value:root.action.vertical || 0;onModified:function(v){root.put("vertical",v)}}
    Ui.NumberField {label:"Horizontal steps";from:-10000;to:10000;value:root.action.horizontal || 0;onModified:function(v){root.put("horizontal",v)}}
  }
  Ui.TextField {
    visible:root.action.type === "hotkey";width:parent.width
    placeholderText:"ctrl, shift, s";text:(root.action.keys || []).join(", ")
    onTextEdited:root.put("keys",text.split(",").map(function(v){return v.trim()}).filter(function(v){return v!==""}))
  }
  Ui.Dropdown {
    visible:root.action.type === "systemcommand";width:parent.width
    label:"Command";value:root.action.sysCommandType || "openurl"
    options:["openurl","openfile","openfolder","executeprogram","createfile","createfolder","copyfile","deletefile","deletefolder","screenshot","printscreen","changevolume","changeaudiodevice","killprocess","shutdown","restart","logoff","quitselfprogram"]
    onChanged:function(v){root.put("sysCommandType",v)}
  }
  Ui.TextField {visible:root.action.type === "systemcommand";width:parent.width;placeholderText:"Path / URL / program / value / Force or Don't force";text:root.action.sysCommandParam1 || "";onTextEdited:root.put("sysCommandParam1",text)}
  Ui.TextField {visible:root.action.type === "systemcommand";width:parent.width;placeholderText:"Arguments / destination / Auto-rename if exists";text:root.action.sysCommandParam2 || "";onTextEdited:root.put("sysCommandParam2",text)}
  Ui.TextField {visible:root.action.type === "runningothertask";width:parent.width;placeholderText:"Saved task name";text:root.action.otherTaskName || "";onTextEdited:root.put("otherTaskName",text)}
  Row {
    visible:root.action.type === "runningothertask";spacing:Style.space(8)
    Ui.NumberField {label:"Delay (s)";from:0;to:86400;value:root.action.otherTaskDelay || 0;onModified:function(v){root.put("otherTaskDelay",v)}}
    Ui.NumberField {label:"Loops (-1 = forever)";from:-1;to:100000;value:root.action.otherTaskLoops || 1;onModified:function(v){root.put("otherTaskLoops",v)}}
  }
  Text {visible:root.error!=="";text:root.error;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;width:parent.width}
}
