#![allow(unused)]

use bytes::Bytes;
use qmetaobject::{prelude::*, queued_callback};
use std::{
    borrow::BorrowMut,
    collections::VecDeque,
    io::Cursor,
    sync::{Arc, Mutex},
};
use symphonia::{
    core::{
        codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL},
        formats::{FormatOptions, FormatReader},
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
    },
    default::{get_codecs, get_probe},
};
use tokio::{
    sync::mpsc::{error::TryRecvError, unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::{spawn_blocking, JoinHandle},
};
use tracing::{debug, error, info, trace, warn, instrument};

use crate::{
    audio::{self, AudioCommand, AudioState, AudioThread},
    comms,
    player::TrackMetadata,
};

#[derive(Debug)]
pub struct PlmStatus {
    pub playing_track: Option<TrackMetadata>,
    pub audio_state: crate::audio::AudioState,
}

pub enum PlmCommand {
    // for ui -> plm
    SetStatusCallback(Box<dyn Fn(PlmStatus) + Send>),
    SetPlaylist(Vec<TrackMetadata>),
    Stop,
    Pause,
    Play,
    Next,

    // for comms -> plm
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
            Self::SetStatusCallback(_) => write!(f, "SetStatusCallback"),
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
            Self::AudioState { state } => f
                .debug_struct("AudioState")
                .field("state", state)
                .finish(),
        }
    }
}

/// It's the Playlist Manager
pub struct PlmTask {
    tx: UnboundedSender<PlmCommand>,
    rx: UnboundedReceiver<PlmCommand>,
    comms_tx: UnboundedSender<comms::Request>,
    audio_tx: UnboundedSender<AudioCommand>,
    audio_join_handle: JoinHandle<()>,
    playlist: VecDeque<(TrackMetadata, LoadStatus)>,
    status_callback: Option<Box<dyn Fn(PlmStatus) + Send>>,
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
    pub fn new(
        tx: UnboundedSender<PlmCommand>,
        rx: UnboundedReceiver<PlmCommand>,
        comms_tx: UnboundedSender<comms::Request>,
    ) -> Self {
        let (audio_tx, audio_rx) = tokio::sync::mpsc::unbounded_channel();
        let plm_tx_for_audio = tx.clone();
        let audio_join_handle =
            spawn_blocking(move || AudioThread::new(audio_rx, plm_tx_for_audio).run());

        Self {
            tx,
            rx,
            comms_tx,
            audio_tx,
            audio_join_handle,
            playlist: Default::default(),
            status_callback: None,
            audio_state: AudioState::Stopped,
            audio_playing_track_id: None,
        }
    }

    pub async fn run(mut self) {
        info!("Starting Playlist Manager task");
        while let Some(cmd) = self.rx.recv().await {
            debug!(
                cmd = format!("{cmd:?}").as_str(),
                "Playlist Manager command"
            );
            match cmd {
                PlmCommand::SetStatusCallback(cb) => {
                    self.status_callback = Some(cb);
                }
                PlmCommand::SetPlaylist(tracks) => {
                    self.audio_tx.send(AudioCommand::Stop);
                    self.playlist = tracks
                        .into_iter()
                        .map(|t| (t, LoadStatus::NotLoaded))
                        .collect();

                    self.load_as_needed();
                }

                PlmCommand::Stop => {
                    self.playlist.clear();
                    self.audio_tx.send(AudioCommand::Stop);
                }

                PlmCommand::Pause => {
                    self.audio_tx.send(AudioCommand::Pause);
                }

                PlmCommand::Play => {
                    self.audio_tx.send(AudioCommand::Play);
                }

                PlmCommand::Next => {
                    self.playlist.pop_front();
                    self.audio_tx.send(AudioCommand::Next);
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
        if let Some(f) = &self.status_callback {
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

            (f)(PlmStatus {
                playing_track,
                audio_state: self.audio_state,
            })
        }
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
                self.comms_tx.send(comms::Request::TrackData(
                    track.id.clone(),
                    // TODO can I change this to FnOnce?
                    Box::new(move |bytes| {
                        tx.send(PlmCommand::LoadTrackData {
                            track_id: track_id.clone(),
                            data: bytes,
                        });
                    }),
                ));
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
            .find(|(t, status)| t.id == track_id);
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
                    self.audio_tx.send(AudioCommand::EnqueueTrackData {
                        track_id: t.id.clone(),
                        data,
                    });
                    *status = LoadStatus::SentToAudioThread;
                }
            }
        }
    }
}
