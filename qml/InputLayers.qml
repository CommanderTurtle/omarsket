import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui

Column {
  id:root
  required property var service
  property string layer: "heatmap"
  spacing:Style.space(8)
  Flow {
    width:parent.width;spacing:Style.space(6)
    Repeater {model:[{id:"heatmap",name:"Heatmap"},{id:"macro",name:"Macros"},{id:"software",name:"Software"},{id:"os",name:"Omarchy"}];Ui.Button {required property var modelData;text:modelData.name;foreground:root.layer===modelData.id?Commons.Color.accent:Commons.Color.foreground;onClicked:root.layer=modelData.id}}
  }
  Loader {width:parent.width;active:root.layer==="heatmap";sourceComponent:Component {Heatmap {width:parent.width;service:root.service}}}
  Loader {width:parent.width;active:root.layer!=="heatmap";sourceComponent:Component {Bindings {width:parent.width;service:root.service;layer:root.layer}}}
}
