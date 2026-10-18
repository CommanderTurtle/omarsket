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
  contentWidth: availableWidth
  clip: true
  QQC.TextArea {
    id: editor
    placeholderText: root.hint
    wrapMode: TextEdit.WrapAnywhere
    textFormat: TextEdit.PlainText
    selectByMouse: true
    color: Commons.Color.foreground
    selectionColor: Style.selectionFillFor(Commons.Color.foreground,Commons.Color.accent)
    selectedTextColor:Commons.Color.foreground
    placeholderTextColor: Qt.darker(Commons.Color.foreground,1.6)
    font.family: Style.font.family
    font.pixelSize: Style.font.body
    readonly property var fieldBorder:Border.controlSpec(activeFocus?"focus":hovered?"hover-cursor":"normal",Commons.Color.foreground,Commons.Color.accent)
    leftPadding:Style.spacing.controlPaddingX+Border.left(fieldBorder)
    rightPadding:Style.spacing.controlPaddingX+Border.right(fieldBorder)
    topPadding:Style.spacing.inputPaddingY+Border.top(fieldBorder)
    bottomPadding:Style.spacing.inputPaddingY+Border.bottom(fieldBorder)
    background: Ui.BorderSurface { color:Style.controlFill(editor.activeFocus,editor.hovered,Commons.Color.foreground,Commons.Color.accent);borderSpec:editor.fieldBorder; radius:Style.cornerRadius }
    onTextChanged: if (activeFocus) root.edited(text)
  }
}
