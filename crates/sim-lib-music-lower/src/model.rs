use num_rational::Ratio;
use thiserror::Error;

use sim_lib_midi_core::{
    Channel, ChannelMessage, DEFAULT_US_PER_QUARTER, MetaBucket, MetaEvent, MidiEvent, MidiPayload,
    TickTime, U7, bpm_to_us_per_quarter, synthetic_origin,
};
use sim_lib_midi_smf::{SmfError, SmfFile, SmfFormat, SmfTrack, write_smf as write_smf_bytes};
use sim_lib_music_core::{
    Articulation, AtomRef, Counterpoint, Music, MusicObject, Note, PianoRoll, Score, Time,
};

use crate::piano_roll::build_piano_roll_file;

/// Error raised while lowering music to a MIDI file.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LowerError {
    /// The target ticks-per-quarter resolution was zero.
    #[error("TPQ must be non-zero")]
    ZeroTpq,
    /// A pitch fell outside the representable MIDI range.
    #[error("pitch is outside MIDI range")]
    PitchOutOfRange,
    /// A note velocity fell outside the playable `1..=127` range.
    #[error("velocity is outside 1..=127")]
    VelocityOutOfRange {
        /// The offending velocity value.
        velocity: u8,
    },
    /// A note channel fell outside the `0..=15` range.
    #[error("channel is outside 0..=15")]
    ChannelOutOfRange {
        /// The offending channel value.
        channel: u8,
    },
    /// A duration could not be represented exactly at the target TPQ.
    #[error("duration cannot be represented at target TPQ")]
    InexactTime,
    /// A tempo value was not positive and finite.
    #[error("tempo must be positive and finite")]
    InvalidTempo,
    /// A time-signature denominator was not a power of two.
    #[error("time signature denominator must be a power of two")]
    InvalidTimeSignature,
    /// A piano-roll cell had no SMF representation.
    #[error("piano-roll cell {cell_kind} in lane {lane} cannot be exported to SMF")]
    UnsupportedPianoRollCell {
        /// The lane containing the unsupported cell.
        lane: String,
        /// The kind of the unsupported cell.
        cell_kind: String,
    },
    /// A serialization error surfaced from `sim-lib-midi-smf`.
    #[error(transparent)]
    Smf(#[from] SmfError),
}

/// A piecewise-constant tempo curve mapping music time to beats per minute.
#[derive(Clone, Debug, PartialEq)]
pub struct TempoMap {
    /// Tempo change points as `(time, bpm)` pairs in time order.
    pub points: Vec<(Time, f64)>,
}

impl TempoMap {
    /// Builds a tempo map holding a single constant tempo from time zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_lower::TempoMap;
    ///
    /// let map = TempoMap::constant(120.0);
    /// assert_eq!(map.points.len(), 1);
    /// ```
    pub fn constant(bpm: f64) -> Self {
        Self {
            points: vec![(Ratio::from_integer(0), bpm)],
        }
    }
}

/// Options controlling how music is lowered to a MIDI file.
#[derive(Clone, Debug, PartialEq)]
pub struct LowerOpts {
    /// Ticks-per-quarter resolution of the output file.
    pub tpq: u32,
    /// Tempo curve written as tempo meta events.
    pub tempo_map: TempoMap,
    /// Policy for splitting notes across MIDI tracks.
    pub track_split: TrackSplit,
}

/// Policy for distributing lowered notes across MIDI tracks.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrackSplit {
    /// Write all notes to a single track.
    SingleTrack,
    /// Split notes into one track per MIDI channel.
    ByChannel,
    /// Split notes into one track per counterpoint voice.
    CounterpointVoices,
}

