//! MIDI input, CC mapping and clock synchronization.
//!
//! Default mapping follows the RC-505 MkII philosophy: each MIDI channel maps to
//! one track, and CC numbers control transport. ASSIGN 1-16 can be re-mapped at
//! runtime through `set_midi_mapping`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::Sender;
use midir::{Ignore, MidiInput};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::state::{f32_to_bits, Shared, TrackCommand};

#[derive(Clone, Debug, Serialize)]
pub struct MidiPort {
    pub index: usize,
    pub name: String,
}

/// One ASSIGN slot: incoming CC -> track action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CcMapping {
    pub cc: u8,
    /// `None` = any channel, otherwise 0-15.
    pub channel: Option<u8>,
    pub track: usize,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MidiEvent {
    Track { track: usize, command: TrackCommand },
    Volume { track: usize, value: f32 },
    Pan { track: usize, value: f32 },
    Clock,
    Transport { play: bool },
}

pub struct MidiHub {
    tx: Sender<(usize, TrackCommand)>,
    shared: Shared,
    mapping: RwLock<Vec<CcMapping>>,
    open_port: RwLock<Option<String>>,
    stop: Arc<AtomicBool>,
}

impl MidiHub {
    pub fn new(tx: Sender<(usize, TrackCommand)>, shared: Shared) -> Self {
        Self {
            tx,
            shared,
            mapping: RwLock::new(default_mapping()),
            open_port: RwLock::new(None),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn list_ports(&self) -> Vec<MidiPort> {
        let input = match MidiInput::new("RC-505 Clone") {
            Ok(input) => input,
            Err(err) => {
                eprintln!("[midi] cannot enumerate ports: {err}");
                return Vec::new();
            }
        };
        input
            .ports()
            .iter()
            .enumerate()
            .map(|(index, port)| MidiPort {
                index,
                name: input
                    .port_name(port)
                    .unwrap_or_else(|_| format!("Port {index}")),
            })
            .collect()
    }

    pub fn mapping(&self) -> Vec<CcMapping> {
        self.mapping.read().clone()
    }

    pub fn set_mapping(&self, mapping: Vec<CcMapping>) {
        *self.mapping.write() = mapping;
    }

    pub fn open_port(&self, index: usize) -> Result<String, String> {
        let ports = self.list_ports();
        let port = ports
            .get(index)
            .ok_or_else(|| format!("MIDI port {index} not found"))?;
        let name = port.name.clone();

        let tx = self.tx.clone();
        let shared = self.shared.clone();
        let mapping = self.mapping.read().clone();
        let stop = self.stop.clone();
        stop.store(false, Ordering::Relaxed);
        *self.open_port.write() = Some(name.clone());

        thread::Builder::new()
            .name("rc505-midi".to_string())
            .spawn(move || {
                let mut input = match MidiInput::new("RC-505 Clone Input") {
                    Ok(input) => input,
                    Err(err) => {
                        eprintln!("[midi] init failed: {err}");
                        return;
                    }
                };
                input.ignore(Ignore::None);
                let ports = input.ports();
                let Some(port) = ports.get(index) else {
                    eprintln!("[midi] port {index} disappeared");
                    return;
                };
                let tx_cb = tx.clone();
                let shared_cb = shared.clone();
                let mapping_cb = mapping.clone();
                let connection = input.connect(
                    port,
                    "rc505-midi-in",
                    move |_timestamp, message, _| {
                        dispatch(message, &tx_cb, &shared_cb, &mapping_cb);
                    },
                    (),
                );
                match connection {
                    Ok(_connection) => {
                        while !stop.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_millis(250));
                        }
                    }
                    Err(err) => eprintln!("[midi] connect failed: {err}"),
                }
            })
            .map_err(|err| err.to_string())?;

        Ok(name)
    }

    pub fn close(&self) {
        self.stop.store(true, Ordering::Relaxed);
        *self.open_port.write() = None;
    }

    pub fn opened_port(&self) -> Option<String> {
        self.open_port.read().clone()
    }
}

/// Default ASSIGN matrix: channel = track, CC = transport.
pub fn default_mapping() -> Vec<CcMapping> {
    let mut mapping = Vec::new();
    for track in 0..crate::state::MAX_TRACKS {
        for (cc, action) in [
            (20u8, "record"),
            (21, "play"),
            (22, "dub"),
            (23, "stop"),
            (24, "clear"),
            (25, "undo"),
        ] {
            mapping.push(CcMapping {
                cc,
                channel: Some(track as u8),
                track,
                action: action.to_string(),
            });
        }
    }
    mapping
}

