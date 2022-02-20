import QtQuick 2.9
import QtQuick.Layouts 1.3
import QtQuick.Controls 2.2 as Controls

Item {
    id: root
    property alias source: cover.source
    property alias title: title.text
    property alias artist: artist.text
    property string artist_id

    signal coverDoubleClicked
    signal artistClicked

    AlbumCover {
        id: cover
        anchors {
            left: parent.left
            right: parent.right
            top: parent.top
        }
        height: width
        onCoverDoubleClicked: root.coverDoubleClicked()
    }

    Controls.Label {
        id: title

        anchors {
            top: cover.bottom
            left: parent.left
            right: parent.right
        }

        wrapMode: Text.NoWrap
        elide: Text.ElideRight
        horizontalAlignment: Text.AlignHCenter
    }

    Controls.Label {
        id: artist
        anchors {
            top: title.bottom
            left: parent.left
            right: parent.right
        }

        wrapMode: Text.NoWrap
        elide: Text.ElideRight
        horizontalAlignment: Text.AlignHCenter
        font.underline: ma.containsMouse  ? true : false

        MouseArea {
            id: ma
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.artistClicked()
        }
    }


}
