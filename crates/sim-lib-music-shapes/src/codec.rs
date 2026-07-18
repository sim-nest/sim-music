use thiserror::Error;

use sim_lib_midi_shapes::{encode_midi_event, encode_smf_file};
use sim_lib_music_analysis::{ChordWindow, ChordWindowMode, DiffFrame, DiffRoll};
use sim_lib_music_core::{
    Arranger, Articulation, AtomRef, Chord, Counterpoint, Melody, MelodyItem, MidiFileObj,
    MidiTrackObj, Music, MusicError, MusicObject, Note, Par, PianoRoll, Progression, Rest, Score,
    Seq, Time, TimedNote,
};
use sim_lib_music_lift::{
    CounterpointLiftOpts, LabelStrategy, ProgressionLiftOpts, VoiceAssignment,
};
use sim_lib_music_transform::{FunctionMap, RetrogradeMode};
use sim_lib_pitch_scale::{Key, Mode};

mod analysis;
mod encode_arranger;
mod filter;
mod lift;
mod parse;

use encode_arranger::encode_arranger_placement;

pub use analysis::{
    decode_chord_window, decode_chord_window_mode, decode_diff_frame, decode_diff_roll,
    decode_function_map, decode_retrograde_mode,
};
pub use filter::{
    custom_filter_from_expr, custom_filter_to_expr, decode_custom_filter, encode_custom_filter,
};
pub use lift::{
    decode_counterpoint_lift_opts, decode_label_strategy, decode_progression_lift_opts,
    decode_voice_assignment,
};
pub use parse::{
    decode_arranger, decode_chord, decode_counterpoint, decode_melody, decode_midi_file,
    decode_midi_track, decode_music, decode_music_file, decode_note, decode_piano_roll,
    decode_progression, decode_rest, decode_score, decode_time,
};

