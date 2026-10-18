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
  readonly property color fg: barForeground
  readonly property string face: bar ? bar.fontFamily : Style.font.family
  property string tab: "Tasks"
  onTabChanged:if(app)app.heatmapVisible=opened&&tab==="Keyboard"
  property var documents: []
  property int documentIndex: -1
  property var draft: Tasks.blank("Untitled")
  property string savedName: ""
  property bool dirty: false
  property bool saving: false
  property int closingIndex: -1
  property string confirmText: ""
  property var confirmCallback: null
  property string nameOperation: ""
  property string nameValue: ""
  property string importPath: ""
  property string addType: "paste"
  property int selectedAction: 0
  property int duplicateTimes: 1
  property bool duplicateVisible: false
  property var pendingRun: null
  property string dataName: ""
  property string dataKind: "paste"
  property var dataAction: Tasks.action("paste")
  property bool dataDirty: false
  property bool dataSaving: false
  property var report: ({})
  property int oldWidth: 1920
  property int oldHeight: 1080
  property int newWidth: 1920
  property int newHeight: 1080
  property real clock: Date.now()
  readonly property bool autoScroll: !app || app.preferences.auto_scroll !== false
  readonly property bool pinned: app && app.preferences.pinned === true
  readonly property var currentJobs: app ? app.jobs.filter(function(j){return j.task===root.savedName || (j.temporary && j.task===root.draft.id)}) : []
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function set(key,value) {var next=Tasks.copy(draft);next[key]=value;draft=next;dirty=true}
  function replaceAction(index,value) {var next=Tasks.copy(draft.actions);next[index]=value;set("actions",next)}
  function stash() {
    if(documentIndex<0)return
    var docs=documents.slice();docs[documentIndex]={saved:savedName,draft:Tasks.copy(draft),dirty:dirty};documents=docs
  }
  function choose(name) {
    if(saving)return
    stash()
    var docs=documents.slice(), index=-1
    if(name)index=docs.findIndex(function(d){return d.saved===name})
    if(index<0){
      var task=name&&app?app.tasks.find(function(t){return t.id===name}):null
      if(name&&!task)return
      if(!task){var names=docs.map(function(d){return d.draft.id}).concat(app?app.tasks.map(function(t){return t.id}):[]);task=Tasks.blank(Tasks.uniqueName(names,"Untitled"))}
      docs.push({saved:name || "",draft:Tasks.copy(task),dirty:!name});index=docs.length-1;documents=docs
    }
    activate(index);tab="Tasks"
  }
  function activate(index) {
    if(saving || index<0 || index>=documents.length)return
    stash();documentIndex=index
    var doc=documents[index];savedName=doc.saved;draft=Tasks.copy(doc.draft);dirty=doc.dirty;selectedAction=0
  }
  function closeDocument(index,discard) {
    if(saving || index<0 || index>=documents.length)return
    stash()
    if(!discard && documents[index].dirty){closingIndex=index;return}
    var docs=documents.slice();docs.splice(index,1);documents=docs;documentIndex=-1;closingIndex=-1
    if(docs.length)activate(Math.min(index,docs.length-1));else choose("")
  }
  function save(callback) {
    if(!app || saving)return
    saving=true
    app.change({method:"save",task:Tasks.copy(draft),previous:savedName || undefined,create:!savedName},function(result,error){
      root.saving=false
      if(error)return
      root.savedName=root.draft.id;root.dirty=false;root.stash()
      if(callback)callback()
    })
  }
  function run(temporary) {
    if(!app || !app.ready || saving || pendingRun)return
    var epoch=app.operationEpoch
    var dispatch=function(){if(!root.app || epoch!==root.app.operationEpoch)return;root.pendingRun=temporary?{method:"run-temporary",task:Tasks.copy(root.draft)}:{method:"run",name:root.savedName};root.close();startRun.restart()}
    if(temporary)dispatch();else save(dispatch)
  }
  function ask(text,callback) {confirmText=text;confirmCallback=callback}
  function removeTask() {
    if(!savedName)return
    var name=savedName
    ask("Delete "+name+".omscht? Queued runs keep their snapshot.",function(){
      root.app.change({method:"delete",name:name},function(result,error){if(!error){root.stash();var index=root.documents.findIndex(function(d){return d.saved===name});if(index>=0)root.closeDocument(index,true)}})
    })
  }
  function importTask() {
    app.chooseFile(false,function(path){
      root.open()
      root.importPath=path;root.nameValue=Tasks.filename(path)
      if(root.app.tasks.some(function(t){return t.id===root.nameValue})){root.nameValue=Tasks.uniqueName(root.app.tasks.map(function(t){return t.id}),root.nameValue);root.nameOperation="import"}
      else root.finishImport(root.nameValue)
    })
  }
  function finishImport(name) {
    app.change({method:"import",path:importPath,name:name},function(result,error){if(!error)root.refreshTask(name,true)})
  }
  function refreshTask(name,open) {
    app.call({method:"snapshot"},function(s,error){
      if(error)return
      root.app.apply(s)
      if(open)root.choose(name)
      else {
        root.stash()
        var task=s.tasks.find(function(t){return t.id===name}),docs=root.documents.slice()
        if(!task)return
        for(var i=0;i<docs.length;i++)if(docs[i].saved===name&&!docs[i].dirty)docs[i]={saved:name,draft:Tasks.copy(task),dirty:false}
        root.documents=docs
        if(root.savedName===name&&!root.dirty)root.draft=Tasks.copy(task)
      }
    })
  }
  function reloadFiles() {
    stash()
    app.call({method:"reload"},function(s,error){
      if(error)return
      root.stash()
      root.app.apply(s)
      var docs=root.documents.slice()
      for(var i=0;i<docs.length;i++){
        if(docs[i].dirty || !docs[i].saved)continue
        var found=s.tasks.find(function(t){return t.id===docs[i].saved})
        if(found)docs[i]={saved:found.id,draft:Tasks.copy(found),dirty:false}
        else docs[i]={saved:"",draft:docs[i].draft,dirty:true}
      }
      root.documents=docs
      if(root.documentIndex>=0){var doc=docs[root.documentIndex];root.savedName=doc.saved;root.draft=Tasks.copy(doc.draft);root.dirty=doc.dirty}
    })
  }
  function exportTask(format) {
    save(function(){var name=root.savedName;root.app.chooseFile(true,function(path){var ext=format==="tasket"?".scht":".omscht";if(!path.toLowerCase().endsWith(ext))path+=ext;root.app.call({method:"export",name:name,path:path,format:format})},name+(format==="tasket"?".scht":".omscht"))})
  }
  function launcher() {save(function(){var name=root.savedName;root.app.chooseFile(true,function(path){if(!path.endsWith(".desktop"))path+=".desktop";root.app.call({method:"shortcut",name:name,path:path})},name+".desktop")})}
  function rescale() {
    save(function(){var name=root.savedName;root.saving=true;root.app.change({method:"rescale",name:name,old:[root.oldWidth,root.oldHeight],new:[root.newWidth,root.newHeight]},function(result,error){root.saving=false;if(!error)root.refreshTask(name,false)})})
  }
  function addAction(type) {
    if(draft.actions.length>=10000){app.error="A task may contain at most 10000 actions";return}
    set("actions",draft.actions.concat([Tasks.action(type)]));selectedAction=draft.actions.length-1
    Qt.callLater(function(){if(root.autoScroll)scroll.contentItem.contentY=Math.max(0,content.height-scroll.availableHeight)})
  }
  function moveAction(index,delta) {set("actions",Tasks.move(draft.actions,index,delta));selectedAction=Math.max(0,Math.min(draft.actions.length-1,index+delta))}
  function copyAction(index) {if(draft.actions.length>=10000)return;var a=Tasks.copy(draft.actions);a.splice(index+1,0,Tasks.copy(a[index]));set("actions",a);selectedAction=index+1}
  function deleteAction(index) {var a=Tasks.copy(draft.actions);a.splice(index,1);set("actions",a);selectedAction=Math.min(index,a.length-1)}
  function preference(name,value) {if(app)app.change({method:"preference",name:name,value:value})}
  function loadData(name) {
    var go=function(){var item=name?root.app.data[name]:null;root.dataName=name || "";root.dataKind=item?item.kind:"paste";var action=Tasks.action(root.dataKind);if(item)action[root.dataKind==="paste"?"content":root.dataKind==="keyssequence"?"keysmap":"cursormovsmap"]=Tasks.copy(item.value);root.dataAction=action;root.dataDirty=false}
    if(dataDirty)ask("Discard unsaved data changes?",go);else go()
  }
  function saveData() {
    var key=dataKind==="paste"?"content":dataKind==="keyssequence"?"keysmap":"cursormovsmap"
    dataSaving=true
    app.change({method:"save-data",name:dataName,item:{kind:dataKind,value:dataAction[key]}},function(result,error){root.dataSaving=false;if(!error)root.dataDirty=false})
  }
  function deleteData() {
    var name=dataName
    ask("Delete reusable data "+name+"?",function(){root.app.change({method:"delete-data",name:name},function(result,error){if(!error&&root.dataName===name){root.dataDirty=false;root.loadData("")}})})
  }
  Timer {id:startRun;interval:300;onTriggered:if(root.app&&root.pendingRun){root.app.call(root.pendingRun);root.pendingRun=null}}
  Timer {interval:1000;running:root.opened;repeat:true;onTriggered:root.clock=Date.now()}
  QtObject {id:dismissal;function close(){if(!root.pinned)root.close()}}
  onOpenedChanged:if(app){app.heatmapVisible=opened&&tab==="Keyboard";if(opened){app.refresh();if(documentIndex<0)choose("")}}
  Connections {
    target: root.app
    function onStopping(){startRun.stop();root.pendingRun=null}
    function onJobsChanged() {
      if(!root.autoScroll || !root.opened || root.tab!=="Tasks")return
      var job=root.currentJobs.find(function(j){return j.state==="running"})
      if(!job)return
      var card=cards.itemAt(job.action)
      if(card){var p=card.mapToItem(content,0,0);scroll.contentItem.contentY=Math.max(0,Math.min(p.y,content.height-scroll.availableHeight))}
    }
  }

  Ui.WidgetButton {
    id:button;anchors.fill:parent;bar:root.bar
    text:"󰑮"+(root.app&&root.app.activeCount>0?" "+root.app.activeCount:"")
    tooltipText:"Omarsket · right-click to stop tasks"
    onPressed:function(mouseButton){if(mouseButton===Qt.RightButton&&root.app)root.app.stop();else root.toggle()}
  }
  Ui.KeyboardPanel {
    id:panel;anchorItem:button;owner:dismissal;bar:root.bar;open:root.opened
    focusTarget:root.tab==="Tasks"?taskName:panelBody
    contentWidth:panel.fittedContentWidth(Style.space(680))
    contentHeight:panel.fittedContentHeight(Style.space(850),Style.space(850))
    Column {
      id:panelBody;anchors.fill:parent;spacing:Style.space(8)
      Keys.onEscapePressed:root.close()
      Column {
        id:header;width:parent.width;spacing:Style.space(8)
        Ui.PanelHero {title:"Omarsket";meta:root.app&&root.app.ready?root.app.activeCount+" active":"Runtime stopped";foreground:root.fg;fontFamily:root.face}
        Flow {
          width:parent.width;spacing:Style.space(6)
          Ui.Button {
            text:"File";enabled:root.app&&root.app.ready&&!root.saving;onClicked:fileMenu.popup()
            TaskMenu {
              id:fileMenu
              QQC.MenuItem {text:"New schedule";onTriggered:root.choose("")}
              QQC.MenuItem {text:"Open / manage schedules…";onTriggered:root.tab="Schedules"}
              QQC.MenuItem {text:"Save";onTriggered:root.save()}
              QQC.MenuItem {text:"Rename…";onTriggered:{root.nameValue=root.draft.id;root.nameOperation="rename"}}
              QQC.MenuItem {text:"Duplicate…";onTriggered:{root.nameValue=Tasks.uniqueName(root.app.tasks.map(function(t){return t.id}),root.draft.id+" copy");root.nameOperation="duplicate"}}
              QQC.MenuItem {text:"Close tab";onTriggered:root.closeDocument(root.documentIndex,false)}
              QQC.MenuSeparator {}
              QQC.MenuItem {text:"Import .omscht / .scht…";onTriggered:root.importTask()}
              QQC.MenuItem {text:"Import folder…";onTriggered:root.app.chooseFile(false,function(path){root.app.change({method:"import-folder",path:path})},"",true)}
              QQC.MenuItem {text:"Export .omscht…";onTriggered:root.exportTask("omarsket")}
              QQC.MenuItem {text:"Export Tasket .scht…";onTriggered:root.exportTask("tasket")}
              QQC.MenuSeparator {}
              QQC.MenuItem {text:"Import collection JSON…";onTriggered:root.app.chooseFile(false,function(path){root.app.change({method:"import-library",path:path})})}
              QQC.MenuItem {text:"Export collection JSON…";onTriggered:root.app.chooseFile(true,function(path){root.app.call({method:"export-library",path:path})},"library.json")}
              QQC.MenuItem {text:"Reload files";onTriggered:root.reloadFiles()}
              QQC.MenuSeparator {}
              QQC.MenuItem {text:"Delete schedule…";enabled:root.savedName!=="";onTriggered:root.removeTask()}
              QQC.MenuItem {text:"Hide panel";onTriggered:root.close()}
            }
          }
          Ui.Button {
            text:"Task";enabled:root.app&&root.app.ready&&!root.saving;onClicked:taskMenu.popup()
            TaskMenu {
              id:taskMenu
              QQC.MenuItem {text:"Save and schedule";onTriggered:root.run(false)}
              QQC.MenuItem {text:"Run temporary copy";onTriggered:root.run(true)}
              QQC.MenuItem {text:"Stop this schedule";onTriggered:root.currentJobs.forEach(function(j){root.app.call({method:"stop",number:j.number})})}
              QQC.MenuSeparator {}
              QQC.MenuItem {text:"Duplicate action list…";onTriggered:root.duplicateVisible=true}
              QQC.MenuItem {text:"Rescale cursor positions…";onTriggered:root.tab="Tools"}
              QQC.MenuItem {text:"Create launcher…";onTriggered:root.launcher()}
              QQC.MenuItem {text:"Startup schedules…";onTriggered:root.tab="Startup"}
            }
          }
          Ui.Button {
            text:"View";onClicked:viewMenu.popup()
            TaskMenu {
              id:viewMenu
              QQC.MenuItem {text:"Follow running action";checkable:true;checked:root.autoScroll;onTriggered:root.preference("auto_scroll",!root.autoScroll)}
              QQC.MenuItem {text:"Pin panel";checkable:true;checked:root.pinned;onTriggered:root.preference("pinned",!root.pinned)}
              QQC.MenuItem {text:"Backend / storage…";onTriggered:root.tab="Tools"}
              QQC.MenuItem {text:"About";onTriggered:root.tab="About"}
            }
          }
          Repeater {model:["Tasks","Data","History"];Ui.Button {required property string modelData;text:modelData;foreground:root.tab===modelData?Commons.Color.accent:root.fg;onClicked:root.tab=modelData}}
          Ui.Button {text:"Keyboard";iconText:"󰌌";foreground:root.tab==="Keyboard"?Commons.Color.accent:root.fg;onClicked:root.tab="Keyboard"}
          Ui.Button {text:"Stop all";iconText:"󰓛";onClicked:if(root.app)root.app.stop()}
          Ui.Button {text:"×";tooltipText:"Hide panel";onClicked:root.close()}
        }
        Ui.PanelSeparator {foreground:root.fg}
      }
      QQC.ScrollView {
        id:scroll;width:parent.width;height:Math.max(0,parent.height-header.height-parent.spacing);clip:true;contentWidth:availableWidth
        Column {
          id:content;width:scroll.availableWidth;spacing:Style.space(10)
          Text {visible:root.app&&root.app.error!=="";text:root.app?root.app.error:"";width:parent.width;wrapMode:Text.Wrap;textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:root.face;font.pixelSize:Style.font.body}
          Ui.Button {visible:root.app&&!root.app.ready;text:"Start runtime";onClicked:root.app.restart()}
          Loader {width:parent.width;active:root.tab==="Keyboard"&&!!root.app;sourceComponent:Component {InputLayers {width:parent.width;service:root.app}}}
          Flow {
            visible:root.closingIndex>=0;width:parent.width;spacing:Style.space(8)
            Text {textFormat:Text.PlainText;text:"Unsaved schedule";color:root.fg;font.family:root.face;font.pixelSize:Style.font.body}
            Ui.Button {text:"Save and close";onClicked:{var index=root.closingIndex;root.activate(index);root.save(function(){root.closeDocument(index,true)})}}
            Ui.Button {text:"Discard";onClicked:root.closeDocument(root.closingIndex,true)}
            Ui.Button {text:"Cancel";onClicked:root.closingIndex=-1}
          }
          Column {
            visible:root.confirmText!=="";width:parent.width;spacing:Style.space(6)
            Text {width:parent.width;wrapMode:Text.Wrap;text:root.confirmText;textFormat:Text.PlainText;color:root.fg;font.family:root.face;font.pixelSize:Style.font.body}
            Row {
              spacing:Style.space(8)
              Ui.Button {text:"Confirm";onClicked:{var callback=root.confirmCallback;root.confirmCallback=null;root.confirmText="";if(callback)callback()}}
              Ui.Button {text:"Cancel";onClicked:{root.confirmText="";root.confirmCallback=null}}
            }
          }
          Column {
            visible:root.nameOperation!=="";width:parent.width;spacing:Style.space(6)
            Ui.TextField {width:parent.width;text:root.nameValue;placeholderText:"Schedule name";onTextEdited:root.nameValue=text}
            Row {
              spacing:Style.space(8)
              Ui.Button {text:"Apply";onClicked:{var op=root.nameOperation,name=root.nameValue;root.nameOperation="";if(op==="import")root.finishImport(name);else if(op==="rename"){root.set("id",name);root.save()}else root.save(function(){root.app.change({method:"duplicate",name:root.savedName,new_name:name},function(result,error){if(!error)root.refreshTask(name,true)})})}}
              Ui.Button {text:"Cancel";onClicked:root.nameOperation=""}
            }
          }
          Column {
            width:parent.width;spacing:Style.space(8);visible:root.tab==="Tasks";enabled:root.app&&root.app.ready&&!root.saving
            Flow {
              width:parent.width;spacing:Style.space(6)
              Repeater {
                model:root.documents
                Row {
                  required property var modelData
                  required property int index
                  Ui.Button {text:(index===root.documentIndex?root.draft.id:modelData.draft.id)+((index===root.documentIndex?root.dirty:modelData.dirty)?" *":"");foreground:index===root.documentIndex?Commons.Color.accent:root.fg;onClicked:root.activate(parent.index)}
                  Ui.Button {text:"×";tooltipText:"Close schedule tab";onClicked:root.closeDocument(parent.index,false)}
                }
              }
              Ui.Button {text:"+";tooltipText:"New schedule";onClicked:root.choose("")}
              Ui.Button {text:"Open…";onClicked:root.tab="Schedules"}
            }
            Ui.TextField {id:taskName;width:parent.width;placeholderText:"Schedule name";text:root.draft.id;onTextEdited:root.set("id",text)}
            Ui.TextField {width:parent.width;placeholderText:"Description";text:root.draft.description;onTextEdited:root.set("description",text)}
            Flow {
              width:parent.width;spacing:Style.space(12)
              Ui.NumberField {label:"Delay (seconds)";from:0;to:86400;value:root.draft.delay_seconds;onModified:function(v){root.set("delay_seconds",v)}}
              Ui.NumberField {label:"Loops (-1 = forever)";from:-1;to:100000;value:root.draft.loop_times;onModified:function(v){root.set("loop_times",v===0?1:v)}}
              Ui.Button {text:root.dirty?"Save *":"Save";iconText:"󰆓";onClicked:root.save()}
              Ui.Button {text:"Schedule";iconText:"󰐊";onClicked:root.run(false)}
              Ui.Button {text:"Run temporary";tooltipText:"Run this draft without saving a file";onClicked:root.run(true)}
            }
            Repeater {
              model:root.currentJobs.filter(function(j){return j.state==="running"||j.state==="scheduled"})
              Flow {
                required property var modelData
                width:parent.width;spacing:Style.space(8)
                Text {textFormat:Text.PlainText;text:"#"+modelData.number+" · "+Tasks.jobText(modelData,root.clock);color:Commons.Color.accent;font.family:root.face;font.pixelSize:Style.font.body}
                Ui.Button {text:"Stop";onClicked:root.app.call({method:"stop",number:parent.modelData.number})}
              }
            }
            Ui.Toggle {width:parent.width;label:"Run when Omarsket opens";checked:root.draft.auto_start;onClicked:root.set("auto_start",!root.draft.auto_start)}
            Ui.Toggle {width:parent.width;label:"Allow system and file commands";checked:root.draft.allow_system;onClicked:root.set("allow_system",!root.draft.allow_system)}
            Ui.PanelSeparator {foreground:root.fg}
            Repeater {
              id:cards;model:root.draft.actions.length
              ActionCard {
                required property int index
                width:parent.width;actionIndex:index;action:root.draft.actions[index] || Tasks.action("wait");service:root.app
                expanded:root.selectedAction===index
                running:root.currentJobs.some(function(j){return j.state==="running"&&j.action===index})
                onSelected:root.selectedAction=root.selectedAction===index?-1:index
                onChanged:function(value){root.replaceAction(index,value)}
                onMoved:function(delta){root.moveAction(index,delta)}
                onToTop:root.moveAction(index,-index)
                onToBottom:root.moveAction(index,root.draft.actions.length-1-index)
                onCopied:root.copyAction(index)
                onRemoved:root.deleteAction(index)
              }
            }
            Flow {
              width:parent.width;spacing:Style.space(8)
              Ui.Dropdown {label:"Add action";value:root.addType;options:["paste","wait","keyssequence","cursormovements","systemcommand","runningothertask","move","click","scroll","hotkey","type"];onChanged:function(v){root.addType=v}}
              Ui.Button {text:"Add";onClicked:root.addAction(root.addType)}
              Ui.Button {text:"Duplicate list…";onClicked:root.duplicateVisible=!root.duplicateVisible}
            }
            Flow {
              visible:root.duplicateVisible;width:parent.width;spacing:Style.space(8)
              Ui.NumberField {label:"Additional copies";from:1;to:100;value:root.duplicateTimes;onModified:function(v){root.duplicateTimes=v}}
              Ui.Button {text:"Duplicate";onClicked:{try{root.set("actions",Tasks.duplicate(root.draft.actions,root.duplicateTimes));root.duplicateVisible=false}catch(e){root.app.error=e.message}}}
              Ui.Button {text:"Cancel";onClicked:root.duplicateVisible=false}
            }
          }
          Column {
            width:parent.width;spacing:Style.space(8);visible:root.tab==="Schedules";enabled:root.app&&root.app.ready&&!root.saving
            Flow {
              width:parent.width;spacing:Style.space(8)
              Ui.Button {text:"New";onClicked:root.choose("")}
              Ui.Button {text:"Import…";onClicked:root.importTask()}
            }
            Repeater {
              model:root.app?root.app.tasks:[]
              Column {
                required property var modelData
                width:parent.width;spacing:Style.space(6)
                Text {width:parent.width;wrapMode:Text.Wrap;text:modelData.id+".omscht · "+modelData.actions.length+" actions";textFormat:Text.PlainText;color:root.fg;font.family:root.face;font.pixelSize:Style.font.subtitle}
                Flow {
                  width:parent.width;spacing:Style.space(6)
                  Ui.Button {text:"Open";onClicked:root.choose(modelData.id)}
                  Ui.Button {text:"Rename…";onClicked:{root.choose(modelData.id);root.nameValue=modelData.id;root.nameOperation="rename"}}
                  Ui.Button {text:"Duplicate…";onClicked:{root.choose(modelData.id);root.nameValue=Tasks.uniqueName(root.app.tasks.map(function(t){return t.id}),modelData.id+" copy");root.nameOperation="duplicate"}}
                  Ui.Button {text:"Export…";onClicked:{root.choose(modelData.id);root.exportTask("omarsket")}}
                  Ui.Button {text:"Delete…";onClicked:{root.choose(modelData.id);root.removeTask()}}
                }
                Ui.PanelSeparator {foreground:root.fg}
              }
            }
          }
          Column {
            width:parent.width;spacing:Style.space(8);visible:root.tab==="Data";enabled:root.app&&root.app.ready&&!root.dataSaving
            Ui.Dropdown {width:parent.width;label:"Saved data";options:root.app?Object.keys(root.app.data):[];value:root.dataName;onChanged:function(v){root.loadData(v)}}
            Ui.TextField {width:parent.width;placeholderText:"Data name";text:root.dataName;onTextEdited:{root.dataName=text;root.dataDirty=true}}
            Ui.Dropdown {label:"Kind";options:["paste","keyssequence","cursormovements"];value:root.dataKind;onChanged:function(v){root.dataKind=v;root.dataAction=Tasks.action(v);root.dataDirty=true}}
            ActionEditor {width:parent.width;action:root.dataAction;service:root.app;dataOnly:true;onChanged:function(v){root.dataAction=v;root.dataDirty=true}}
            Flow {
              width:parent.width;spacing:Style.space(8)
              Ui.Button {text:"New data";onClicked:root.loadData("")}
              Ui.Button {text:root.dataDirty?"Save data *":"Save data";onClicked:root.saveData()}
              Ui.Button {text:"Delete…";onClicked:root.deleteData()}
              Ui.Button {text:"Import…";onClicked:root.app.chooseFile(false,function(path){root.app.change({method:"import-data",path:path})})}
              Ui.Button {text:"Export…";onClicked:root.app.chooseFile(true,function(path){root.app.call({method:"export-data",path:path})},"data.json")}
            }
          }
          Column {
            width:parent.width;spacing:Style.space(8);visible:root.tab==="History"
            Repeater {
              model:root.app?root.app.jobs.slice().reverse():[]
              Column {
                required property var modelData
                width:parent.width;spacing:Style.space(4)
                Text {width:parent.width;wrapMode:Text.Wrap;text:"#"+modelData.number+" · "+modelData.task+(modelData.temporary?" (temporary)":"")+" · "+Tasks.jobText(modelData,root.clock);textFormat:Text.PlainText;color:root.fg;font.family:root.face;font.pixelSize:Style.font.body}
                Ui.Button {visible:modelData.state==="scheduled"||modelData.state==="running";text:"Stop";onClicked:root.app.call({method:"stop",number:modelData.number})}
                Text {visible:modelData.error!=="";text:modelData.error;width:parent.width;wrapMode:Text.Wrap;textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:root.face;font.pixelSize:Style.font.caption}
              }
            }
          }
          Column {
            width:parent.width;spacing:Style.space(8);visible:root.tab==="Startup";enabled:root.app&&root.app.ready&&!root.saving
            Ui.PanelSectionHeader {text:"Run when Omarsket opens";foreground:root.fg}
            Repeater {
              model:root.app?root.app.tasks:[]
              Ui.Toggle {required property var modelData;width:parent.width;label:modelData.id;checked:modelData.auto_start;onClicked:{var task=Tasks.copy(modelData);task.auto_start=!task.auto_start;root.app.change({method:"save",task:task},function(result,error){if(!error&&root.savedName===task.id)root.set("auto_start",task.auto_start)})}}
            }
          }
          Column {
            width:parent.width;spacing:Style.space(8);visible:root.tab==="Tools";enabled:root.app&&root.app.ready&&!root.saving
            Text {width:parent.width;wrapMode:Text.Wrap;text:root.app?root.app.directory+"/schedules/*.omscht":"";textFormat:Text.PlainText;color:root.fg;font.family:root.face;font.pixelSize:Style.font.caption}
            Ui.Button {text:"Check Wayland backend";onClicked:root.app.call({method:"doctor"},function(result,error){if(!error){root.report=result;if(result.geometry){root.newWidth=result.geometry[2];root.newHeight=result.geometry[3]}}})}
            Text {width:parent.width;wrapMode:Text.Wrap;text:JSON.stringify(root.report,null,2);textFormat:Text.PlainText;color:root.fg;font.family:root.face;font.pixelSize:Style.font.caption}
            Ui.PanelSectionHeader {text:"Rescale current schedule";foreground:root.fg}
            Flow {
              width:parent.width;spacing:Style.space(8)
              Ui.NumberField {label:"Old width";from:1;to:100000;value:root.oldWidth;onModified:function(v){root.oldWidth=v}}
              Ui.NumberField {label:"Old height";from:1;to:100000;value:root.oldHeight;onModified:function(v){root.oldHeight=v}}
              Ui.NumberField {label:"New width";from:1;to:100000;value:root.newWidth;onModified:function(v){root.newWidth=v}}
              Ui.NumberField {label:"New height";from:1;to:100000;value:root.newHeight;onModified:function(v){root.newHeight=v}}
            }
            Ui.Button {text:"Apply scale";onClicked:root.rescale()}
            Repeater {model:root.app?root.app.notices:[];Text {required property string modelData;width:parent.width;text:modelData;wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.fg;font.family:root.face;font.pixelSize:Style.font.caption}}
          }
          Column {
            width:parent.width;spacing:Style.space(8);visible:root.tab==="About"
            Text {width:parent.width;wrapMode:Text.Wrap;text:"Omarsket · 0.1.0\nQuickshell / Rust / Wayland-automation\nTheme: Omarchy\nCommanderTurtle/omarsket";textFormat:Text.PlainText;color:root.fg;font.family:root.face;font.pixelSize:Style.font.body}
          }
        }
      }
    }
  }
}
