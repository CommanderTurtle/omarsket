import QtQuick
import QtQuick.Controls as QQC
import Quickshell
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui
import "model/Tasks.js" as Tasks

Ui.Panel {
  id: root
  moduleName: "io.github.commanderturtle.omarsket"
  manageIpc: false
  readonly property var app: bar && bar.shell ? bar.shell.serviceFor(moduleName) : null
  readonly property color fg: bar ? bar.foreground : Commons.Color.foreground
  readonly property string face: bar ? bar.fontFamily : Style.font.family
  property string tab: "Tasks"
  property var draft: Tasks.blank("Untitled")
  property bool dirty: false
  property bool deleteArmed: false
  property string pendingName: ""
  property bool switchPending: false
  property string addType: "paste"
  property string pendingRun: ""
  property string dataName: ""
  property string dataKind: "paste"
  property string dataText: ""
  property var report: ({})
  property int oldWidth: 1920
  property int oldHeight: 1080
  property int newWidth: 1920
  property int newHeight: 1080
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight
  function set(key,value) { var next=Tasks.copy(draft); next[key]=value; draft=next; dirty=true }
  function replaceAction(index,value) {var next=Tasks.copy(draft.actions);next[index]=value;set("actions",next)}
  function newName() {var n=1;while(app && app.tasks.some(function(t){return t.id==="Untitled "+n}))n++;return "Untitled "+n}
  function load(name) {
    if (!name) draft=Tasks.blank(newName())
    else {var found=app.tasks.find(function(t){return t.id===name});if(!found)return;draft=Tasks.copy(found)}
    dirty=false;deleteArmed=false;switchPending=false
  }
  function choose(name) {if(dirty){pendingName=name;switchPending=true}else load(name)}
  function save(callback) {
    if(!app)return
    app.change({method:"save",task:Tasks.copy(draft)},function(){root.dirty=false;if(callback)callback()})
  }
  function run() {save(function(){root.pendingRun=root.draft.id;root.close();startRun.restart()})}
  Timer {id:startRun;interval:300;onTriggered:if(root.app)root.app.call({method:"run",name:root.pendingRun})}
  Timer {id:deleteTimer;interval:4000;onTriggered:root.deleteArmed=false}
  onOpenedChanged:if(opened&&app)app.refresh()

  Ui.WidgetButton {
    id:button
    anchors.fill:parent
    bar:root.bar
    text:"󰑮"+(root.app&&root.app.activeCount>0?" "+root.app.activeCount:"")
    tooltipText:"Omarsket · right-click to stop tasks"
    onPressed:function(mouseButton){if(mouseButton===Qt.RightButton&&root.app)root.app.stop();else root.toggle()}
  }
  Ui.KeyboardPanel {
    id:panel
    anchorItem:button;owner:root;bar:root.bar;open:root.opened
    focusTarget:taskName
    contentWidth:panel.fittedContentWidth(Style.space(650))
    contentHeight:panel.fittedContentHeight(content.implicitHeight,Style.space(800))
    QQC.ScrollView {
      anchors.fill:parent
      clip:true
      contentWidth:availableWidth
      Column {
        id:content
        width:parent.width
        spacing:Style.space(10)
        Ui.PanelHero {title:"Omarsket";meta:root.app&&root.app.ready?(root.app.activeCount+" active"):"Runtime not started";foreground:root.fg;fontFamily:root.face}
        Row {
          spacing:Style.space(6)
          Repeater {model:["Tasks","Data","History","Tools"];Ui.Button {required property string modelData;text:modelData;foreground:root.tab===modelData?Commons.Color.accent:root.fg;onClicked:root.tab=modelData}}
          Ui.Button {text:"Stop all";iconText:"󰓛";onClicked:if(root.app)root.app.stop()}
          Ui.Button {text:"×";tooltipText:"Close";onClicked:root.close()}
        }
        Text {visible:root.app&&root.app.error!=="";text:root.app?root.app.error:"";width:parent.width;wrapMode:Text.Wrap;color:Commons.Color.urgent;font.family:root.face;font.pixelSize:Style.font.body}
        Ui.Button {visible:root.app&&!root.app.ready;text:"Start runtime";onClicked:root.app.restart()}
        Ui.PanelSeparator {foreground:root.fg}
        Column {
          width:parent.width;spacing:Style.space(8);visible:root.tab==="Tasks"
          Row {
            spacing:Style.space(8)
            Ui.Dropdown {label:"Saved tasks";width:Style.space(300);value:root.draft.id;options:root.app?root.app.tasks.map(function(t){return t.id}):[];onChanged:function(value){root.choose(value)}}
            Ui.Button {text:"New";onClicked:root.choose("")}
            Ui.Button {text:"Import";onClicked:if(root.app)root.app.chooseFile(false,function(path){var name=path.split("/").pop().replace(/\.(scht|json)$/i,"");root.app.change({method:"import",path:path,name:name},function(){root.app.call({method:"snapshot"},function(s){root.app.apply(s);root.choose(name)})})})}
          }
          Row {
            visible:root.switchPending;spacing:Style.space(8)
            Text {text:"Unsaved changes";color:root.fg;font.family:root.face;font.pixelSize:Style.font.body}
            Ui.Button {text:"Discard";onClicked:root.load(root.pendingName)}
            Ui.Button {text:"Keep editing";onClicked:root.switchPending=false}
          }
          Ui.TextField {id:taskName;width:parent.width;placeholderText:"Task name";text:root.draft.id;onTextEdited:root.set("id",text)}
          Ui.TextField {width:parent.width;placeholderText:"Description";text:root.draft.description;onTextEdited:root.set("description",text)}
          Row {
            spacing:Style.space(12)
            Ui.NumberField {label:"Delay (seconds)";from:0;to:86400;value:root.draft.delay_seconds;onModified:function(value){root.set("delay_seconds",value)}}
            Ui.NumberField {label:"Loops (-1 = forever)";from:-1;to:100000;value:root.draft.loop_times;onModified:function(value){root.set("loop_times",value)}}
            Ui.Button {text:root.dirty?"Save *":"Save";iconText:"󰆓";onClicked:root.save()}
            Ui.Button {text:"Run";iconText:"󰐊";onClicked:root.run()}
          }
          Ui.Toggle {width:parent.width;label:"Run when Omarsket opens";checked:root.draft.auto_start;onClicked:root.set("auto_start",!root.draft.auto_start)}
          Ui.Toggle {width:parent.width;label:"Allow system and file commands";checked:root.draft.allow_system;onClicked:root.set("allow_system",!root.draft.allow_system)}
          Ui.PanelSeparator {foreground:root.fg}
          Repeater {
            model:root.draft.actions.length
            Rectangle {
              id:card
              required property int index
              width:parent.width
              implicitHeight:actionColumn.implicitHeight+Style.space(20)
              radius:Style.cornerRadius
              color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04)
              Column {
                id:actionColumn
                x:Style.space(10);y:Style.space(10);width:parent.width-Style.space(20);spacing:Style.space(8)
                Row {
                  spacing:Style.space(6)
                  Text {text:(card.index+1)+" · "+Tasks.title(root.draft.actions[card.index]);width:Style.space(285);color:root.fg;font.family:root.face;font.pixelSize:Style.font.title;elide:Text.ElideRight}
                  Ui.Button {text:"↑";tooltipText:"Move up";onClicked:root.set("actions",Tasks.move(root.draft.actions,card.index,-1))}
                  Ui.Button {text:"↓";tooltipText:"Move down";onClicked:root.set("actions",Tasks.move(root.draft.actions,card.index,1))}
                  Ui.Button {text:"+";tooltipText:"Duplicate action";onClicked:{var a=Tasks.copy(root.draft.actions);a.splice(card.index+1,0,Tasks.copy(a[card.index]));root.set("actions",a)}}
                  Ui.Button {text:"×";tooltipText:"Remove action";onClicked:{var a=Tasks.copy(root.draft.actions);a.splice(card.index,1);root.set("actions",a)}}
                }
                ActionEditor {width:parent.width;action:root.draft.actions[card.index] || Tasks.action("wait");service:root.app;onChanged:function(value){root.replaceAction(card.index,value)}}
              }
            }
          }
          Row {
            spacing:Style.space(8)
            Ui.Dropdown {label:"Add action";value:root.addType;options:["paste","wait","keyssequence","cursormovements","move","click","scroll","hotkey","type","systemcommand","runningothertask"];onChanged:function(value){root.addType=value}}
            Ui.Button {text:"Add";onClicked:root.set("actions",root.draft.actions.concat([Tasks.action(root.addType)]))}
            Ui.Button {text:"Duplicate list";onClicked:{try{root.set("actions",Tasks.duplicate(root.draft.actions,1))}catch(e){if(root.app)root.app.error=e.message}}}
          }
          Row {
            spacing:Style.space(8)
            Ui.Button {text:"Export";onClicked:root.save(function(){root.app.chooseFile(true,function(path){root.app.call({method:"export",name:root.draft.id,path:path,format:"omarsket"})})})}
            Ui.Button {text:"Export .scht";onClicked:root.save(function(){root.app.chooseFile(true,function(path){root.app.call({method:"export",name:root.draft.id,path:path,format:"tasket"})})})}
            Ui.Button {text:"Launcher";onClicked:root.save(function(){root.app.chooseFile(true,function(path){root.app.call({method:"shortcut",name:root.draft.id,path:path})})})}
            Ui.Button {text:root.deleteArmed?"Confirm delete":"Delete";onClicked:{if(!root.deleteArmed){root.deleteArmed=true;deleteTimer.restart()}else root.app.change({method:"delete",name:root.draft.id},function(){root.load("")})}}
          }
        }
        Column {
          width:parent.width;spacing:Style.space(8);visible:root.tab==="Data"
          Ui.Dropdown {label:"Saved data";options:root.app?Object.keys(root.app.data):[];value:root.dataName;onChanged:function(value){root.dataName=value;var item=root.app.data[value];root.dataKind=item.kind;root.dataText=item.kind==="paste"?item.value:JSON.stringify(item.value,null,2)}}
          Ui.TextField {width:parent.width;placeholderText:"Data name";text:root.dataName;onTextEdited:root.dataName=text}
          Ui.Dropdown {label:"Kind";options:["paste","keyssequence","cursormovements"];value:root.dataKind;onChanged:function(value){root.dataKind=value;root.dataText=value==="paste"?"":value==="keyssequence"?"{}":"[]"}}
          CodeArea {width:parent.width;text:root.dataText;onEdited:function(value){root.dataText=value}}
          Row {
            spacing:Style.space(8)
            Ui.Button {text:"Save data";onClicked:{try{root.app.change({method:"save-data",name:root.dataName,item:{kind:root.dataKind,value:root.dataKind==="paste"?root.dataText:JSON.parse(root.dataText)}})}catch(e){root.app.error=e.message}}}
            Ui.Button {text:"Delete data";onClicked:root.app.change({method:"delete-data",name:root.dataName})}
            Ui.Button {text:"Import";onClicked:root.app.chooseFile(false,function(path){root.app.change({method:"import-data",path:path})})}
            Ui.Button {text:"Export";onClicked:root.app.chooseFile(true,function(path){root.app.call({method:"export-data",path:path})})}
          }
        }
        Column {
          width:parent.width;spacing:Style.space(8);visible:root.tab==="History"
          Repeater {
            model:root.app?root.app.jobs.slice().reverse():[]
            Column {
              required property var modelData
              width:parent.width;spacing:Style.space(4)
              Row {
                spacing:Style.space(8)
                Text {text:"#"+modelData.number+" · "+modelData.task+" · "+modelData.state+" · "+modelData.completed_loops+" loops";width:Style.space(470);color:root.fg;font.family:root.face;font.pixelSize:Style.font.body;elide:Text.ElideRight}
                Ui.Button {visible:modelData.state==="scheduled"||modelData.state==="running";text:"Stop";onClicked:root.app.call({method:"stop",number:modelData.number})}
              }
              Text {visible:modelData.error!=="";text:modelData.error;width:parent.width;wrapMode:Text.Wrap;color:Commons.Color.urgent;font.family:root.face;font.pixelSize:Style.font.caption}
            }
          }
        }
        Column {
          width:parent.width;spacing:Style.space(8);visible:root.tab==="Tools"
          Ui.Button {text:"Check Wayland backend";onClicked:root.app.call({method:"doctor"},function(result){root.report=result;if(result.geometry){root.newWidth=result.geometry[2];root.newHeight=result.geometry[3]}})}
          Text {width:parent.width;wrapMode:Text.Wrap;text:JSON.stringify(root.report,null,2);color:root.fg;font.family:root.face;font.pixelSize:Style.font.caption}
          Ui.PanelSectionHeader {text:"Rescale current task";foreground:root.fg}
          Row {
            spacing:Style.space(8)
            Ui.NumberField {label:"Old width";from:1;to:100000;value:root.oldWidth;onModified:function(v){root.oldWidth=v}}
            Ui.NumberField {label:"Old height";from:1;to:100000;value:root.oldHeight;onModified:function(v){root.oldHeight=v}}
            Ui.NumberField {label:"New width";from:1;to:100000;value:root.newWidth;onModified:function(v){root.newWidth=v}}
            Ui.NumberField {label:"New height";from:1;to:100000;value:root.newHeight;onModified:function(v){root.newHeight=v}}
          }
          Ui.Button {text:"Apply scale";onClicked:root.save(function(){root.app.change({method:"rescale",name:root.draft.id,old:[root.oldWidth,root.oldHeight],new:[root.newWidth,root.newHeight]},function(){root.app.call({method:"snapshot"},function(s){root.app.apply(s);root.load(root.draft.id)})})})}
        }
      }
    }
  }
}
