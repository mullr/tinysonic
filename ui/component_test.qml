import QtQuick 2.15

Column {
    width: 1000
    Row {
        height: 300
        spacing: 20
        padding: 40

        AlbumCoverGridItem {
            width: 200
            height: 300
            source: "http://10.0.0.3:4533/rest/getCoverArt?u=admin&t=f12f6b4c3bf23e1664d4487ce64506b3&s=6998a5&f=json&v=1.8.0&c=NavidromeUI&id=3e35f8ea44a6caf88b16b8c69fccdb7f&_=2021-07-17T19%3A31%3A54.207889161Z"
            title: "kpoppy"
            artist: "Twice"
            artist_id: "1234"
            onCoverDoubleClicked: console.log("Cover doubleclicked")
            onArtistClicked: console.log("Artist clicked")
        }

        AlbumCoverGridItem {
            width: 200
            height: 300
            source: "http://10.0.0.3:4533/rest/getCoverArt?u=admin&t=f12f6b4c3bf23e1664d4487ce64506b3&s=6998a5&f=json&v=1.8.0&c=NavidromeUI&id=0e9fe32eedbb07ecc12369a533c459b1&_=2021-07-17T19%3A17%3A07.026427529Z"
            title: "Jazzy"
            artist: "Glenn Miller"
        }

        AlbumCoverGridItem {
            width: 200
            height: 300
            source: "/home/mullr/storage/Music/シートベルツ/Cowboy Bebop: CD-Box/cover.jpg"
            title: "Jazzy and Japanese"
            artist: "The seatbelts"
        }

        AlbumCoverGridItem {
            width: 200
            height: 200
            title: "I have no cover and I must scream"
            artist: "Existential dread, the album"
        }
    }

    Row {
        Rectangle {
            height: 200
            width: 1000
            border.width: 2

            PlayingBar {
                height: 200
                width: 1000

                currentCover: "http://10.0.0.3:4533/rest/getCoverArt?u=admin&t=f12f6b4c3bf23e1664d4487ce64506b3&s=6998a5&f=json&v=1.8.0&c=NavidromeUI&id=3e35f8ea44a6caf88b16b8c69fccdb7f&_=2021-07-17T19%3A31%3A54.207889161Z"
                currentTrackName: "some track"
                currentArtist: "Popular artist"
                currentAlbum: "A work of seminal import"
            }
        }
    }
}
