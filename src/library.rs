use bytes::Bytes;
use subsonic_client::{AlbumListType, SubsonicAuth};

use crate::Config;

const GET_ALBUMS_WINDOW_SIZE: usize = 100;
const NUM_SEARCH_RESULTS: usize = 100;

#[derive(Debug, Default, Clone)]
pub struct Album {
    pub album_id: String,
    pub name: String,
    pub artist: String,
    pub cover_url: String,
}

#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct TrackMetadata {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub stream_url: String,
    pub cover_url: String,
}

pub struct Library {
    client: subsonic_client::Client,
}

impl Library {
    pub fn new(config: Config) -> Library {
        Library {
            client: subsonic_client::Client::new(
                SubsonicAuth::new(&config.username, &config.password),
                &config.url,
            ),
        }
    }

    pub async fn list_albums(&self, album_list_type: AlbumListType) -> Vec<Album> {
        let window_size = if album_list_type == AlbumListType::Random {
            18
        } else {
            GET_ALBUMS_WINDOW_SIZE
        };

        let mut offset = 0;
        let mut albums = vec![];
        loop {
            let mut fetched_count = 0;
            for child in self
                .client
                .get_album_list(
                    album_list_type.clone(),
                    Some(window_size),
                    Some(offset),
                    None,
                )
                .await
                .unwrap()
            {
                fetched_count += 1;
                albums.push(Album {
                    album_id: child.id,
                    name: child.title,
                    artist: child.artist.unwrap_or_else(|| "".to_string()),
                    cover_url: match child.cover_art {
                        Some(art_id) => self
                            .client
                            .cover_art_url(&art_id, Some(200))
                            .unwrap()
                            .to_string(),
                        None => "".to_string(),
                    },
                });
            }

            if fetched_count < GET_ALBUMS_WINDOW_SIZE {
                break;
            } else if album_list_type == AlbumListType::Random {
                break;
            } else {
                offset += GET_ALBUMS_WINDOW_SIZE;
            }
        }

        albums
    }

    pub async fn search(&self, search: String) -> Vec<Album> {
        let res = self
            .client
            .search2(
                search,
                NUM_SEARCH_RESULTS, // artist count
                0,                  // artist offset
                NUM_SEARCH_RESULTS, // album count
                0,                  // album offset
                0,                  // song count
                0,                  // song offset
                None,               // folder id
            )
            .await
            .unwrap();

        // TODO handle artists
        res.albums
            .into_iter()
            .map(|child| Album {
                album_id: child.id,
                name: child.title,
                artist: child.artist.unwrap_or_else(|| "".to_string()),
                cover_url: match child.cover_art {
                    Some(art_id) => self
                        .client
                        .cover_art_url(&art_id, Some(200))
                        .unwrap()
                        .to_string(),
                    None => "".to_string(),
                },
            })
            .collect::<Vec<_>>()
    }

    pub async fn track_data(&self, track_id: &str) -> Bytes {
        self.client
            .stream(&track_id, None, None, None, None, None)
            .await
            .unwrap()
    }

    pub async fn album_tracks(&self, id: &str) -> Vec<TrackMetadata> {
        let album = self.client.get_album(id).await.unwrap();
        let md = album.album_id3;
        let mut tracks = vec![];
        for child in album.songs.into_iter() {
            let stream_url = self
                .client
                .stream_url(&child.id, None, None, None, None, None)
                .unwrap()
                .to_string();
            tracks.push(TrackMetadata {
                id: child.id,
                name: child.title,
                artist: child.artist.unwrap_or_else(|| "".to_string()),
                album: md.name.clone(),
                stream_url,
                cover_url: match child.cover_art {
                    Some(art_id) => self
                        .client
                        .cover_art_url(&art_id, None)
                        .unwrap()
                        .to_string(),
                    None => "".to_string(),
                },
            });
        }

        tracks
    }
}
