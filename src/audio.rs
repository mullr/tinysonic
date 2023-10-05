use bytes::Bytes;
use std::{collections::VecDeque, io::Cursor};
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
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, info, warn};

use crate::plm::PlmCommand;

pub enum AudioCommand {
    EnqueueTrackData { track_id: String, data: Bytes },
    Stop,
    Pause,
    Play,
    Next,
}

#[derive(Eq, PartialEq, Debug, Copy, Clone, Default)]
pub enum AudioState {
    #[default]
    Stopped,
    Playing,
    WillPlayWhenDataArrives,
    Paused,
}

pub struct AudioThread {
    rx: UnboundedReceiver<AudioCommand>,
    plm_tx: UnboundedSender<PlmCommand>,
    state: AudioState,
    queue: VecDeque<(String, Bytes)>,
    playing_state_data: Option<PlayingStateData>,
}

impl AudioThread {
    pub fn new(
        rx: UnboundedReceiver<AudioCommand>,
        notify_tx: UnboundedSender<PlmCommand>,
    ) -> Self {
        Self {
            rx,
            plm_tx: notify_tx,
            state: AudioState::Stopped,
            queue: Default::default(),
            playing_state_data: None,
        }
    }

    pub fn run(mut self) {
        info!("Starting Audio Thread");
        let mut last_state = self.state;

        loop {
            if last_state != self.state {
                debug!(
                    old_state = format!("{:?}", last_state).as_str(),
                    new_state = format!("{:?}", self.state).as_str(),
                    "Audio thread state transition"
                );
                last_state = self.state;

                if self.state == AudioState::Playing {
                    if let Some(psd) = &self.playing_state_data {
                        self.plm_tx
                            .send(PlmCommand::AudioPlayingTrack {
                                track_id: psd.track_id.clone(),
                            })
                            .unwrap();
                    }
                }
                self.plm_tx
                    .send(PlmCommand::AudioState { state: self.state })
                    .unwrap();
            }

            // do a non-blocking recieve when we're in a playing state
            let recv_res = if self.state == AudioState::Playing {
                self.rx.try_recv()
            } else {
                match self.rx.blocking_recv() {
                    Some(cmd) => Ok(cmd),
                    None => Err(TryRecvError::Disconnected),
                }
            };

            match recv_res {
                Ok(cmd) => match cmd {
                    AudioCommand::EnqueueTrackData { track_id, data } => {
                        self.queue.push_back((track_id, data));
                        if self.state == AudioState::WillPlayWhenDataArrives {
                            self.state = self.play_next_track();
                        }
                    }

                    AudioCommand::Stop => {
                        self.queue.clear();
                        self.playing_state_data = None;
                        self.state = AudioState::Stopped;
                    }

                    AudioCommand::Pause => match self.state {
                        AudioState::Stopped => (),
                        AudioState::Playing
                        | AudioState::WillPlayWhenDataArrives
                        | AudioState::Paused => self.state = AudioState::Paused,
                    },

                    AudioCommand::Play => match self.state {
                        AudioState::Stopped => {
                            self.state = self.play_next_track();
                        }
                        AudioState::Playing => (),
                        AudioState::WillPlayWhenDataArrives => (),
                        AudioState::Paused => {
                            self.state = AudioState::Playing;
                        }
                    },

                    AudioCommand::Next => {
                        // TODO update playing state data
                        // self.playing_state_data = None;
                        self.queue.pop_front();
                        self.play_next_track();
                        // if self.queue.is_empty() {
                        //     self.playing_state_data = None;
                        //     self.state = AudioState::Stopped;
                        // }
                    }
                },

                Err(TryRecvError::Disconnected) => return,

                // No commands? play some audio.
                Err(TryRecvError::Empty) => {
                    let _psd = match self.playing_state_data {
                        Some(ref mut psd) => {
                            if !psd.process() {
                                self.plm_tx
                                    .send(PlmCommand::AudioFinishedTrack {
                                        track_id: psd.track_id.clone(),
                                    })
                                    .unwrap();
                                // TODO preserve the audio output from psd
                                self.state = self.play_next_track();
                            }
                        }
                        None => {
                            error!(
                                "Somehow we're in the playing state, with no \
                                 audio player data. Stopping."
                            );
                            self.state = AudioState::Stopped;
                        }
                    };
                }
            }
        }
    }

