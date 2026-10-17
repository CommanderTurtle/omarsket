import QtQuick
import QtQuick.Controls as QQC
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui
import "model/Tasks.js" as Tasks

Rectangle {
  id:root
  required property int actionIndex
  required property var action
  property var service:null
  property bool expanded:false
  property bool running:false
  signal selected()
  signal changed(var value)
  signal moved(int delta)
  signal toTop()
  signal toBottom()
  signal copied()
  signal removed()
  implicitHeight:column.implicitHeight+Style.space(16)
  radius:Style.cornerRadius
  color:Qt.rgba(Commons.Color.foreground.r,Commons.Color.foreground.g,Commons.Color.foreground.b,0.04)
  border.width:running?1:0
  border.color:Commons.Color.accent
  Column {
    id:column;x:Style.space(8);y:Style.space(8);width:parent.width-Style.space(16);spacing:Style.space(8)
    Flow {
      width:parent.width;spacing:Style.space(6)
      Ui.Button {text:(root.expanded?"▾ ":"▸ ")+(root.actionIndex+1)+" · "+Tasks.title(root.action);onClicked:root.selected()}
      Ui.Button {text:"↑";tooltipText:"Move up";onClicked:root.moved(-1)}
      Ui.Button {text:"↓";tooltipText:"Move down";onClicked:root.moved(1)}
      Ui.Button {
        text:"⋯";tooltipText:"Action menu";onClicked:menu.popup()
        TaskMenu {
          id:menu
          QQC.MenuItem {text:"Move to top";onTriggered:root.toTop()}
          QQC.MenuItem {text:"Move to bottom";onTriggered:root.toBottom()}
          QQC.MenuItem {text:"Duplicate";onTriggered:root.copied()}
          QQC.MenuItem {text:"Delete";onTriggered:root.removed()}
        }
      }
    }
    Loader {
      width:parent.width;active:root.expanded
      sourceComponent:ActionEditor {width:parent.width;action:root.action;service:root.service;onChanged:function(v){root.changed(v)}}
    }
  }
}
