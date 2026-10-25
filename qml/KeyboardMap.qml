import QtQuick
import QtQuick.Controls as QQC
import qs.Commons
import qs.Commons as Commons
import "model/Keyboard.js" as Keyboard

Item {
  id:root
  property var values: ({})
  property string selected: ""
  property bool showMouse: false
  property var keymap: ({})
  property var hints: ({})
  readonly property var geometry:Keyboard.geometry(width,showMouse)
  readonly property real unit:geometry.unit
  readonly property real maximum:Keyboard.maximum(values)
  implicitHeight:geometry.height
  height:implicitHeight
  signal keySelected(var key)
  Repeater {
    model:Keyboard.layout().concat(root.showMouse?Keyboard.mouse().map(function(k){return Object.assign({},k,{x:k.x+8,y:k.y+6.8})}):[])
    Rectangle {
      id:key
      required property var modelData
      readonly property string legend:Keyboard.label(modelData,root.keymap)
      readonly property string hint:String(root.hints[modelData.id]||"")
      readonly property real heat:Number(root.values[modelData.id])||0
      x:modelData.x*root.unit;y:modelData.y*root.unit
      width:Math.max(0,modelData.w*root.unit-Style.space(2));height:Math.max(0,modelData.h*root.unit-Style.space(2))
      radius:Style.cornerRadius
      color:heat>0?Qt.tint(Commons.Color.background,Qt.rgba(Commons.Color.urgent.r,Commons.Color.urgent.g,Commons.Color.urgent.b,0.12+0.75*heat/root.maximum)):Commons.Color.background
      border.width:1;border.color:root.selected===modelData.id?Commons.Color.accent:Qt.rgba(Commons.Color.foreground.r,Commons.Color.foreground.g,Commons.Color.foreground.b,0.3)
      Accessible.name:legend+" "+heat.toFixed(3)+(hint?" · "+hint:"")
      Text {anchors.centerIn:parent;anchors.verticalCenterOffset:key.modelData.fn?-root.unit*.12:0;width:Math.max(0,parent.width-Style.space(2));horizontalAlignment:Text.AlignHCenter;elide:Text.ElideRight;text:key.legend;textFormat:Text.PlainText;color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Math.max(1,Math.min(Style.font.caption,root.unit*(key.legend.length>4?0.26:0.36)))}
      Text {visible:text!=="";anchors{bottom:parent.bottom;horizontalCenter:parent.horizontalCenter;bottomMargin:Style.space(2)} text:key.modelData.fn;textFormat:Text.PlainText;color:Commons.Color.accent;font.family:Style.font.family;font.pixelSize:Math.max(1,Math.min(Style.font.caption,root.unit*0.2))}
      Rectangle {visible:key.hint!=="";anchors{right:parent.right;top:parent.top;margins:Style.space(2)} width:Math.min(Style.space(4),root.unit*.15);height:width;radius:width/2;color:Commons.Color.accent}
      QQC.ToolTip.visible:pointer.containsMouse
      QQC.ToolTip.text:key.legend+(key.hint?"\n"+key.hint:"")
      MouseArea {id:pointer;anchors.fill:parent;hoverEnabled:true;onClicked:root.keySelected(key.modelData)}
    }
  }
}
