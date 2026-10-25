import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui

Column {
  id:root
  required property var service
  property string inputLayer: "heatmap"
  signal resetScroll()
  onInputLayerChanged:resetScroll()
  Component.onCompleted:service.refreshNative()
  spacing:Style.space(8)
  Flow {
    width:parent.width;spacing:Style.space(6)
    Repeater {model:[{id:"heatmap",name:"Heatmap"},{id:"macro",name:"Macros"},{id:"software",name:"Software"},{id:"os",name:"Omarchy"},{id:"driver",name:"Virtual input"},{id:"hardware",name:"Onboard"}];Ui.Button {required property var modelData;text:modelData.name;foreground:root.inputLayer===modelData.id?Commons.Color.accent:Commons.Color.foreground;onClicked:root.inputLayer=modelData.id}}
  }
  Flow {
    width:parent.width;spacing:Style.space(6)
    Ui.Button {text:root.service.nativeRefreshing?"Reading hotkeys…":"Refresh hotkeys";enabled:root.service.ready&&!root.service.nativeRefreshing;onClicked:root.service.refreshNative()}
    Text {width:Math.min(implicitWidth,root.width);wrapMode:Text.Wrap;text:root.service.nativeKeymap.layout||"Physical ANSI reference";textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  }
  Text {visible:root.service.nativeError!=="";width:parent.width;wrapMode:Text.Wrap;text:root.service.nativeError;textFormat:Text.PlainText;color:Commons.Color.urgent;font.family:Style.font.family;font.pixelSize:Style.font.caption}
  Loader {width:parent.width;active:root.inputLayer==="heatmap";sourceComponent:Component {Heatmap {width:parent.width;service:root.service}}}
  Loader {width:parent.width;active:["macro","software","os"].indexOf(root.inputLayer)>=0;sourceComponent:Component {Bindings {width:parent.width;service:root.service;bindingLayer:root.inputLayer}}}
  Loader {width:parent.width;active:root.inputLayer==="driver";sourceComponent:Component {InputRouter {width:parent.width;service:root.service}}}
  Loader {width:parent.width;active:root.inputLayer==="hardware";sourceComponent:Component {Hardware {width:parent.width;service:root.service}}}
}