/// Error raised while decoding a music `#(...)` form.
#[derive(Debug, Error)]
pub enum MusicShapeError {
    /// A time/duration field was not a valid `numer/denom` rational.
    #[error("invalid time shape")]
    InvalidTime,
    /// The form did not match a recognized music shape or field set.
    #[error("invalid music shape")]
    InvalidMusic,
    /// Input ended before the form was complete.
    #[error("unexpected end of input")]
    UnexpectedEof,
    /// A token in the form text was malformed.
    #[error("invalid token in music shape")]
    InvalidToken,
    /// A construction error surfaced from `sim-lib-music-core`.
    #[error(transparent)]
    Music(#[from] MusicError),
}

/// Encodes a `Time` as its `numer/denom` text form.
///
/// # Examples
///
/// ```
/// use sim_lib_music_shapes::{decode_time, encode_time};
///
/// let time = decode_time("1/4").unwrap();
/// assert_eq!(encode_time(time), "1/4");
/// ```
pub fn encode_time(time: Time) -> String {
    format!("{}/{}", time.numer(), time.denom())
}

/// Encodes a `Note` as its `#(Note ...)` form.
pub fn encode_note(note: &Note) -> String {
    format!(
        "#(Note dur={} pitch={} vel={} channel={} articulation={})",
        encode_time(note.duration),
        encode_pitch(note.pitch),
        note.velocity,
        note.channel.0,
        encode_articulation(note.articulation),
    )
}

/// Encodes a `Rest` as its `#(Rest ...)` form.
pub fn encode_rest(rest: &Rest) -> String {
    format!("#(Rest dur={})", encode_time(rest.duration))
}

/// Encodes a `Par` parallel composition as its `#(Par ...)` form.
pub fn encode_par(par: &Par) -> String {
    format!(
        "#(Par children=[{}])",
        par.children
            .iter()
            .map(|child| encode_music_object(child.as_ref()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Encodes a `Seq` sequential composition as its `#(Seq ...)` form.
pub fn encode_seq(seq: &Seq) -> String {
    format!(
        "#(Seq children=[{}])",
        seq.children
            .iter()
            .map(|child| encode_music_object(child.as_ref()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Encodes a `Chord` as its `#(Chord ...)` form.
pub fn encode_chord(chord: &Chord) -> String {
    format!(
        "#(Chord dur={} symbol={} pitches=[{}] vel={} channel={})",
        encode_time(chord.duration),
        encode_string(&chord.symbol),
        chord
            .pitches
            .iter()
            .map(|pitch| encode_pitch(*pitch))
            .collect::<Vec<_>>()
            .join(","),
        chord.velocity,
        chord.channel.0,
    )
}

/// Encodes a `Melody` as its `#(Melody ...)` form.
pub fn encode_melody(melody: &Melody) -> String {
    format!(
        "#(Melody items=[{}])",
        melody
            .items
            .iter()
            .map(encode_melody_item)
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Encodes a `Progression` as its `#(Progression ...)` form.
pub fn encode_progression(progression: &Progression) -> String {
    let key = progression
        .key
        .as_deref()
        .map(encode_string)
        .unwrap_or_else(|| "none".to_owned());
    format!(
        "#(Progression key={} chords=[{}])",
        key,
        progression
            .chords
            .iter()
            .map(encode_chord)
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Encodes a `Counterpoint` as its `#(Counterpoint ...)` form.
pub fn encode_counterpoint(counterpoint: &Counterpoint) -> String {
    format!(
        "#(Counterpoint voice_names=[{}] voices=[{}])",
        counterpoint
            .voice_names
            .iter()
            .map(|name| encode_string(name))
            .collect::<Vec<_>>()
            .join(","),
        counterpoint
            .voices
            .iter()
            .map(encode_melody)
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Encodes a `PianoRoll` as its `#(PianoRoll ...)` form.
pub fn encode_piano_roll(roll: &PianoRoll) -> String {
    format!(
        "#(PianoRoll items=[{}])",
        roll.items
            .iter()
            .map(encode_timed_note)
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Encodes an `Arranger` as its `#(Arranger ...)` form.
pub fn encode_arranger(arranger: &Arranger) -> String {
    format!(
        "#(Arranger lanes=[{}] placements=[{}])",
        arranger
            .lanes
            .iter()
            .map(|lane| encode_string(&lane.0))
            .collect::<Vec<_>>()
            .join(","),
        arranger
            .placements
            .iter()
            .map(encode_arranger_placement)
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Encodes a `MidiTrackObj` as its `#(MidiTrackObj ...)` form.
pub fn encode_midi_track(track: &MidiTrackObj) -> String {
    let channel_hint = track
        .channel_hint
        .map(|channel| channel.0.to_string())
        .unwrap_or_else(|| "none".to_owned());
    format!(
        "#(MidiTrackObj channel_hint={} events=[{}])",
        channel_hint,
        track
            .events
            .iter()
            .map(|event| encode_string(&encode_midi_event(event)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Encodes a `MidiFileObj` as its `#(MidiFileObj ...)` form.
pub fn encode_midi_file(file: &MidiFileObj) -> String {
    format!(
        "#(MidiFileObj smf={})",
        encode_string(&encode_smf_file(&file.file))
    )
}

/// Encodes a `Score` as its `#(Score ...)` root form.
pub fn encode_score(score: &Score) -> String {
    let key = score
        .key
        .as_deref()
        .map(encode_string)
        .unwrap_or_else(|| "none".to_owned());
    format!(
        "#(Score tempo={} time_sig={}/{} key={} body={})",
        score.tempo_bpm,
        score.time_signature.0,
        score.time_signature.1,
        key,
        encode_music(&score.body),
    )
}

/// Encodes any `Music` variant by dispatching to the matching `encode_*` form.
pub fn encode_music(music: &Music) -> String {
    match music {
        Music::Note(note) => encode_note(note),
        Music::Rest(rest) => encode_rest(rest),
        Music::Par(par) => encode_par(par),
        Music::Seq(seq) => encode_seq(seq),
        Music::Chord(chord) => encode_chord(chord),
        Music::Melody(melody) => encode_melody(melody),
        Music::Progression(progression) => encode_progression(progression),
        Music::Counterpoint(counterpoint) => encode_counterpoint(counterpoint),
        Music::PianoRoll(roll) => encode_piano_roll(roll),
        Music::Arranger(arranger) => encode_arranger(arranger),
        Music::MidiTrack(track) => encode_midi_track(track),
        Music::MidiFile(file) => encode_midi_file(file),
    }
}

/// Encodes a top-level music file, which is a `Score`, to its root form.
pub fn encode_music_file(score: &Score) -> String {
    encode_score(score)
}

/// Encodes a `RetrogradeMode` selector as its `#(RetrogradeMode ...)` form.
pub fn encode_retrograde_mode(mode: RetrogradeMode) -> String {
    let value = match mode {
        RetrogradeMode::Cutout => "Cutout",
        RetrogradeMode::PinnedNoteOn => "PinnedNoteOn",
    };
    format!("#(RetrogradeMode value={value})")
}

/// Encodes a `FunctionMap` as its `#(FunctionMap ...)` form.
pub fn encode_function_map(map: &FunctionMap) -> String {
    match map {
        FunctionMap::Custom(scale) => format!(
            "#(FunctionMap kind=Custom tonic={} mode={})",
            scale.tonic.canonical_name(),
            encode_mode(scale.mode),
        ),
        _ => format!("#(FunctionMap kind={})", map.name()),
    }
}

/// Encodes a `ChordWindowMode` selector as its `#(ChordWindowMode ...)` form.
pub fn encode_chord_window_mode(mode: ChordWindowMode) -> String {
    let value = match mode {
        ChordWindowMode::SoundingNotes => "SoundingNotes",
        ChordWindowMode::StartingNotes => "StartingNotes",
    };
    format!("#(ChordWindowMode value={value})")
}

/// Encodes a `LabelStrategy` selector as its `#(LabelStrategy ...)` form.
pub fn encode_label_strategy(strategy: LabelStrategy) -> String {
    let value = match strategy {
        LabelStrategy::Functional => "Functional",
        LabelStrategy::JazzChord => "JazzChord",
        LabelStrategy::SetClass => "SetClass",
    };
    format!("#(LabelStrategy value={value})")
}

/// Encodes a `VoiceAssignment` selector as its `#(VoiceAssignment ...)` form.
pub fn encode_voice_assignment(assignment: VoiceAssignment) -> String {
    let value = match assignment {
        VoiceAssignment::ChannelOnly => "ChannelOnly",
        VoiceAssignment::TrackThenChannel => "TrackThenChannel",
        VoiceAssignment::HighestFirst => "HighestFirst",
        VoiceAssignment::LowestFirst => "LowestFirst",
    };
    format!("#(VoiceAssignment value={value})")
}

/// Encodes `ProgressionLiftOpts` as its `#(ProgressionLiftOpts ...)` form.
pub fn encode_progression_lift_opts(opts: &ProgressionLiftOpts) -> String {
    format!(
        "#(ProgressionLiftOpts grid={} min_notes={} key_hint={} label_strategy={} window_mode={})",
        encode_time(opts.grid),
        opts.min_notes,
        encode_key_hint(opts.key_hint),
        encode_label_strategy_atom(opts.label_strategy),
        encode_chord_window_mode_atom(opts.window_mode),
    )
}

/// Encodes `CounterpointLiftOpts` as its `#(CounterpointLiftOpts ...)` form.
pub fn encode_counterpoint_lift_opts(opts: &CounterpointLiftOpts) -> String {
    format!(
        "#(CounterpointLiftOpts min_rest_to_close={} max_voices_per_track={} voice_assignment={})",
        encode_time(opts.min_rest_to_close),
        opts.max_voices_per_track,
        encode_voice_assignment_atom(opts.voice_assignment),
    )
}

/// Encodes a `DiffFrame` analysis frame as its `#(DiffFrame ...)` form.
pub fn encode_diff_frame(frame: &DiffFrame) -> String {
    format!(
        "#(DiffFrame at={} sounding={} started={} ended={} slurred={})",
        encode_time(frame.at),
        frame.sounding.bits,
        frame.started.bits,
        frame.ended.bits,
        frame.slurred.bits,
    )
}

/// Encodes a `DiffRoll` analysis view as its `#(DiffRoll ...)` form.
pub fn encode_diff_roll(diff: &DiffRoll) -> String {
    format!(
        "#(DiffRoll frames=[{}])",
        diff.frames
            .iter()
            .map(encode_diff_frame)
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Encodes a `ChordWindow` analysis result as its `#(ChordWindow ...)` form.
pub fn encode_chord_window(window: &ChordWindow) -> String {
    let root = window
        .bit_chord
        .root
        .map(|root| root.canonical_name().to_owned())
        .unwrap_or_else(|| "none".to_owned());
    let mode = match window.mode {
        ChordWindowMode::SoundingNotes => "SoundingNotes",
        ChordWindowMode::StartingNotes => "StartingNotes",
    };
    format!(
        "#(ChordWindow at={} until={} mode={} range={} pitch_classes={} root={})",
        encode_time(window.at),
        encode_time(window.until),
        mode,
        window.range_mask.bits,
        window.pitch_class_mask.bits(),
        root,
    )
}

pub(crate) fn encode_music_object(value: &dyn MusicObject) -> String {
    if let Some(music) = value.as_any().downcast_ref::<Music>() {
        return encode_music(music);
    }
    if let Some(note) = value.as_any().downcast_ref::<Note>() {
        return encode_note(note);
    }
    if let Some(rest) = value.as_any().downcast_ref::<Rest>() {
        return encode_rest(rest);
    }
    if let Some(par) = value.as_any().downcast_ref::<Par>() {
        return encode_par(par);
    }
    if let Some(seq) = value.as_any().downcast_ref::<Seq>() {
        return encode_seq(seq);
    }
    if let Some(chord) = value.as_any().downcast_ref::<Chord>() {
        return encode_chord(chord);
    }
    if let Some(melody) = value.as_any().downcast_ref::<Melody>() {
        return encode_melody(melody);
    }
    if let Some(progression) = value.as_any().downcast_ref::<Progression>() {
        return encode_progression(progression);
    }
    if let Some(counterpoint) = value.as_any().downcast_ref::<Counterpoint>() {
        return encode_counterpoint(counterpoint);
    }
    if let Some(roll) = value.as_any().downcast_ref::<PianoRoll>() {
        return encode_piano_roll(roll);
    }
    if let Some(arranger) = value.as_any().downcast_ref::<Arranger>() {
        return encode_arranger(arranger);
    }
    if let Some(track) = value.as_any().downcast_ref::<MidiTrackObj>() {
        return encode_midi_track(track);
    }
    if let Some(file) = value.as_any().downcast_ref::<MidiFileObj>() {
        return encode_midi_file(file);
    }
    let mut atoms = Vec::new();
    value.voices(Time::from_integer(0), &mut atoms);
    if let Some(atom) = atoms.first() {
        return match &atom.atom {
            AtomRef::Note(note) => encode_note(note),
            AtomRef::Rest(rest) => encode_rest(rest),
            AtomRef::Phantom(_) => panic!("unsupported phantom music object {}", value.kind()),
        };
    }
    panic!("unsupported music object {}", value.kind());
}

pub(crate) fn encode_articulation(articulation: Articulation) -> &'static str {
    match articulation {
        Articulation::Normal => "Normal",
        Articulation::Staccato => "Staccato",
        Articulation::Legato => "Legato",
        Articulation::Tenuto => "Tenuto",
        Articulation::Accent => "Accent",
        Articulation::Marcato => "Marcato",
    }
}

pub(crate) fn encode_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub(crate) fn encode_pitch(pitch: sim_lib_music_core::Pitch) -> String {
    format!("{}{}", pitch.class.canonical_name(), pitch.octave)
}

fn encode_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Major => "major",
        Mode::MinorNatural => "minor-natural",
        Mode::MinorHarmonic => "minor-harmonic",
        Mode::MinorMelodic => "minor-melodic",
        Mode::Dorian => "dorian",
        Mode::Phrygian => "phrygian",
        Mode::Lydian => "lydian",
        Mode::Mixolydian => "mixolydian",
        Mode::Aeolian => "aeolian",
        Mode::Locrian => "locrian",
        Mode::WholeTone => "whole-tone",
        Mode::Diminished => "diminished",
        Mode::Chromatic => "chromatic",
    }
}

fn encode_key_hint(key: Option<Key>) -> String {
    key.map(|key| format!("{}:{}", key.tonic.canonical_name(), encode_mode(key.mode)))
        .unwrap_or_else(|| "none".to_owned())
}

fn encode_label_strategy_atom(strategy: LabelStrategy) -> &'static str {
    match strategy {
        LabelStrategy::Functional => "Functional",
        LabelStrategy::JazzChord => "JazzChord",
        LabelStrategy::SetClass => "SetClass",
    }
}

fn encode_voice_assignment_atom(assignment: VoiceAssignment) -> &'static str {
    match assignment {
        VoiceAssignment::ChannelOnly => "ChannelOnly",
        VoiceAssignment::TrackThenChannel => "TrackThenChannel",
        VoiceAssignment::HighestFirst => "HighestFirst",
        VoiceAssignment::LowestFirst => "LowestFirst",
    }
}

fn encode_chord_window_mode_atom(mode: ChordWindowMode) -> &'static str {
    match mode {
        ChordWindowMode::SoundingNotes => "SoundingNotes",
        ChordWindowMode::StartingNotes => "StartingNotes",
    }
}

pub(crate) fn decode_mode(value: &str) -> Option<Mode> {
    Some(match value {
        "major" => Mode::Major,
        "minor-natural" => Mode::MinorNatural,
        "minor-harmonic" => Mode::MinorHarmonic,
        "minor-melodic" => Mode::MinorMelodic,
        "dorian" => Mode::Dorian,
        "phrygian" => Mode::Phrygian,
        "lydian" => Mode::Lydian,
        "mixolydian" => Mode::Mixolydian,
        "aeolian" => Mode::Aeolian,
        "locrian" => Mode::Locrian,
        "whole-tone" => Mode::WholeTone,
        "diminished" => Mode::Diminished,
        "chromatic" => Mode::Chromatic,
        _ => return None,
    })
}

fn encode_melody_item(item: &MelodyItem) -> String {
    match item {
        MelodyItem::Note(note) => encode_note(note),
        MelodyItem::Rest(rest) => encode_rest(rest),
    }
}

fn encode_timed_note(item: &TimedNote) -> String {
    format!(
        "#(TimedNote onset={} note={})",
        encode_time(item.onset),
        encode_note(&item.note)
    )
}
