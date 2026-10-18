import QtQuick
import qs.Commons
import qs.Commons as Commons
import "model/Keyboard.js" as Keyboard

Item {
  id:root
  property var values: ({})
  property string selected: ""
  property bool showMouse: false
  readonly property real unit:width/22.8
  readonly property real maximum:Keyboard.maximum(values)
  implicitHeight:unit*(showMouse?10.8:6.6)
  signal keySelected(var key)
  Repeater {
    model:Keyboard.layout().concat(root.showMouse?Keyboard.mouse().map(function(k){return Object.assign({},k,{x:k.x+8,y:k.y+6.8})}):[])
    Rectangle {
      id:key
      required property var modelData
      readonly property real heat:Number(root.values[modelData.id])||0
      x:modelData.x*root.unit;y:modelData.y*root.unit
      width:modelData.w*root.unit-Style.space(2);height:modelData.h*root.unit-Style.space(2)
      radius:Style.cornerRadius
      color:heat>0?Qt.tint(Commons.Color.background,Qt.rgba(Commons.Color.urgent.r,Commons.Color.urgent.g,Commons.Color.urgent.b,0.12+0.75*heat/root.maximum)):Commons.Color.background
      border.width:1;border.color:root.selected===modelData.id?Commons.Color.accent:Qt.rgba(Commons.Color.foreground.r,Commons.Color.foreground.g,Commons.Color.foreground.b,0.3)
      Accessible.name:modelData.label+" "+heat.toFixed(3)
      Text {anchors.centerIn:parent;text:key.modelData.label;textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Math.min(Style.font.caption,root.unit*(key.modelData.label.length>4?0.26:0.36))}
      Text {anchors{bottom:parent.bottom;horizontalCenter:parent.horizontalCenter;bottomMargin:Style.space(2)} text:key.modelData.fn;textFormat:Text.PlainText;color:Commons.Color.accent;font.family:Style.font.family;font.pixelSize:Math.min(Style.font.caption,root.unit*0.2)}
      MouseArea {anchors.fill:parent;onClicked:root.keySelected(key.modelData)}
    }
  }
}
