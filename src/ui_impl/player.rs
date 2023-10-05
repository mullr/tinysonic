use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::UnboundedSender;

use crate::{
    audio::AudioState,
    library::Library,
    plm::{PlmCommand, PlmStatus},
    ui_interface::{PlayerEmitter, PlayerTrait},
};

pub struct Player {
    emit: PlayerEmitter,
    library: Option<Arc<Library>>,
    plm_tx: Option<UnboundedSender<PlmCommand>>,
    plm_status: PlmStatus,
    incoming_plm_status: Arc<Mutex<Option<PlmStatus>>>,
}

impl Player {
    fn library(&self) -> &Arc<Library> {
        self.library.as_ref().unwrap()
    }

    fn plm_tx(&self) -> &UnboundedSender<PlmCommand> {
        self.plm_tx.as_ref().unwrap()
    }
}

impl PlayerTrait for Player {
    fn new(emit: PlayerEmitter) -> Self {
        Player {
            emit,
            library: None,
            plm_tx: None,
            plm_status: Default::default(),
            incoming_plm_status: Arc::new(Mutex::new(None)),
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

    fn set_plm_tx(&mut self, p: u64) {
        let incoming_plm_status = self.incoming_plm_status.clone();
        let mut emit = self.emit.clone();
        let sender_ref = unsafe { &*(p as *const UnboundedSender<PlmCommand>) };

        sender_ref
            .send(PlmCommand::SetStatusCallback(Box::new(move |status| {
                let mut guard = incoming_plm_status.lock().unwrap();
                *guard = Some(status);
                emit.invoke_handle_incoming_plm_status();
            })))
            .unwrap();
        self.plm_tx = Some(sender_ref.clone());
    }

    fn handle_incoming_plm_status(&mut self) {
        if let Some(mut new_plm_status) = self.incoming_plm_status.lock().unwrap().take() {
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
        self.plm_tx().send(PlmCommand::Next).unwrap();
    }

    fn pause(&mut self) -> () {
        self.plm_tx().send(PlmCommand::Pause).unwrap();
    }

    fn play(&mut self) -> () {
        self.plm_tx().send(PlmCommand::Play).unwrap();
    }

    fn stop(&mut self) -> () {
        self.plm_tx().send(PlmCommand::Stop).unwrap();
    }

    fn play_album(&mut self, id: String) -> () {
        let library = self.library().clone();
        let plm_tx = self.plm_tx().clone();
        tokio::spawn(async move {
            let tracks = library.album_tracks(&id).await;
            plm_tx.send(PlmCommand::SetPlaylist(tracks)).unwrap();
            plm_tx.send(PlmCommand::Play).unwrap();
        });
    }
}
