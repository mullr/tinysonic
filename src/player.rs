#![allow(unused)]

use bytes::Bytes;
use qmetaobject::{prelude::*, queued_callback};
use std::{
    borrow::BorrowMut,
    cell::RefCell,
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
use tracing::{debug, error, info, trace, warn};

use crate::{
    audio::AudioState,
    comms,
    plm::{self, PlmCommand, PlmStatus},
};

#[derive(Default, Clone, Debug)]
pub struct TrackMetadata {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub stream_url: String,
    pub cover_url: String,
}

#[allow(non_snake_case)]
#[derive(QObject)]
pub struct Player {
    base: qt_base_class!(trait QObject),
    comms_tx: UnboundedSender<comms::Request>,
    plm_tx: UnboundedSender<PlmCommand>,

    play_state: qt_property!(QString; NOTIFY play_state_changed),
    play_state_changed: qt_signal!(),

    current_artist: qt_property!(QString; NOTIFY current_artist_changed),
    current_artist_changed: qt_signal!(),

    current_image_url: qt_property!(QString; NOTIFY current_image_url_changed),
    current_image_url_changed: qt_signal!(),

    current_album: qt_property!(QString; NOTIFY current_album_changed),
    current_album_changed: qt_signal!(),

    current_track_name: qt_property!(QString; NOTIFY current_track_name_changed),
    current_track_name_changed: qt_signal!(),

    setup: qt_method!(fn(&mut self)),
    play_album: qt_method!(fn(&mut self, id: QString)),
    play: qt_method!(fn(&mut self)),
    pause: qt_method!(fn(&mut self)),
    next: qt_method!(fn(&mut self)),
    stop: qt_method!(fn(&mut self)),
}

// Required for qml_register_singleton_instance, but unused
impl Default for Player {
    fn default() -> Self {
        unreachable!()
    }
}

impl Player {
    pub fn new(
        comms_tx: UnboundedSender<comms::Request>,
        plm_tx: UnboundedSender<plm::PlmCommand>,
    ) -> Self {
        let player = Self {
            base: Default::default(),
            comms_tx,
            plm_tx,

            play_state: Default::default(),
            play_state_changed: Default::default(),
            current_artist: Default::default(),
            current_artist_changed: Default::default(),
            current_image_url: Default::default(),
            current_image_url_changed: Default::default(),
            current_album: Default::default(),
            current_album_changed: Default::default(),
            current_track_name: Default::default(),
            current_track_name_changed: Default::default(),

            setup: Default::default(),
            play_album: Default::default(),
            play: Default::default(),
            pause: Default::default(),
            next: Default::default(),
            stop: Default::default(),
        };

        player
    }

    // call me from qml, so we're definitely already pinned
    pub fn setup(&mut self) {
        let self_ptr = QPointer::from(self as &Self);
        let cb = Box::new(queued_callback(move |status: PlmStatus| {
            self_ptr.as_pinned().borrow_mut().map(|self_| {
                let mut self_mut = self_.borrow_mut();
                self_mut.update_status(status)
            });
        }));

        self.plm_tx.send(PlmCommand::SetStatusCallback(cb));
    }

    fn update_status(&mut self, status: PlmStatus) {
        // TODO can I use an enum for this with qml?
        self.play_state = match status.audio_state {
            AudioState::Stopped => "stop".into(),
            AudioState::Playing | AudioState::WillPlayWhenDataArrives => "play".into(),
            AudioState::Paused => "pause".into(),
        };

        match status.playing_track {
            Some(t) => {
                self.current_artist = t.artist.into();
                self.current_image_url = t.cover_url.into();
                self.current_album = t.album.into();
                self.current_track_name = t.name.into();
            }
            None => {
                // This is disabled so the artwork and name stays on the bottom bar while it slides away.

                // self.current_artist = "".into();
                // self.current_image_url = "".into();
                // self.current_album = "".into();
                // self.current_track_name = "".into();
            }
        }

        self.play_state_changed();
        self.current_artist_changed();
        self.current_image_url_changed();
        self.current_album_changed();
        self.current_track_name_changed();
    }

    fn clear_current_album_status(&mut self) {
        self.current_artist = "".into();
        self.current_image_url = "".into();
        self.current_album = "".into();
        self.current_track_name = "".into();

        self.current_artist_changed();
        self.current_image_url_changed();
        self.current_album_changed();
        self.current_track_name_changed();
    }

    fn play_album(&mut self, id: QString) {
        let self_ptr = QPointer::from(self as &Self);
        let id: String = id.into();
        info!(id = id.as_str(), "Play album");
        self.comms_tx
            .send(comms::Request::AlbumTracks(
                id,
                Box::new(queued_callback(move |tracks: Vec<TrackMetadata>| {
                    self_ptr.as_pinned().borrow_mut().map(|self_| {
                        let mut self_mut = self_.borrow_mut();
                        self_mut.plm_tx.send(PlmCommand::SetPlaylist(tracks));
                        self_mut.plm_tx.send(PlmCommand::Play);
                    });
                })),
            ))
            .unwrap();
        self.clear_current_album_status();
    }

    fn play(&mut self) {
        self.plm_tx.send(PlmCommand::Play);
    }

    fn pause(&mut self) {
        self.plm_tx.send(PlmCommand::Pause);
    }

    fn next(&mut self) {
        self.plm_tx.send(PlmCommand::Next);
    }

    fn stop(&mut self) {
        self.plm_tx.send(PlmCommand::Stop);
    }
}
