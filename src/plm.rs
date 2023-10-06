use bytes::Bytes;
use std::{collections::VecDeque, sync::Arc};
use tokio::{
    sync::{mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender}, watch},
    task::{spawn_blocking, JoinHandle},
};
use tracing::{debug, error, info};

use crate::{
    audio::{self, AudioCommand, AudioState, AudioThread},
    library::{Library, TrackMetadata},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlmStatus {
    pub playing_track: Option<TrackMetadata>,
    pub audio_state: crate::audio::AudioState,
}

pub struct PlaylistManager {
    tx: UnboundedSender<PlmCommand>,
    status_rx: watch::Receiver<PlmStatus>
}

impl PlaylistManager {
    pub fn new(library: Arc<Library>) -> PlaylistManager {
        let (tx, rx) = unbounded_channel::<PlmCommand>();
        let (status_tx, status_rx) = watch::channel(PlmStatus::default());

        let tx2 = tx.clone();
        tokio::spawn(async move { PlmTask::new(tx2, rx, status_tx, library).run().await });
        PlaylistManager { tx, status_rx }
    }

    pub fn status_rx(&self) -> watch::Receiver<PlmStatus> {
        self.status_rx.clone()
    }

    pub fn set_playlist(&self, tracks: Vec<TrackMetadata>) {
        self.tx.send(PlmCommand::SetPlaylist(tracks)).unwrap();
    }

    pub fn stop(&self) {
        self.tx.send(PlmCommand::Stop).unwrap();
    }

    pub fn pause(&self) {
        self.tx.send(PlmCommand::Pause).unwrap();
    }

    pub fn play(&self) {
        self.tx.send(PlmCommand::Play).unwrap();
    }

    pub fn next(&self) {
        self.tx.send(PlmCommand::Next).unwrap();
    }
}

pub enum PlmCommand {
    // for ui -> plm
    SetPlaylist(Vec<TrackMetadata>),
    Stop,
    Pause,
    Play,
    Next,

    // for library -> plm
    LoadTrackData { track_id: String, data: Bytes },

    // for audio -> plm
    AudioSkippedTrack { track_id: String },
    AudioFinishedTrack { track_id: String },
    AudioPlayingTrack { track_id: String },
    AudioState { state: audio::AudioState },
}

impl std::fmt::Debug for PlmCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetPlaylist(_) => write!(f, "SetPlaylist"),
            Self::Stop => write!(f, "Stop"),
            Self::Pause => write!(f, "Pause"),
            Self::Play => write!(f, "Play"),
            Self::Next => write!(f, "Next"),

            Self::LoadTrackData { track_id, .. } => f
                .debug_struct("LoadTrackData")
                .field("track_id", track_id)
                .finish(),
            Self::AudioSkippedTrack { track_id } => f
                .debug_struct("AudioSkippedTrack")
                .field("track_id", track_id)
                .finish(),
            Self::AudioFinishedTrack { track_id } => f
                .debug_struct("AudioFinishedTrack")
                .field("track_id", track_id)
                .finish(),
            Self::AudioPlayingTrack { track_id } => f
                .debug_struct("AudioPlayingTrack")
                .field("track_id", track_id)
                .finish(),
            Self::AudioState { state } => {
                f.debug_struct("AudioState").field("state", state).finish()
            }
        }
    }
}

/// It's the Playlist Manager
struct PlmTask {
    tx: UnboundedSender<PlmCommand>,
    rx: UnboundedReceiver<PlmCommand>,
    library: Arc<Library>,
    audio_tx: UnboundedSender<AudioCommand>,
    _audio_join_handle: JoinHandle<()>,
    playlist: VecDeque<(TrackMetadata, LoadStatus)>,
    status_tx: watch::Sender<PlmStatus>,
    audio_state: AudioState,
    audio_playing_track_id: Option<String>,
}

#[derive(Debug)]
enum LoadStatus {
    NotLoaded,
    Loading,
    SentToAudioThread,
}

impl PlmTask {
    fn new(
        tx: UnboundedSender<PlmCommand>,
        rx: UnboundedReceiver<PlmCommand>,
        status_tx: watch::Sender<PlmStatus>,
        library: Arc<Library>,
    ) -> Self {
        let (audio_tx, audio_rx) = tokio::sync::mpsc::unbounded_channel();
        let plm_tx_for_audio = tx.clone();
        let audio_join_handle =
            spawn_blocking(move || AudioThread::new(audio_rx, plm_tx_for_audio).run());

        Self {
            tx,
            rx,
            library,
            audio_tx,
            _audio_join_handle: audio_join_handle,
            playlist: Default::default(),
            status_tx,
            audio_state: AudioState::Stopped,
            audio_playing_track_id: None,
        }
    }