impl Default for LowerOpts {
    fn default() -> Self {
        Self {
            tpq: 480,
            tempo_map: TempoMap::constant(120.0),
            track_split: TrackSplit::SingleTrack,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LoweredNote {
    onset: Time,
    note: Note,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LowerTrack {
    name: String,
    notes: Vec<LoweredNote>,
}

pub(crate) fn checked_tick_time(ticks: i64, tpq: u32) -> Result<TickTime, LowerError> {
    TickTime::new(ticks, tpq).map_err(|_| LowerError::ZeroTpq)
}

pub(crate) fn time_to_ticks(time: Time, tpq: u32) -> Result<i64, LowerError> {
    if tpq == 0 {
        return Err(LowerError::ZeroTpq);
    }
    let scaled = time * Ratio::from_integer(i64::from(tpq) * 4);
    if *scaled.denom() != 1 {
        return Err(LowerError::InexactTime);
    }
    Ok(*scaled.numer())
}

fn bpm_to_tempo_meta(bpm: f64) -> Result<MetaEvent, LowerError> {
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(LowerError::InvalidTempo);
    }
    Ok(MetaEvent::Tempo {
        us_per_quarter: bpm_to_us_per_quarter(bpm),
    })
}

pub(crate) fn tempo_meta_events(opts: &LowerOpts) -> Result<Vec<MidiEvent>, LowerError> {
    let points = if opts.tempo_map.points.is_empty() {
        vec![(Ratio::from_integer(0), 120.0)]
    } else {
        opts.tempo_map.points.clone()
    };
    points
        .into_iter()
        .map(|(time, bpm)| {
            let ticks = time_to_ticks(time, opts.tpq)?;
            Ok(MidiEvent {
                time: checked_tick_time(ticks, opts.tpq)?,
                origin: synthetic_origin(),
                payload: MidiPayload::Meta(bpm_to_tempo_meta(bpm)?),
            })
        })
        .collect()
}

fn gate_duration(note: &Note) -> Time {
    match note.articulation {
        Articulation::Staccato => note.duration / Ratio::from_integer(2),
        Articulation::Legato | Articulation::Tenuto => note.duration,
        _ => note.duration,
    }
}

pub(crate) fn validate_note(note: &Note) -> Result<(u8, Channel, U7), LowerError> {
    let midi_key = note.pitch.to_midi().ok_or(LowerError::PitchOutOfRange)?;
    let channel = Channel::new(note.channel.0).map_err(|_| LowerError::ChannelOutOfRange {
        channel: note.channel.0,
    })?;
    if !(1..=127).contains(&note.velocity) {
        return Err(LowerError::VelocityOutOfRange {
            velocity: note.velocity,
        });
    }
    Ok((midi_key, channel, U7(note.velocity)))
}

/// Lowers any music object into an in-memory MIDI file.
pub fn lower(object: &dyn MusicObject, opts: &LowerOpts) -> Result<SmfFile, LowerError> {
    build_lowered_file(object, None, opts)
}

/// Lowers a `Score` into a MIDI file, applying the score's tempo to the map.
pub fn lower_score(score: &Score, opts: &LowerOpts) -> Result<SmfFile, LowerError> {
    let mut opts = opts.clone();
    opts.tempo_map = score_tempo_map(score, &opts.tempo_map);
    build_lowered_file(&score.body, Some(score), &opts)
}

/// Lowers a `Score` and serializes the resulting MIDI file to bytes.
pub fn write_smf(score: &Score, opts: &LowerOpts) -> Result<Vec<u8>, LowerError> {
    let file = lower_score(score, opts)?;
    Ok(write_smf_bytes(&file)?)
}

/// Returns whether two music objects lower to byte-identical MIDI files.
pub fn equivalent_under_lowering(
    left: &dyn MusicObject,
    right: &dyn MusicObject,
    opts: &LowerOpts,
) -> Result<bool, LowerError> {
    Ok(lower(left, opts)? == lower(right, opts)?)
}

fn build_lowered_file(
    object: &dyn MusicObject,
    score: Option<&Score>,
    opts: &LowerOpts,
) -> Result<SmfFile, LowerError> {
    if let Some(roll) = piano_roll_object(object) {
        return build_piano_roll_file(roll, score, opts);
    }
    let note_tracks = build_note_tracks(object, opts.track_split);
    let multi_track = note_tracks.len() > 1;
    let mut tracks = Vec::new();
    let mut meta_events = tempo_meta_events(opts)?;
    if let Some(score) = score {
        meta_events.extend(score_meta_events(score, opts.tpq)?);
    }
    if multi_track {
        tracks.push(SmfTrack {
            events: with_track_name("Conductor", meta_events, opts.tpq)?,
        });
        for track in note_tracks {
            tracks.push(SmfTrack {
                events: lower_track_events(&track, opts.tpq)?,
            });
        }
    } else {
        let mut events = meta_events;
        if let Some(track) = note_tracks.first() {
            events.extend(lowered_note_events(&track.notes, opts.tpq)?);
            let name = score.map_or_else(|| "Music".to_owned(), |_| "Score".to_owned());
            tracks.push(SmfTrack {
                events: with_track_name(&name, events, opts.tpq)?,
            });
        } else {
            let name = score.map_or_else(|| "Music".to_owned(), |_| "Score".to_owned());
            tracks.push(SmfTrack {
                events: with_track_name(&name, events, opts.tpq)?,
            });
        }
    }
    let mut file = SmfFile {
        format: if multi_track {
            SmfFormat::Simultaneous
        } else {
            SmfFormat::SingleTrack
        },
        tpq: opts.tpq,
        tracks,
    };
    file.canonicalize();
    Ok(file)
}

fn score_tempo_map(score: &Score, opts_tempo_map: &TempoMap) -> TempoMap {
    let mut points = vec![(Ratio::from_integer(0), f64::from(score.tempo_bpm))];
    points.extend(
        opts_tempo_map
            .points
            .iter()
            .filter(|(time, _)| *time > Ratio::from_integer(0))
            .cloned(),
    );
    TempoMap { points }
}

pub(crate) fn score_meta_events(score: &Score, tpq: u32) -> Result<Vec<MidiEvent>, LowerError> {
    let zero = checked_tick_time(0, tpq)?;
    let mut events = vec![MidiEvent {
        time: zero,
        origin: synthetic_origin(),
        payload: MidiPayload::Meta(MetaEvent::TimeSig {
            num: score.time_signature.0,
            den_pow2: time_signature_den_pow2(score.time_signature.1)?,
            clocks_per_click: 24,
            thirty_seconds_per_quarter: 8,
        }),
    }];
    if let Some(key) = score.key.as_deref().and_then(parse_key_signature) {
        events.push(MidiEvent {
            time: zero,
            origin: synthetic_origin(),
            payload: MidiPayload::Meta(MetaEvent::KeySig {
                sharps_flats: key.0,
                minor: key.1,
            }),
        });
    }
    Ok(events)
}

fn build_note_tracks(object: &dyn MusicObject, track_split: TrackSplit) -> Vec<LowerTrack> {
    match track_split {
        TrackSplit::SingleTrack => vec![LowerTrack {
            name: object.kind().to_owned(),
            notes: collect_notes(object),
        }],
        TrackSplit::ByChannel => channel_tracks(object),
        TrackSplit::CounterpointVoices => counterpoint_tracks(object).unwrap_or_else(|| {
            vec![LowerTrack {
                name: "Voice 1".to_owned(),
                notes: collect_notes(object),
            }]
        }),
    }
}

fn collect_notes(object: &dyn MusicObject) -> Vec<LoweredNote> {
    let mut atoms = Vec::new();
    object.voices(Ratio::from_integer(0), &mut atoms);
    let mut notes = atoms
        .into_iter()
        .filter_map(|atom| match atom.atom {
            AtomRef::Note(note) => Some(LoweredNote {
                onset: atom.onset,
                note,
            }),
            AtomRef::Rest(_) | AtomRef::Phantom(_) => None,
        })
        .collect::<Vec<_>>();
    notes.sort_by_key(|lowered| {
        (
            lowered.onset,
            lowered.note.channel.0,
            lowered.note.pitch.semitone(),
        )
    });
    notes
}

fn channel_tracks(object: &dyn MusicObject) -> Vec<LowerTrack> {
    let mut groups = std::collections::BTreeMap::<u8, Vec<LoweredNote>>::new();
    for note in collect_notes(object) {
        groups.entry(note.note.channel.0).or_default().push(note);
    }
    if groups.is_empty() {
        return vec![LowerTrack {
            name: object.kind().to_owned(),
            notes: Vec::new(),
        }];
    }
    groups
        .into_iter()
        .map(|(channel, notes)| LowerTrack {
            name: format!("Channel {}", channel + 1),
            notes,
        })
        .collect()
}

fn counterpoint_tracks(object: &dyn MusicObject) -> Option<Vec<LowerTrack>> {
    let counterpoint = score_counterpoint(object)?;
    let tracks = counterpoint
        .voices
        .iter()
        .zip(counterpoint.normalized_voice_names())
        .map(|(voice, name)| LowerTrack {
            name,
            notes: collect_notes(voice),
        })
        .collect::<Vec<_>>();
    Some(if tracks.is_empty() {
        vec![LowerTrack {
            name: "Voice 1".to_owned(),
            notes: Vec::new(),
        }]
    } else {
        tracks
    })
}

fn score_counterpoint(object: &dyn MusicObject) -> Option<&Counterpoint> {
    if let Some(counterpoint) = object.as_any().downcast_ref::<Counterpoint>() {
        return Some(counterpoint);
    }
    if let Some(Music::Counterpoint(counterpoint)) = object.as_any().downcast_ref::<Music>() {
        return Some(counterpoint);
    }
    let score = object.as_any().downcast_ref::<Score>()?;
    match &score.body {
        Music::Counterpoint(counterpoint) => Some(counterpoint),
        _ => None,
    }
}

fn piano_roll_object(object: &dyn MusicObject) -> Option<&PianoRoll> {
    if let Some(roll) = object.as_any().downcast_ref::<PianoRoll>() {
        return Some(roll);
    }
    if let Some(Music::PianoRoll(roll)) = object.as_any().downcast_ref::<Music>() {
        return Some(roll);
    }
    None
}

fn lower_track_events(track: &LowerTrack, tpq: u32) -> Result<Vec<MidiEvent>, LowerError> {
    let events = lowered_note_events(&track.notes, tpq)?;
    with_track_name(&track.name, events, tpq)
}

fn lowered_note_events(notes: &[LoweredNote], tpq: u32) -> Result<Vec<MidiEvent>, LowerError> {
    let mut events = Vec::with_capacity(notes.len().saturating_mul(2));
    for note in notes {
        let (midi_key, channel, velocity) = validate_note(&note.note)?;
        let start = time_to_ticks(note.onset, tpq)?;
        let end = time_to_ticks(note.onset + gate_duration(&note.note), tpq)?;
        events.push(MidiEvent {
            time: checked_tick_time(start, tpq)?,
            origin: synthetic_origin(),
            payload: MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: channel,
                key: U7(midi_key),
                vel: velocity,
            }),
        });
        events.push(MidiEvent {
            time: checked_tick_time(end, tpq)?,
            origin: synthetic_origin(),
            payload: MidiPayload::Channel(ChannelMessage::NoteOff {
                ch: channel,
                key: U7(midi_key),
                vel: U7(0),
            }),
        });
    }
    Ok(events)
}

pub(crate) fn with_track_name(
    name: &str,
    mut events: Vec<MidiEvent>,
    tpq: u32,
) -> Result<Vec<MidiEvent>, LowerError> {
    events.push(MidiEvent {
        time: checked_tick_time(0, tpq)?,
        origin: synthetic_origin(),
        payload: MidiPayload::Meta(MetaEvent::Other(MetaBucket {
            type_byte: 0x03,
            data: name.as_bytes().to_vec(),
        })),
    });
    Ok(events)
}

fn time_signature_den_pow2(denominator: u8) -> Result<u8, LowerError> {
    if denominator == 0 || !denominator.is_power_of_two() {
        return Err(LowerError::InvalidTimeSignature);
    }
    Ok(denominator.trailing_zeros() as u8)
}

fn parse_key_signature(key: &str) -> Option<(i8, bool)> {
    let trimmed = key.trim();
    let (tonic, minor) = trimmed
        .strip_suffix('m')
        .map(|tonic| (tonic, true))
        .unwrap_or((trimmed, false));
    let sharps_flats = match tonic {
        "C" => 0,
        "G" => 1,
        "D" => 2,
        "A" => 3,
        "E" => 4,
        "B" => 5,
        "F#" | "Gb" => {
            if tonic == "F#" {
                6
            } else {
                -6
            }
        }
        "C#" | "Db" => {
            if tonic == "C#" {
                7
            } else {
                -5
            }
        }
        "F" => -1,
        "Bb" => -2,
        "Eb" => -3,
        "Ab" => -4,
        "Cb" => -7,
        _ => return None,
    };
    Some((sharps_flats, minor))
}

/// Returns the default microseconds-per-quarter-note tempo value.
pub const fn default_us_per_quarter() -> u32 {
    DEFAULT_US_PER_QUARTER
}
