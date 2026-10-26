import QtQuick
import QtQuick.Controls as QQC
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui
import "model/Keyboard.js" as Keyboard

Column {
  id:root
  required property var service
  property string sessionId: ""
  property string name: ""
  property int work: 25
  property int rest: 5
  property var weights: ({})
  property var selectedKey: null
  property bool showMouse: false
  readonly property var book:service.heatmapState
  readonly property var sessions:book.sessions||[]
  readonly property var selected:sessions.find(function(s){return s.id===(root.sessionId||root.book.active)})||sessions[sessions.length-1]||null
  spacing:Style.space(8)
  Component.onCompleted:service.openHeatmap()
  Text {text:"Heatmap · recorded key usage";color:Commons.Color.accent;font.family:Style.font.family;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
  Text {visible:!root.service.heatmapConnected;text:root.service.heatmapError||"Connecting…";color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
  Ui.Button {visible:!root.service.heatmapConnected&&!!root.service.heatmapError;text:"Retry";onClicked:root.service.openHeatmap()}
  Column {
    width:parent.width;spacing:Style.space(8);visible:root.service.heatmapConnected
    KeyboardMap {width:parent.width;keymap:root.service.nativeKeymap;values:Keyboard.normalized(root.selected);showMouse:root.showMouse;selected:root.selectedKey?root.selectedKey.id:"";onKeySelected:function(key){root.selectedKey=key}}
    Ui.Toggle {width:parent.width;label:"Show mouse";checked:root.showMouse;onClicked:root.showMouse=!checked}
    Flow {
      width:parent.width;spacing:Style.space(8)
      Text {visible:root.selectedKey!==null;text:root.selectedKey?Keyboard.label(root.selectedKey,root.service.nativeKeymap)+" · "+Number(root.selected&&root.selected.values[root.selectedKey.id]||0).toFixed(3):"";color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
      Ui.NumberField {visible:!!root.selectedKey&&root.selectedKey.code!==null;label:"Weight (%)";from:1;to:10000;value:root.selectedKey?Math.round((root.weights[root.selectedKey.id]||1)*100):100;onModified:function(v){var next=Object.assign({},root.weights);next[root.selectedKey.id]=v/100;root.weights=next}}
    }
    Ui.TextField {width:parent.width;placeholderText:"Session name";text:root.name;onTextEdited:root.name=text}
    Flow {
      width:parent.width;spacing:Style.space(8)
      Ui.NumberField {label:"Focus minutes (0 = continuous)";from:0;to:1440;value:root.work;onModified:function(v){root.work=v}}
      Ui.NumberField {label:"Break minutes";from:0;to:1440;value:root.rest;onModified:function(v){root.rest=v}}
      Ui.Button {text:root.service.heatmapBusy?"Updating…":"Start";enabled:!root.service.heatmapBusy&&root.name.trim()!=="";onClicked:{root.sessionId="";root.service.heatmapCommand({action:"start",name:root.name,work:root.work*60,rest:root.rest*60,weights:root.weights})}}
      Ui.Button {text:"End";enabled:!root.service.heatmapBusy&&root.book.active!==null;onClicked:root.service.heatmapCommand({action:"end"})}
    }
    Text {visible:!!root.selected;text:root.selected?root.selected.name+" · "+Math.floor(root.selected.elapsed/60)+":"+String(root.selected.elapsed%60).padStart(2,"0")+(root.selected.id===root.book.active?" · "+(root.selected.work&&root.selected.rest&&root.selected.elapsed%(root.selected.work+root.selected.rest)>=root.selected.work?"Break":"Running"):" · Saved"):"";color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
    Text {visible:root.book.active!==null&&!root.service.heatmapAvailable;text:"Native key events unavailable";color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
    Text {visible:root.service.heatmapError!=="";text:root.service.heatmapError;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
    Repeater {
      model:root.sessions.slice().reverse()
      Flow {
        required property var modelData
        width:parent.width;spacing:Style.space(6)
        Ui.Button {text:parent.modelData.name;foreground:root.selected&&root.selected.id===parent.modelData.id?Commons.Color.accent:Commons.Color.foreground;onClicked:root.sessionId=parent.modelData.id}
        Ui.Button {text:"Resume";enabled:!root.service.heatmapBusy&&parent.modelData.id!==root.book.active;onClicked:root.service.heatmapCommand({action:"resume","id":parent.modelData.id})}
        Ui.Button {text:"Delete";enabled:!root.service.heatmapBusy&&parent.modelData.id!==root.book.active;onClicked:root.service.heatmapCommand({action:"delete","id":parent.modelData.id})}
      }
    }
  }
}
