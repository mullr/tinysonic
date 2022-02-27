use qmetaobject::{prelude::*, queued_callback, USER_ROLE};
use std::{borrow::BorrowMut, collections::HashMap};
use subsonic_client::AlbumListType;
use tokio::sync::mpsc::UnboundedSender;

use crate::comms;

#[derive(Default, Clone)]
pub struct Album {
    pub album_id: String,
    pub name: String,
    pub artist: String,
    pub cover_url: String,
}

#[allow(non_snake_case)]
#[derive(QObject)]
pub struct Albums {
    base: qt_base_class!(trait QAbstractListModel),
    comms_tx: UnboundedSender<comms::Request>,

    list: Vec<Album>,
    fetch: qt_method!(fn(&mut self)),

    #[allow(unused)]
    sort_order: qt_property!(QString; READ get_sort_order WRITE set_sort_order),
    sort_order_changed: qt_signal!(),
    album_list_type: AlbumListType,

    #[allow(unused)]
    search: qt_property!(QString; READ get_search WRITE set_search NOTIFY search_changed),
    search_changed: qt_signal!(),
    search_string: String,
}

// Required for qml_register_singleton_instance, but unused
impl Default for Albums {
    fn default() -> Self {
        unreachable!()
    }
}

impl Albums {
    pub fn new(comms_tx: UnboundedSender<comms::Request>) -> Self {
        Self {
            base: Default::default(),
            list: Default::default(),
            comms_tx,
            // add: Default::default(),
            fetch: Default::default(),

            sort_order: Default::default(),
            sort_order_changed: Default::default(),
            album_list_type: AlbumListType::Random,

            search: Default::default(),
            search_changed: Default::default(),
            search_string: "".into(),
        }
    }

    fn fetch(&mut self) {
        (self as &mut dyn QAbstractListModel).begin_reset_model();
        self.list.clear();
        (self as &mut dyn QAbstractListModel).end_reset_model();

        let self_ptr = QPointer::from(self as &Self);

        if self.search_string.is_empty() {
            self.comms_tx
                .send(comms::Request::Albums(
                    self.album_list_type.clone(),
                    Box::new(queued_callback(move |albums: Vec<Album>| {
                        self_ptr.as_pinned().borrow_mut().map(|self_| {
                            let mut self_mut = self_.borrow_mut();
                            self_mut.add_albums(albums);
                        });
                    })),
                ))
                .unwrap();
        } else {
            self.comms_tx
                .send(comms::Request::Search(
                    self.search_string.clone(),
                    Box::new(queued_callback(move |albums: Vec<Album>| {
                        self_ptr.as_pinned().borrow_mut().map(|self_| {
                            let mut self_mut = self_.borrow_mut();
                            self_mut.add_albums(albums);
                        });
                    })),
                ))
                .unwrap();
        }
    }

    fn add_albums(&mut self, albums: Vec<Album>) {
        let end = self.list.len();
        (self as &mut dyn QAbstractListModel).begin_insert_rows(end as i32, end as i32);
        self.list.extend(albums.into_iter());
        (self as &mut dyn QAbstractListModel).end_reset_model();
    }

    fn get_sort_order(&self) -> QString {
        let s = match self.album_list_type {
            AlbumListType::Random => "random",
            AlbumListType::Newest => "newest",
            AlbumListType::Highest => "highest",
            AlbumListType::Frequent => "frequent",
            AlbumListType::Recent => "recent",
            AlbumListType::AlphabeticalByName => "by_name",
            AlbumListType::AlphabeticalByArtist => "by_artist",
            AlbumListType::Starred => "starred",
            AlbumListType::ByYear { .. } => "",
            AlbumListType::ByGenre { .. } => "",
        };
        s.into()
    }

    fn set_sort_order(&mut self, order: QString) {
        let order: String = order.into();
        let alt = match order.as_str() {
            "random" => AlbumListType::Random,
            "newest" => AlbumListType::Newest,
            "highest" => AlbumListType::Highest,
            "frequent" => AlbumListType::Frequent,
            "recent" => AlbumListType::Recent,
            "by_name" => AlbumListType::AlphabeticalByName,
            "by_artist" => AlbumListType::AlphabeticalByArtist,
            "starred" => AlbumListType::Starred,
            _ => return,
        };

        if self.album_list_type != alt {
            self.album_list_type = alt;
            self.fetch()
        }
    }

    fn get_search(&self) -> QString {
        self.search_string.as_str().into()
    }

    fn set_search(&mut self, search: QString) {
        let search: String = search.into();
        if self.search_string != search {
            self.search_string = search;
            self.search_changed();
        }

        self.fetch();
    }
}

const ALBUM_NAME: i32 = USER_ROLE;
const ALBUM_ARTIST: i32 = USER_ROLE + 1;
const ALBUM_COVER_URL: i32 = USER_ROLE + 2;
const ALBUM_ID: i32 = USER_ROLE + 3;

impl QAbstractListModel for Albums {
    fn row_count(&self) -> i32 {
        self.list.len() as i32
    }
    fn data(&self, index: QModelIndex, role: i32) -> QVariant {
        let idx = index.row() as usize;
        if let Some(album) = self.list.get(idx) {
            match role {
                ALBUM_NAME => QString::from(album.name.as_str()).into(),
                ALBUM_ARTIST => QString::from(album.artist.as_str()).into(),
                ALBUM_COVER_URL => QString::from(album.cover_url.as_str()).into(),
                ALBUM_ID => QString::from(album.album_id.as_str()).into(),
                _ => QVariant::default(),
            }
        } else {
            QVariant::default()
        }
    }
    fn role_names(&self) -> HashMap<i32, QByteArray> {
        let mut map = HashMap::new();
        map.insert(ALBUM_NAME, "name".into());
        map.insert(ALBUM_ARTIST, "artist".into());
        map.insert(ALBUM_COVER_URL, "cover_url".into());
        map.insert(ALBUM_ID, "album_id".into());
        map
    }
}
