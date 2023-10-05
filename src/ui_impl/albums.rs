use std::sync::Arc;

use subsonic_client::AlbumListType;
use tokio::sync::Mutex;

use crate::{
    library::{Album, Library},
    ui_interface::{AlbumsEmitter, AlbumsList, AlbumsTrait},
};

pub struct Albums {
    emit: AlbumsEmitter,
    model: AlbumsList,

    library: Option<Arc<Library>>,

    list: Vec<Album>,
    incoming: Arc<Mutex<Option<Vec<Album>>>>,
    album_list_type: AlbumListType,
    search: String,
}

impl AlbumsTrait for Albums {
    fn new(emit: AlbumsEmitter, model: AlbumsList) -> Self {
        Self {
            emit,
            model,
            library: None,
            list: vec![],
            incoming: Arc::new(Mutex::new(None)),
            album_list_type: AlbumListType::Random,
            search: Default::default(),
        }
    }

    fn emit(&mut self) -> &mut AlbumsEmitter {
        &mut self.emit
    }

    fn set_library(&mut self, p: u64) {
        unsafe {
            let arc_ref = &*(p as *const Arc<Library>);
            self.library = Some(arc_ref.clone());
        }
    }

    fn sort_order(&self) -> &str {
        match self.album_list_type {
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
        }
    }

    fn set_sort_order(&mut self, order: String) {
        let order: String = order.into();
        let new_album_list_type = match order.as_str() {
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

        if self.album_list_type != new_album_list_type {
            self.album_list_type = new_album_list_type;
            self.emit.sort_order_changed();
            self.fetch()
        }
    }

    fn search(&self) -> &str {
        &self.search
    }

    fn set_search(&mut self, value: String) {
        if self.search != value {
            self.search = value;
            self.emit.search_changed();
            self.fetch();
        }
    }

    fn row_count(&self) -> usize {
        self.list.len()
    }

    fn album_id(&self, index: usize) -> &str {
        self.list
            .get(index)
            .map(|a| a.album_id.as_str())
            .unwrap_or_default()
    }

    fn artist(&self, index: usize) -> &str {
        self.list
            .get(index)
            .map(|a| a.artist.as_str())
            .unwrap_or_default()
    }

    fn cover_url(&self, index: usize) -> &str {
        self.list
            .get(index)
            .map(|a| a.cover_url.as_str())
            .unwrap_or_default()
    }

    fn name(&self, index: usize) -> &str {
        self.list
            .get(index)
            .map(|a| a.name.as_str())
            .unwrap_or_default()
    }

    /// Fetch new albums from the library
    fn fetch(&mut self) {
        self.model.begin_reset_model();
        self.list.clear();
        self.model.end_reset_model();

        let library = self.library.as_ref().unwrap().clone();
        let mut emit = self.emit.clone();
        let incoming = self.incoming.clone();

        if self.search.is_empty() {
            let album_list_type = self.album_list_type.clone();
            tokio::spawn(async move {
                let albums = library.list_albums(album_list_type).await;
                *incoming.lock().await = Some(albums);
                emit.invoke_handle_incoming_list();
            });
        } else {
            let search = self.search.clone();

            tokio::spawn(async move {
                let albums = library.search(search).await;
                *incoming.lock().await = Some(albums);
                emit.invoke_handle_incoming_list();
            });
        }
    }

    /// The album list was updated. Dispatched on the ui thread by `fetch`.
    fn handle_incoming_list(&mut self) {
        if let Some(albums) = self.incoming.blocking_lock().take() {
            self.model.begin_reset_model();
            self.list = albums;
            self.model.end_reset_model();
        }
    }
}