    async fn run(mut self) {
        info!("Starting Playlist Manager task");
        while let Some(cmd) = self.rx.recv().await {
            debug!(
                cmd = format!("{cmd:?}").as_str(),
                "Playlist Manager command"
            );
            match cmd {
                PlmCommand::SetPlaylist(tracks) => {
                    self.audio_tx.send(AudioCommand::Stop).unwrap();
                    self.playlist = tracks
                        .into_iter()
                        .map(|t| (t, LoadStatus::NotLoaded))
                        .collect();

                    self.load_as_needed();
                }

                PlmCommand::Stop => {
                    self.playlist.clear();
                    self.audio_tx.send(AudioCommand::Stop).unwrap();
                }

                PlmCommand::Pause => {
                    self.audio_tx.send(AudioCommand::Pause).unwrap();
                }

                PlmCommand::Play => {
                    self.audio_tx.send(AudioCommand::Play).unwrap();
                }

                PlmCommand::Next => {
                    self.playlist.pop_front();
                    self.audio_tx.send(AudioCommand::Next).unwrap();
                    self.load_as_needed();
                }

                PlmCommand::LoadTrackData { track_id, data } => {
                    self.load_track_data(track_id, data);
                    self.load_as_needed();
                }

                PlmCommand::AudioSkippedTrack { track_id }
                | PlmCommand::AudioFinishedTrack { track_id } => {
                    self.playlist.retain(|t| t.0.id != track_id);
                    self.load_as_needed();
                    // SHOULD be this, but will be corrected later if it's wrong.
                    if let Some((t, _)) = self.playlist.get(0) {
                        self.audio_playing_track_id = Some(t.id.clone());
                        self.publish_status();
                    }
                }

                PlmCommand::AudioPlayingTrack { track_id } => {
                    self.audio_playing_track_id = Some(track_id);
                    self.publish_status();
                }

                PlmCommand::AudioState { state } => {
                    self.audio_state = state;
                    self.publish_status();
                }
            }
        }
    }

    /// Try and get the first two entries in the playlist (now
    /// playing, and next) to the 'SentToAudioThread' state.
    fn load_as_needed(&mut self) {
        self.load_pl_index(0);

        // Only try to load 'up next' if 'playing' is already fully loaded.
        if let Some((_, LoadStatus::SentToAudioThread)) = self.playlist.get(0) {
            self.load_pl_index(1);
        }
    }

    fn publish_status(&mut self) {
        debug!("Publishing plm status");
        let playing_track = self
            .audio_playing_track_id
            .as_ref()
            .and_then(|playing_track_id| {
                self.playlist.iter().find_map(|(t, _)| {
                    if &t.id == playing_track_id {
                        Some(t.clone())
                    } else {
                        None
                    }
                })
            });

        let status = PlmStatus {
            playing_track,
            audio_state: self.audio_state,
        };

        self.status_tx.send(status).unwrap();
    }

    fn load_pl_index(&mut self, index: usize) {
        let (track, load_status) = match self.playlist.get_mut(index) {
            Some(t) => t,
            None => return,
        };

        match load_status {
            LoadStatus::NotLoaded => {
                let tx = self.tx.clone();
                let track_id = track.id.clone();
                let library = self.library.clone();
                tokio::spawn(async move {
                    let data = library.track_data(&track_id).await;
                    tx.send(PlmCommand::LoadTrackData {
                        track_id: track_id.clone(),
                        data,
                    })
                    .unwrap();
                });

                *load_status = LoadStatus::Loading;
            }

            // we're doing it...
            LoadStatus::Loading => (),

            // we did it!
            LoadStatus::SentToAudioThread => (),
        }
    }

    fn load_track_data(&mut self, track_id: String, data: Bytes) {
        let entry = self
            .playlist
            .iter_mut()
            .find(|(t, _status)| t.id == track_id);
        if let Some((t, status)) = entry {
            match status {
                LoadStatus::NotLoaded | LoadStatus::SentToAudioThread => {
                    error!(
                        status = format!("{:?}", status).as_str(),
                        track_id = t.id.as_str(),
                        "Unexpected load status"
                    );
                }
                LoadStatus::Loading => {
                    self.audio_tx
                        .send(AudioCommand::EnqueueTrackData {
                            track_id: t.id.clone(),
                            data,
                        })
                        .unwrap();
                    *status = LoadStatus::SentToAudioThread;
                }
            }
        }
    }
}
