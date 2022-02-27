use bytes::Bytes;
use subsonic_client::{AlbumListType, SubsonicAuth};
use tokio::sync::mpsc;

use crate::Config;

const GET_ALBUMS_WINDOW_SIZE: usize = 100;
const NUM_SEARCH_RESULTS: usize = 100;

pub enum Request {
    Albums(AlbumListType, Respond<Vec<super::albums::Album>>),
    AlbumTracks(String, Respond<Vec<super::player::TrackMetadata>>),
    Search(String, Respond<Vec<super::albums::Album>>),
    TrackData(String, Respond<Bytes>),
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Request::Albums(alt, _) => f.debug_tuple("GetAlbums").field(alt).finish(),
            Request::AlbumTracks(id, _) => f.debug_tuple("AlbumTracks").field(id).finish(),
            Request::Search(term, _) => f.debug_tuple("Search").field(term).finish(),
            Request::TrackData(id, _) => f.debug_tuple("TrackData").field(id).finish(),
        }
    }
}

type Respond<T> = Box<dyn Fn(T) + Send>;

pub async fn run(config: Config, mut rx: mpsc::UnboundedReceiver<Request>) {
    let client = subsonic_client::Client::new(
        SubsonicAuth::new(&config.username, &config.password),
        &config.url,
    );

    while let Some(msg) = rx.recv().await {
        match msg {
            Request::Albums(album_list_type, respond) => {
                let window_size = if album_list_type == AlbumListType::Random {
                    18
                } else {
                    GET_ALBUMS_WINDOW_SIZE
                };

                let mut offset = 0;
                loop {
                    let children = client
                        .get_album_list(
                            album_list_type.clone(),
                            Some(window_size),
                            Some(offset),
                            None,
                        )
                        .await
                        .unwrap();

                    let albums = children
                        .into_iter()
                        .map(|child| super::albums::Album {
                            album_id: child.id,
                            name: child.title,
                            artist: child.artist.unwrap_or_else(|| "".to_string()),
                            cover_url: match child.cover_art {
                                Some(art_id) => client
                                    .cover_art_url(&art_id, Some(200))
                                    .unwrap()
                                    .to_string(),
                                None => "".to_string(),
                            },
                        })
                        .collect::<Vec<_>>();
                    let fetched_count = albums.len();
                    respond(albums);

                    if fetched_count < GET_ALBUMS_WINDOW_SIZE {
                        break;
                    } else if album_list_type == AlbumListType::Random {
                        break;
                    } else {
                        offset += GET_ALBUMS_WINDOW_SIZE;
                    }
                }
            }
            Request::AlbumTracks(id, respond) => {
                let album = client.get_album(&id).await.unwrap();
                let md = album.album_id3;
                let mut tracks = vec![];
                for child in album.songs.into_iter() {
                    let stream_url = client
                        .stream_url(&child.id, None, None, None, None, None)
                        .unwrap()
                        .to_string();
                    tracks.push(super::player::TrackMetadata {
                        id: child.id,
                        name: child.title,
                        artist: child.artist.unwrap_or_else(|| "".to_string()),
                        album: md.name.clone(),
                        stream_url,
                        cover_url: match child.cover_art {
                            Some(art_id) => {
                                client.cover_art_url(&art_id, None).unwrap().to_string()
                            }
                            None => "".to_string(),
                        },
                    });
                }

                respond(tracks);
            }
            Request::Search(query, respond) => {
                let res = client
                    .search2(
                        query,
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

                let albums = res
                    .albums
                    .into_iter()
                    .map(|child| super::albums::Album {
                        album_id: child.id,
                        name: child.title,
                        artist: child.artist.unwrap_or_else(|| "".to_string()),
                        cover_url: match child.cover_art {
                            Some(art_id) => client
                                .cover_art_url(&art_id, Some(200))
                                .unwrap()
                                .to_string(),
                            None => "".to_string(),
                        },
                    })
                    .collect::<Vec<_>>();
                respond(albums);
            }
            Request::TrackData(id, respond) => {
                let bytes = client
                    .stream(&id, None, None, None, None, None)
                    .await
                    .unwrap();
                respond(bytes);
            }
        }
    }
}
