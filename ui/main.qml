import QtQuick 2.9
import QtQuick.Controls 2.2 as Controls
import QtQuick.Layouts 1.3
import QtGraphicalEffects 1.0
import org.kde.kirigami 2.12 as Kirigami

import io.github.mullr.tinysonic 1.0

Kirigami.ApplicationWindow {
    id: root
    title: "tinysonic"
    pageStack.initialPage: albums_page

    Component.onCompleted: {
        Albums.fetch()
        Player.setup()
    }

    Kirigami.ScrollablePage {
        id: albums_page
        leftPadding: 0

        titleDelegate: RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.fillHeight: true
            Layout.fillWidth: true

            Controls.ComboBox {
                Layout.bottomMargin: 2
                Layout.fillHeight: true
                textRole: "text"
                currentIndex: 2 // Random, aligns with model initial value

                model: ListModel {
                    id: sortOrderItems
                    ListElement { text: "All (By Artist)"; value: "by_artist"}
                    ListElement { text: "All (By Album)"; value: "by_name"}
                    ListElement { text: "Random"; value: "random"}
                    ListElement { text: "Recently Played"; value: "recent"}
                    ListElement { text: "Recently Added"; value: "newest"}
                    ListElement { text: "Most Played"; value: "frequent"}
                }

                onCurrentIndexChanged: {
                    Albums.sort_order = sortOrderItems.get(currentIndex).value
                }
            }

            Controls.Button {
                icon.name: "view-refresh"
                onClicked: Albums.fetch()
            }

            Kirigami.SearchField {
                id: searchField
                Layout.bottomMargin: 2
                Layout.fillHeight: true
                Layout.fillWidth: true
                autoAccept: false
                text: Albums.search
                onAccepted: Albums.search = searchField.text
            }
        }

        footer: Item {
            id: footer_container
            height: 0

            PlayingBar {
                id: playing_bar
                height: 150
                width: parent.width

                currentCover: Player.current_image_url
                currentTrackName: Player.current_track_name
                currentArtist: Player.current_artist
                currentAlbum: Player.current_album
                playState: Player.play_state

                onPlay: Player.play()
                onPause: Player.pause()
                onStop: Player.stop()
                onNext: Player.next()
            }

            states: State {
                when: Player.play_state == "play" || Player.play_state == "pause"
                PropertyChanges {
                    target: footer_container
                    height: playing_bar.height
                }
            }

            transitions: Transition {
                NumberAnimation { properties: "height"; easing.type: Easing.InOutQuad }
            }
        }
        
        GridView {
            id: view
            model: Albums
            cellWidth: 227
            cellHeight: 280
            topMargin: 0
            bottomMargin: 0

            delegate: Column {
                id: albumView
                Kirigami.Theme.inherit: false
                Kirigami.Theme.colorSet: Kirigami.Theme.View
                width: 200

                Item {
                    AlbumCoverGridItem {
                        y: 27
                        x: 27
                        width: 200
                        height: 300
                        source: cover_url
                        title: model.name
                        artist: model.artist
                        onCoverDoubleClicked: Player.play_album(model.album_id)
                        onArtistClicked: Albums.search = model.artist
                    }
                }
            }
        }
    }
}
