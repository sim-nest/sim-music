use thiserror::Error;

use sim_codec::encode_string_literal;
use sim_lib_midi_smf::{SmfError, SmfFormat, read_smf, write_smf};
use sim_lib_midi_wasm_frame::{MidiEventFrame, MidiWasmError, frame_array_to_smf};
use sim_lib_music_analysis::DiffRoll;
use sim_lib_music_core::{Counterpoint, Music, PianoRoll, Progression, Score};
use sim_lib_music_lift::{
    CounterpointLiftOpts, LiftError, ProgressionLiftOpts, lift_to_counterpoint, lift_to_diff_roll,
    lift_to_piano_roll, lift_to_progression,
};
use sim_lib_music_lower::{LowerError, LowerOpts, lower_score};
use sim_lib_music_shapes::{
    MusicShapeError, decode_music_file, encode_counterpoint, encode_diff_roll, encode_music_file,
    encode_piano_roll, encode_progression, music_score_class_symbol,
};

/// Error raised by the music wasm facade entrypoints.
///
/// Wraps the errors of each underlying stage (SMF, MIDI frames, lift, lower,
/// shape codecs, and the music-core model).
#[derive(Debug, Error)]
pub enum MusicWasmError {
    /// Error reading or writing Standard MIDI File bytes.
    #[error(transparent)]
    Smf(#[from] SmfError),
    /// Error converting between MIDI event frames and SMF.
    #[error(transparent)]
    MidiWasm(#[from] MidiWasmError),
    /// Error lifting MIDI into higher-level music objects.
    #[error(transparent)]
    Lift(#[from] LiftError),
    /// Error lowering a score down to MIDI.
    #[error(transparent)]
    Lower(#[from] LowerError),
    /// Error encoding or decoding a music shape surface.
    #[error(transparent)]
    Shape(#[from] MusicShapeError),
    /// Error surfaced from the underlying music-core model.
    #[error(transparent)]
    Music(#[from] sim_lib_music_core::MusicError),
}

/// Frame-safe text report of music objects derived from a single input.
///
/// Each field holds the encoded surface form of one analyzed object, suitable
/// for crossing the wasm boundary as a string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicDemoReport {
    /// Encoded chord progression.
    pub progression: String,
    /// Encoded counterpoint.
    pub counterpoint: String,
    /// Encoded piano roll.
    pub piano_roll: String,
    /// Encoded difference roll.
    pub diff_roll: String,
    /// Encoded music-file form of the reconstructed score.
    pub music_file: String,
}

/// Stable wasm engine entrypoint names for the music facade.
///
/// Holds the ABI symbol that browser and engine adapters call for each
/// operation, kept stable across builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicWasmEngineEntryPoints {
    /// Entrypoint name lowering a music file to MIDI event frames.
    pub lower_frames: &'static str,
    /// Entrypoint name lifting MIDI event frames to a music file.
    pub lift_frames: &'static str,
    /// Entrypoint name analyzing input into a demo report.
    pub analyze: &'static str,
}

/// Returns the stable wasm engine entrypoint names for the music facade.
///
/// # Examples
///
/// ```
/// use sim_lib_music_wasm_frame::music_wasm_engine_entry_points;
///
/// let entry = music_wasm_engine_entry_points();
/// assert_eq!(entry.analyze, "music-wasm-analyze");
/// ```
pub fn music_wasm_engine_entry_points() -> MusicWasmEngineEntryPoints {
    MusicWasmEngineEntryPoints {
        lower_frames: "music-wasm-lower-frames",
        lift_frames: "music-wasm-lift-frames",
        analyze: "music-wasm-analyze",
    }
}

/// Analyzes Standard MIDI File bytes into a [`MusicDemoReport`].
pub fn analyze_smf_bytes(bytes: &[u8]) -> Result<MusicDemoReport, MusicWasmError> {
    let file = read_smf(bytes)?;
    let progression = lift_to_progression(&file, ProgressionLiftOpts::default())?;
    let counterpoint = lift_to_counterpoint(&file, CounterpointLiftOpts::default())?;
    let piano_roll = lift_to_piano_roll(&file)?;
    let diff_roll = lift_to_diff_roll(&file)?;
    let score = score_from_counterpoint(&counterpoint)?;
    report(progression, counterpoint, piano_roll, diff_roll, score)
}

/// Decodes a music-file surface, lowers it to MIDI, and analyzes the result.
pub fn analyze_music_file(input: &str) -> Result<MusicDemoReport, MusicWasmError> {
    let score = decode_music_file(input)?;
    let smf = lower_score(&score, &LowerOpts::default())?;
    analyze_smf_bytes(&write_smf(&smf)?)
}

/// Decodes a music-file surface and lowers it to MIDI event frames.
pub fn lower_music_file_to_frames(input: &str) -> Result<Vec<MidiEventFrame>, MusicWasmError> {
    let score = decode_music_file(input)?;
    let smf = lower_score(&score, &LowerOpts::default())?;
    Ok(sim_lib_midi_wasm_frame::smf_to_event_frames(&smf))
}

/// Lifts MIDI event frames into a counterpoint score encoded as a music file.
pub fn lift_frames_to_music_file(frames: &[MidiEventFrame]) -> Result<String, MusicWasmError> {
    let file = frame_array_to_smf(frames, SmfFormat::SingleTrack)?;
    let counterpoint = lift_to_counterpoint(&file, CounterpointLiftOpts::default())?;
    let score = score_from_counterpoint(&counterpoint)?;
    Ok(score_citizen_text(&score)?)
}

fn report(
    progression: Progression,
    counterpoint: Counterpoint,
    piano_roll: PianoRoll,
    diff_roll: DiffRoll,
    score: Score,
) -> Result<MusicDemoReport, MusicWasmError> {
    Ok(MusicDemoReport {
        progression: encode_progression(&progression),
        counterpoint: encode_counterpoint(&counterpoint),
        piano_roll: encode_piano_roll(&piano_roll),
        diff_roll: encode_diff_roll(&diff_roll),
        music_file: score_citizen_text(&score)?,
    })
}

fn score_from_counterpoint(counterpoint: &Counterpoint) -> Result<Score, MusicWasmError> {
    Ok(Score::new(
        120,
        (4, 4),
        None,
        Music::Counterpoint(counterpoint.clone()),
    )?)
}

fn score_citizen_text(score: &Score) -> Result<String, MusicShapeError> {
    let form = encode_music_file(score)?;
    Ok(format!(
        "#({} v1 {})",
        music_score_class_symbol(),
        encode_string_literal(&form)
    ))
}
