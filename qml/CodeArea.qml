import QtQuick
import QtQuick.Controls as QQC
import qs.Commons
import qs.Commons as Commons
import qs.Ui as Ui

QQC.ScrollView {
  id: root
  property alias text: editor.text
  property string hint: ""
  signal edited(string value)
  implicitHeight: Style.space(125)
  clip: true
  QQC.TextArea {
    id: editor
    placeholderText: root.hint
    wrapMode: TextEdit.WrapAnywhere
    selectByMouse: true
    color: Commons.Color.foreground
    selectionColor: Qt.rgba(Commons.Color.accent.r,Commons.Color.accent.g,Commons.Color.accent.b,0.3)
    placeholderTextColor: Qt.darker(Commons.Color.foreground,1.6)
    font.family: Style.font.family
    font.pixelSize: Style.font.body
    background: Ui.BorderSurface { color: Qt.rgba(Commons.Color.foreground.r,Commons.Color.foreground.g,Commons.Color.foreground.b,0.04); radius:Style.cornerRadius }
    onTextChanged: if (activeFocus) root.edited(text)
  }
}
