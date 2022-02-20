import QtQuick 2.0
import org.kde.kirigami 2.12 as Kirigami
import QtQuick.Layouts 1.3
import QtQuick.Controls 2.2 as Controls

Controls.Pane {
    id: root

    property alias currentCover: now_playing_cover.source
    property string currentTrackName
    property string currentArtist
    property string currentAlbum
    property string playState: "play"

    signal play
    signal pause
    signal stop
    signal next
    

    background: Item {
        anchors.fill: parent

        Rectangle {
            anchors.fill: parent
            color: "#eff0f1"
        }

        Rectangle {
            anchors {
                top: parent.top
                left: parent.left
                right: parent.right
            }
            color: "darkgrey"
            height: 1
        }

    }

    RowLayout {
        anchors.fill: parent

        AlbumCover {
            id: now_playing_cover
            Layout.preferredHeight: parent.height
            Layout.preferredWidth: parent.height
            Layout.rightMargin: 10
        }

        ColumnLayout {
            Kirigami.Heading {
                Layout.fillWidth: true
                level: 2
                text: root.currentTrackName + " - " + root.currentArtist
            }

            Kirigami.Separator {
                Layout.fillWidth: true
            }

            Controls.Label {
                Layout.fillWidth: true
                text: root.currentAlbum
            }
        }

        Controls.Button {
            Layout.alignment: Qt.AlignRight
            Layout.columnSpan: 2
            text: "Stop"
            onClicked: root.stop()
        }

        Controls.Button {
            Layout.alignment: Qt.AlignRight
            Layout.columnSpan: 2
            visible: root.playState === "play"
            text: "Pause"
            onClicked: root.pause()
        }

        Controls.Button {
            Layout.alignment: Qt.AlignRight
            Layout.columnSpan: 2
            visible: root.playState !== "play"
            text: "Play"
            onClicked: root.play()
        }

        Controls.Button {
            Layout.alignment: Qt.AlignRight
            Layout.columnSpan: 2
            text: "Next"
            onClicked: root.next()
        }
    }
}