    fn play_next_track(&mut self) -> AudioState {
        loop {
            let (track_id, buf) = match self.queue.pop_front() {
                Some(buf) => buf,
                None => {
                    return AudioState::WillPlayWhenDataArrives;
                }
            };
            match PlayingStateData::new_for_track(track_id.clone(), buf) {
                Some(psd) => {
                    self.playing_state_data = Some(psd);
                    return AudioState::Playing;
                }
                None => {
                    warn!("Skipped unplayable track");
                    self.plm_tx.send(PlmCommand::AudioSkippedTrack { track_id }).unwrap();
                }
            }
        }
    }
}

/// All the state needed for actually playing audio, configured to
/// work against a single buffer at a time.
struct PlayingStateData {
    track_id: String,
    reader: Box<dyn FormatReader>,
    audio_track_id: u32,
    decoder: Box<dyn Decoder>,
    audio_output: Option<Box<dyn crate::output::AudioOutput>>,
}

impl PlayingStateData {
    /// Returns none if the track can't be played
    fn new_for_track(track_id: String, data: Bytes) -> Option<Self> {
        let mss = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());
        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let metadata_opts: MetadataOptions = Default::default();
        let hint = Hint::new();
        let probe_res = match get_probe().format(&hint, mss, &format_opts, &metadata_opts) {
            Ok(probed) => probed,
            Err(err) => {
                warn!("file not supported. reason? {}", err);
                return None;
            }
        };

        let reader = probe_res.format;
        let audio_track = match first_supported_track(reader.tracks()) {
            Some(t) => t,
            None => {
                warn!("no supported tracks");
                return None;
            }
        };
        let audio_track_id = audio_track.id;

        let decode_opts = DecoderOptions::default();
        let decoder = get_codecs()
            .make(&audio_track.codec_params, &decode_opts)
            .unwrap();

        Some(PlayingStateData {
            track_id,
            reader,
            audio_output: None,
            audio_track_id,
            decoder,
        })
    }

    /// Process one packet of audio. Return true if everything is fine enough,
    /// or false is something is wrong and we can't play this track
    /// anymore.
    fn process(&mut self) -> bool {
        // No command; run audio.
        let packet = loop {
            match self.reader.next_packet() {
                Ok(p) => {
                    // If the packet does not belong to the selected track, skip it.
                    if p.track_id() == self.audio_track_id {
                        break p;
                    }
                }
                Err(e) => {
                    // TODO don't print the end of stream error
                    warn!("reader error: {e}");
                    return false;
                }
            };
        };

        let decoded = match self.decoder.decode(&packet) {
            Ok(d) => d,
            Err(e) => {
                warn!("decode error: {}", e);
                return true;
            }
        };

        // If the audio output is not open, try to open it.
        if self.audio_output.is_none() {
            // Get the audio buffer specification. This is a description of the decoded
            // audio buffer's sample format and sample rate.
            let spec = *decoded.spec();

            // Get the capacity of the decoded buffer. Note that this is capacity, not
            // length! The capacity of the decoded buffer is constant for the life of the
            // decoder, but the length is not.
            let duration = decoded.capacity() as u64;

            // Try to open the audio output.
            self.audio_output
                .replace(crate::output::try_open(spec, duration).unwrap());
        } else {
            // TODO: Check the audio spec. and duration hasn't changed.
        }

        if let Some(audio_output) = &mut self.audio_output {
            // TODO error here?
            audio_output.write(decoded).unwrap();
        }

        true
    }
}

fn first_supported_track(
    tracks: &[symphonia::core::formats::Track],
) -> Option<&symphonia::core::formats::Track> {
    tracks
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
}
