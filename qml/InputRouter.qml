import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui
import "model/Tasks.js" as Tasks

Column {
  id:root
  required property var service
  property bool optIn:false
  property var book: ({profiles:[],devices:[],active:null,helper:false})
  property var profile: ({name:"",devices:[]})
  property var previous:null
  property int deviceIndex:0
  property string selected:"ts_uinput"
  property string devicePath:""
  property string error:""
  property bool loading:false
  property bool alive:true
  property bool settingTouched:false
  property bool editing:false
  property bool showMouse:false
  property string selectedKey:""
  property int sourceCode:33
  property int targetCode:34
  property bool pickingTarget:false
  property bool disableKey:false
  property var modifiers:[]
  property double authorizingUntil:0
  readonly property var device:profile.devices[deviceIndex]||null
  readonly property bool starting:Date.now()<authorizingUntil&&!book.active
  spacing:Style.space(8)
  function call(request,callback){var owner=root;request.method="input-router";service.call(request,function(v,e){if(!owner||!owner.alive)return;owner.error=e||"";if(callback)callback(v,e)})}
  function load(first){if(loading)return;loading=true;call({action:"snapshot"},function(v,e){root.loading=false;if(!e){root.book=v;if(first&&!root.settingTouched&&v.active)root.optIn=true;if(v.error){root.error=v.error;root.authorizingUntil=0}}})}
  function choose(name){selected=name;editing=false;previous=book.profiles.find(function(p){return p.name===name})||null;profile=previous?Tasks.copy(previous):{name:"",devices:[]};deviceIndex=0}
  function update(key,value){var p=Tasks.copy(profile);p[key]=value;profile=p}
  function addDevice(){var found=book.devices.find(function(d){return d.path===root.devicePath});if(!found||profile.devices.some(function(d){return d.path===found.path}))return;update("devices",profile.devices.concat([{path:found.path,vendor:found.vendor,product:found.product,keys:{}}]));deviceIndex=profile.devices.length-1}
  function mapKey(){if(!device)return;var p=Tasks.copy(profile);p.devices[deviceIndex].keys[String(sourceCode)]=disableKey?[]:modifiers.concat([targetCode]).filter(function(k,i,a){return a.indexOf(k)===i});profile=p}
  function removeKey(code){var p=Tasks.copy(profile);delete p.devices[deviceIndex].keys[code];profile=p}
  function save(){if(loading)return;loading=true;var saved=Tasks.copy(profile);call({action:"save",profile:saved,expected:previous&&previous.name===saved.name?previous:null},function(v,e){root.loading=false;if(!e){root.book=v;root.choose(saved.name)}})}
  Component.onCompleted:load(true)
  Component.onDestruction:{alive=false;if(root.starting)service.call({method:"input-router",action:"stop"})}
  Timer {interval:1000;repeat:true;running:root.optIn&&(!!root.book.active||Date.now()<root.authorizingUntil);onTriggered:root.load()}
  Text {text:"ts_uinput · native touch / pass-through";textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  Ui.Toggle {label:"Enable virtual keyboard/mouse layer";checked:root.optIn;onClicked:{root.settingTouched=true;if(checked){root.call({action:"stop"},function(){root.load()});root.optIn=false;root.authorizingUntil=0;root.editing=false}else{root.optIn=true;root.load()}}}
  Column {
    visible:root.optIn;width:parent.width;spacing:Style.space(8)
    Text {visible:!root.book.helper;width:parent.width;wrapMode:Text.Wrap;text:"Initialize the optional input helper in quick-start.md.";textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
    KeyboardMap {width:parent.width;showMouse:root.showMouse;selected:root.selectedKey;onKeySelected:function(k){if(!root.editing||k.code===null)return;root.selectedKey=k.id;if(root.pickingTarget)root.targetCode=k.code;else root.sourceCode=k.code}}
    Ui.Toggle {label:"Show mouse";checked:root.showMouse;onClicked:root.showMouse=!checked}
    Ui.Dropdown {label:"Profile";options:["ts_uinput"].concat(root.book.profiles.map(function(p){return p.name}));value:root.selected;enabled:!root.editing;onChanged:function(v){root.choose(v)}}
    Flow {
      width:parent.width;spacing:Style.space(6)
      Ui.Button {text:"New";onClicked:{root.choose("ts_uinput");root.selected="";root.editing=true}}
      Ui.Button {text:"Edit";iconText:"󰏫";enabled:root.selected!=="ts_uinput"&&!root.book.active;onClicked:root.editing=true}
      Ui.Button {text:"Save";iconText:"󰆓";enabled:!root.loading&&root.editing&&root.profile.name.trim()!==""&&root.profile.devices.length>0;onClicked:root.save()}
      Ui.Button {text:"Cancel";visible:root.editing;onClicked:root.choose(root.previous?root.previous.name:"ts_uinput")}
      Ui.Button {text:"Delete";enabled:root.selected!=="ts_uinput"&&!root.editing&&!root.book.active;onClicked:root.call({action:"delete",name:root.selected},function(v,e){if(!e){root.book=v;root.choose("ts_uinput")}})}
    }
    Column {
      visible:root.editing;enabled:!root.loading;width:parent.width;spacing:Style.space(8)
      Ui.TextField {width:parent.width;placeholderText:"Named input profile";text:root.profile.name;onTextEdited:root.update("name",text)}
      Ui.Dropdown {label:"Physical device";options:root.book.devices.map(function(d){return d.path});value:root.devicePath;onChanged:function(v){root.devicePath=v}}
      Ui.Button {text:"Add device";enabled:root.devicePath!=="";onClicked:root.addDevice()}
      Ui.Dropdown {label:"Editing device";options:root.profile.devices.map(function(d){return d.path});value:root.device?root.device.path:"";onChanged:function(v){root.deviceIndex=root.profile.devices.findIndex(function(d){return d.path===v})}}
      Ui.Button {text:"Remove device";enabled:!!root.device;onClicked:{root.update("devices",root.profile.devices.filter(function(_,i){return i!==root.deviceIndex}));root.deviceIndex=0}}
      Ui.Toggle {label:"Pick output key";checked:root.pickingTarget;onClicked:root.pickingTarget=!checked}
      Flow {
        width:parent.width;spacing:Style.space(6)
        Ui.NumberField {label:"Input (evdev)";from:1;to:767;value:root.sourceCode;onModified:function(v){root.sourceCode=v}}
        Ui.NumberField {label:"Output (evdev)";from:1;to:767;value:root.targetCode;onModified:function(v){root.targetCode=v}}
        Repeater {model:[{code:29,label:"Ctrl"},{code:42,label:"Shift"},{code:56,label:"Alt"},{code:125,label:"Super"}];Ui.Toggle {required property var modelData;label:modelData.label;checked:root.modifiers.indexOf(modelData.code)>=0;onClicked:root.modifiers=checked?root.modifiers.filter(function(v){return v!==modelData.code}):root.modifiers.concat([modelData.code])}}
        Ui.Toggle {label:"Disable key";checked:root.disableKey;onClicked:root.disableKey=!checked}
        Ui.Button {text:"Set mapping";enabled:!!root.device;onClicked:root.mapKey()}
      }
      Repeater {
        model:root.device?Object.keys(root.device.keys):[]
        Row {
          required property string modelData
          spacing:Style.space(6)
          Text {text:parent.modelData+" → "+root.device.keys[parent.modelData].join(" + ");textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.body}
          Ui.Button {text:"×";onClicked:root.removeKey(parent.modelData)}
        }
      }
    }
    Flow {
      width:parent.width;spacing:Style.space(6)
      Ui.Button {text:"Start · authorize";enabled:root.book.helper&&!root.editing&&!root.book.active&&!root.starting&&root.selected!=="ts_uinput"&&root.selected!=="";onClicked:{root.authorizingUntil=Date.now()+120000;root.call({action:"start",name:root.selected},function(v,e){if(e)root.authorizingUntil=0;root.load()})}}
      Ui.Button {text:"Stop";enabled:!!root.book.active||Date.now()<root.authorizingUntil;onClicked:root.call({action:"stop"},function(){root.authorizingUntil=0;root.load()})}
      Ui.Button {text:"Refresh";onClicked:root.load()}
    }
    Text {visible:!!root.book.active;text:root.book.active?root.book.active.profile+" · active · Esc+Backspace+Enter to stop":"";textFormat:Text.PlainText;color:Commons.Color.accent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  }
  Text {visible:root.error!=="";width:parent.width;wrapMode:Text.Wrap;text:root.error;textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
}
