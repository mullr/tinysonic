import QtQuick 2.9
import QtQuick.Controls 2.2 as Controls
import QtQuick.Layouts 1.3

Item {
    id: root

    property alias source: cover.source
    /* property bool show_hover_buttons: false */
    signal coverDoubleClicked
    signal playButtonClicked


    Image {
        id: cover
        /* property bool hovered: true */
        asynchronous: false
        mipmap: true

        anchors.fill: parent
        fillMode: Image.PreserveAspectFit

        Rectangle {
            id: bg
            z: -1
            anchors.centerIn: parent
            width: cover.status == Image.Ready ? cover.paintedWidth : root.width
            height: cover.status == Image.Ready ? cover.paintedHeight : root.height

            color: "white"
        }

        BorderImage {
            z: -2
            anchors {
                horizontalCenter: bg.horizontalCenter
                verticalCenter: bg.verticalCenter
            }

            width: bg.width + 12
            height: bg.height + 12

            border { left: 10; top: 6; right: 10; bottom: 10 }
            source: "shadow.png"
        }

        MouseArea {
            anchors.fill: bg
            onDoubleClicked: coverDoubleClicked()

            /* hoverEnabled: true */
            /* onEntered: { */
            /*     cover.hovered = true */
            /* } */
            /* onExited: { */
            /*     cover.hovered = false */
            /* } */

            /* Controls.Button { */
            /*     /\* Kirigami.Theme.inherit: false *\/ */
            /*     /\* Kirigami.Theme.colorSet: Kirigami.Theme.View *\/ */
            /*     visible: cover.hovered */
            /*     anchors { */
            /*         right: parent.right */
            /*         bottom: parent.bottom */
            /*         rightMargin: 5 */
            /*         bottomMargin: 5 */
            /*     } */
            /*     width: 50 */

            /*     icon.name: "media-playback-start" */
            /*     onClicked: playButtonClicked() */
            /* } */
        }
    }
}