/// Pure parser, unit-testable without any MIDI hardware.
pub fn parse_message(message: &[u8], mapping: &[CcMapping]) -> Option<MidiEvent> {
    let status = *message.first()?;
    match status {
        0xF8 => return Some(MidiEvent::Clock),
        0xFA => return Some(MidiEvent::Transport { play: true }),
        0xFC | 0xFB => return Some(MidiEvent::Transport { play: false }),
        _ => {}
    }
    let kind = status & 0xF0;
    let channel = status & 0x0F;
    let data1 = *message.get(1)?;
    let data2 = message.get(2).copied().unwrap_or(0);

    match kind {
        0xB0 => {
            if data1 == 7 {
                return Some(MidiEvent::Volume {
                    track: (channel as usize).min(crate::state::MAX_TRACKS - 1),
                    value: data2 as f32 / 127.0,
                });
            }
            if data1 == 10 {
                return Some(MidiEvent::Pan {
                    track: (channel as usize).min(crate::state::MAX_TRACKS - 1),
                    value: (data2 as f32 / 127.0) * 2.0 - 1.0,
                });
            }
            let entry = mapping.iter().find(|entry| {
                entry.cc == data1
                    && (entry.channel.is_none() || entry.channel == Some(channel))
                    && entry.track < crate::state::MAX_TRACKS
            })?;
            let command = TrackCommand::parse(&entry.action)?;
            Some(MidiEvent::Track {
                track: entry.track,
                command,
            })
        }
        0x90 if data2 > 0 => {
            // Note 36..40 = play/stop toggle per track, 41..45 = record.
            let note = data1 as usize;
            if (36..=40).contains(&note) {
                Some(MidiEvent::Track {
                    track: note - 36,
                    command: TrackCommand::Play,
                })
            } else if (41..=45).contains(&note) {
                Some(MidiEvent::Track {
                    track: note - 41,
                    command: TrackCommand::Record,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn dispatch(
    message: &[u8],
    tx: &Sender<(usize, TrackCommand)>,
    shared: &Shared,
    mapping: &[CcMapping],
) {
    shared.midi_activity.fetch_add(1, Ordering::Relaxed);
    match parse_message(message, mapping) {
        Some(MidiEvent::Clock) => {
            shared.midi_clock.fetch_add(1, Ordering::Relaxed);
        }
        Some(MidiEvent::Track { track, command }) => {
            let _ = tx.try_send((track, command));
        }
        Some(MidiEvent::Volume { track, value }) => {
            shared.tracks[track]
                .volume
                .store(f32_to_bits(value), Ordering::Relaxed);
        }
        Some(MidiEvent::Pan { track, value }) => {
            shared.tracks[track]
                .pan
                .store(f32_to_bits(value), Ordering::Relaxed);
        }
        Some(MidiEvent::Transport { play }) => {
            let command = if play {
                TrackCommand::Play
            } else {
                TrackCommand::Stop
            };
            for track in 0..crate::state::MAX_TRACKS {
                let _ = tx.try_send((track, command));
            }
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mapping_covers_all_tracks() {
        let mapping = default_mapping();
        assert_eq!(mapping.len(), crate::state::MAX_TRACKS * 6);
        for track in 0..crate::state::MAX_TRACKS {
            assert!(mapping.iter().any(|m| m.track == track && m.action == "record"));
        }
    }

    #[test]
    fn cc_transport_maps_to_channel_track() {
        let mapping = default_mapping();
        // CC 20 on channel 3 = record track 3.
        let event = parse_message(&[0xB3, 20, 127], &mapping).unwrap();
        assert_eq!(
            event,
            MidiEvent::Track {
                track: 3,
                command: TrackCommand::Record
            }
        );
    }

    #[test]
    fn volume_and_pan_are_normalized() {
        let mapping = default_mapping();
        assert_eq!(
            parse_message(&[0xB0, 7, 127], &mapping),
            Some(MidiEvent::Volume { track: 0, value: 1.0 })
        );
        assert_eq!(
            parse_message(&[0xB0, 10, 0], &mapping),
            Some(MidiEvent::Pan { track: 0, value: -1.0 })
        );
    }

    #[test]
    fn note_and_clock_messages() {
        let mapping = default_mapping();
        assert_eq!(
            parse_message(&[0x90, 37, 100], &mapping),
            Some(MidiEvent::Track {
                track: 1,
                command: TrackCommand::Play
            })
        );
        assert_eq!(parse_message(&[0xF8], &mapping), Some(MidiEvent::Clock));
        assert_eq!(
            parse_message(&[0xFA], &mapping),
            Some(MidiEvent::Transport { play: true })
        );
    }

    #[test]
    fn unknown_cc_is_ignored() {
        let mapping = default_mapping();
        assert!(parse_message(&[0xB0, 99, 1], &mapping).is_none());
        assert!(parse_message(&[], &mapping).is_none());
    }
}
