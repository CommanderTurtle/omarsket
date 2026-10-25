import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui

Column {
  id:root
  required property var service
  property bool optIn:false
  property bool showMouse:false
  property var vendors:[]
  property var state: ({providers:[]})
  property bool loading:false
  property bool alive:true
  property string error:""
  function infer(){var owner=root;loading=true;service.call({method:"hardware-infer",vendors:vendors.slice()},function(v,e){if(!owner||!owner.alive)return;owner.loading=false;owner.error=e||"";if(!e&&owner.optIn)owner.state=v})}
  Component.onDestruction:alive=false
  spacing:Style.space(8)
  Ui.Toggle {label:"Enable onboard discovery";checked:root.optIn;onClicked:{root.optIn=!checked;if(checked){root.state={providers:[]};root.error=""}}}
  Column {
    enabled:root.optIn;width:parent.width;spacing:Style.space(8)
    KeyboardMap {width:parent.width;keymap:root.service.nativeKeymap;showMouse:root.showMouse;opacity:root.optIn?1:.4}
    Ui.Toggle {label:"Show mouse";checked:root.showMouse;onClicked:root.showMouse=!checked}
    Flow {
      width:parent.width;spacing:Style.space(8)
      Repeater {model:["razer","steelseries","logitech"];Ui.Toggle {required property string modelData;label:modelData;checked:root.vendors.indexOf(modelData)>=0;onClicked:root.vendors=checked?root.vendors.filter(function(v){return v!==modelData}):root.vendors.concat([modelData])}}
      Ui.Button {text:root.loading?"Reading…":"Infer";enabled:!root.loading&&root.vendors.length>0;onClicked:root.infer()}
    }
    Repeater {
      model:root.state.providers||[]
      Column {
        required property var modelData
        width:parent.width;spacing:Style.space(6)
        Text {width:parent.width;wrapMode:Text.Wrap;text:parent.modelData.provider+" · "+(parent.modelData.available?"read-only":"not connected");textFormat:Text.PlainText;color:parent.modelData.available?Commons.Color.foreground:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.body}
        CodeArea {width:parent.width;visible:parent.modelData.available;readOnly:true;text:JSON.stringify(parent.modelData,null,2)}
      }
    }
    Text {width:parent.width;wrapMode:Text.Wrap;text:"Read-only native state. Hardware writes need a supported device-specific operation; software macro support does not imply onboard memory.";textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  }
  Text {visible:root.error!=="";text:root.error;textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
}
