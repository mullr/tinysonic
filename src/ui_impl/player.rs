use std::sync::Arc;
use tokio::sync::watch;

use crate::{
    audio::AudioState,
    library::Library,
    plm::{PlaylistManager, PlmStatus},
    ui_interface::{PlayerEmitter, PlayerTrait},
};

pub struct Player {
    emit: PlayerEmitter,
    library: Option<Arc<Library>>,
    plm: Option<Arc<PlaylistManager>>,
    plm_status: PlmStatus,
    plm_status_rx: watch::Receiver<PlmStatus>,
}

impl Player {
    fn library(&self) -> &Arc<Library> {
        self.library.as_ref().unwrap()
    }

    fn plm(&self) -> &Arc<PlaylistManager> {
        self.plm.as_ref().unwrap()
    }
}

impl PlayerTrait for Player {
    fn new(emit: PlayerEmitter) -> Self {
        let (_, initial_plm_status_rx) = watch::channel(PlmStatus::default());

        Player {
            emit,
            library: None,
            plm: None,
            plm_status: Default::default(),
            plm_status_rx: initial_plm_status_rx,
        }
    }

    fn emit(&mut self) -> &mut PlayerEmitter {
        &mut self.emit
    }

    fn set_library(&mut self, p: u64) {
        unsafe {
            let arc_ref = &*(p as *const Arc<Library>);
            self.library = Some(arc_ref.clone());
        }
    }

    fn set_plm(&mut self, p: u64) {
        let mut emit = self.emit.clone();
        let plm_ref = unsafe { &*(p as *const Arc<PlaylistManager>) };
        self.plm = Some(plm_ref.clone());
        self.plm_status_rx = self.plm().status_rx();

        let mut poll_status = self.plm_status_rx.clone();
        tokio::spawn(async move {
            loop {
                let _ = poll_status.changed().await;
                emit.invoke_handle_incoming_plm_status();
            }
        });

        self.handle_incoming_plm_status();
    }

    fn handle_incoming_plm_status(&mut self) {
        let mut new_plm_status = self.plm_status_rx.borrow().clone();
        std::mem::swap(&mut self.plm_status, &mut new_plm_status);

        if new_plm_status.playing_track != self.plm_status.playing_track {
            // could be a little more fine-grained...
            self.emit.current_album_changed();
            self.emit.current_artist_changed();
            self.emit.current_track_name_changed();
            self.emit.current_image_url_changed();
        }
        if new_plm_status.audio_state != self.plm_status.audio_state {
            self.emit.play_state_changed();
        }
    }

    fn current_album(&self) -> &str {
        &self
            .plm_status
            .playing_track
            .as_ref()
            .map(|tm| tm.album.as_str())
            .unwrap_or_default()
    }

    fn current_artist(&self) -> &str {
        &self
            .plm_status
            .playing_track
            .as_ref()
            .map(|tm| tm.artist.as_str())
            .unwrap_or_default()
    }

    fn current_image_url(&self) -> &str {
        &self
            .plm_status
            .playing_track
            .as_ref()
            .map(|tm| tm.cover_url.as_str())
            .unwrap_or_default()
    }

    fn current_track_name(&self) -> &str {
        &self
            .plm_status
            .playing_track
            .as_ref()
            .map(|tm| tm.name.as_str())
            .unwrap_or_default()
    }

    fn play_state(&self) -> &str {
        match self.plm_status.audio_state {
            AudioState::Stopped => "stop".into(),
            AudioState::Playing | AudioState::WillPlayWhenDataArrives => "play".into(),
            AudioState::Paused => "pause".into(),
        }
    }

    fn next(&mut self) -> () {
        self.plm().next();
    }

    fn pause(&mut self) -> () {
        self.plm().pause();
    }

    fn play(&mut self) -> () {
        self.plm().play();
    }

    fn stop(&mut self) -> () {
        self.plm().stop();
    }

    fn play_album(&mut self, id: String) -> () {
        let library = self.library().clone();
        let plm = self.plm().clone();

        tokio::spawn(async move {
            let tracks = library.album_tracks(&id).await;
            plm.set_playlist(tracks);
            plm.play();
        });
    }
}
