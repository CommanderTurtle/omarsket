import QtQuick
import qs.Commons
import qs.Commons as Commons

Column {
  required property var service
  spacing:Style.space(8)
  Text {text:"Heatmap · future feature";textFormat:Text.PlainText;color:Commons.Color.accent;font.family:Style.font.family;font.pixelSize:Style.font.body}
  Text {
    width:parent.width;wrapMode:Text.Wrap;textFormat:Text.PlainText
    text:"Not recording or connecting. Planned sessions will start explicitly in Macrohelp, with a temporary local log that Omarsket can read and the user can delete."
    color:Commons.Color.foreground;font.family:Style.font.family;font.pixelSize:Style.font.body
  }
}
