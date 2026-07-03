use sim_lib_music_core::{
    Chord, Counterpoint, Melody, MelodyItem, MidiFileObj, MidiTrackObj, MusicError, MusicObject,
    Par, PianoRoll, Progression, Score, Seq, TimedNote,
};

/// Boxes any music object behind the `MusicObject` trait object.
///
/// Convenience for collecting heterogeneous material into a `Par` or `Seq`.
pub fn boxed<T>(value: T) -> Box<dyn MusicObject>
where
    T: MusicObject + 'static,
{
    Box::new(value)
}

/// Builds a parallel container that layers its children simultaneously.
pub fn par(children: Vec<Box<dyn MusicObject>>) -> Par {
    Par { children }
}

/// Builds a sequential container that plays its children one after another.
pub fn seq(children: Vec<Box<dyn MusicObject>>) -> Seq {
    Seq { children }
}

/// Builds a validated melody from a list of melody items.
pub fn melody(items: Vec<MelodyItem>) -> Result<Melody, MusicError> {
    Melody::new(items)
}

/// Builds a validated chord progression in an optional key.
pub fn progression(key: Option<String>, chords: Vec<Chord>) -> Result<Progression, MusicError> {
    Progression::new(key, chords)
}

/// Builds validated counterpoint from named voice melodies.
pub fn counterpoint(
    voices: Vec<Melody>,
    voice_names: Vec<String>,
) -> Result<Counterpoint, MusicError> {
    Counterpoint::new(voices, voice_names)
}

/// Builds a validated piano roll from timed notes.
pub fn piano_roll(items: Vec<TimedNote>) -> Result<PianoRoll, MusicError> {
    PianoRoll::new(items)
}

/// Builds a MIDI track object from raw events and an optional channel hint.
pub fn midi_track(
    events: Vec<sim_lib_music_core::MidiEvent>,
    channel_hint: Option<sim_lib_music_core::Channel>,
) -> MidiTrackObj {
    MidiTrackObj::new(events, channel_hint)
}

/// Builds a MIDI file object from a parsed standard MIDI file.
pub fn midi_file(file: sim_lib_music_core::SmfFile) -> MidiFileObj {
    MidiFileObj::new(file)
}

/// Builds a validated score with tempo, time signature, key, and body.
pub fn score(
    tempo_bpm: u32,
    time_signature: (u8, u8),
    key: Option<String>,
    body: sim_lib_music_core::Music,
) -> Result<Score, MusicError> {
    Score::new(tempo_bpm, time_signature, key, body)
}
