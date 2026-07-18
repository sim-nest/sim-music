use num_rational::Ratio;

use sim_lib_music_core::{Music, MusicObject, Note, Time, TimedNote};
use sim_lib_pitch_chord::Chord;
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_scale::Scale;

use crate::{
    CallablePitchMap, PitchRemap, RetrogradeMode, TransformDiagnostic, TransformDiagnosticCode,
    TransformReport, canonical_roll, map_notes, retrograde_with_mode, to_piano_roll,
};

/// Amount and kind of pitch displacement for a transpose transform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PitchDelta {
    /// Move by a fixed number of semitones.
    Semitones(i32),
    /// Move by whole octaves.
    Octaves(i16),
    /// Move by scale degrees within `scale`.
    ScaleDegrees {
        /// Scale that defines the diatonic steps.
        scale: Scale,
        /// Number of scale degrees to move.
        steps: i32,
    },
    /// Move by the interval nearest to a frequency ratio.
    FrequencyRatio(Ratio<i64>),
    /// Move via a named [`CallablePitchMap`].
    Custom(CallablePitchMap),
}

/// Transpose transform that shifts pitch by a [`PitchDelta`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransposeTransform {
    /// Displacement applied to every note.
    pub by: PitchDelta,
}

impl TransposeTransform {
    /// Builds a transpose transform from a pitch delta.
    pub fn new(by: PitchDelta) -> Self {
        Self { by }
    }

    /// Applies the transpose and returns just the music.
    pub fn apply(&self, object: &dyn MusicObject) -> Music {
        self.apply_report(object).music
    }

    /// Applies the transpose, returning the music and any diagnostics.
    pub fn apply_report(&self, object: &dyn MusicObject) -> TransformReport {
        match &self.by {
            PitchDelta::Semitones(semitones) => TransformReport::clean(map_notes(object, |note| {
                let pitch = note.pitch.transpose(*semitones);
                note_with_pitch(note, pitch)
            })),
            PitchDelta::Octaves(octaves) => {
                let semitones = i32::from(*octaves) * 12;
                TransformReport::clean(map_notes(object, |note| {
                    let pitch = note.pitch.transpose(semitones);
                    note_with_pitch(note, pitch)
                }))
            }
            PitchDelta::ScaleDegrees { scale, steps } => {
                map_pitches_with_diagnostics(object, "transpose", |pitch| {
                    scale.transpose_diatonic(pitch, *steps).map_err(|_| {
                        TransformDiagnostic::new(
                            TransformDiagnosticCode::PitchOutOfScale,
                            "transpose",
                            format!("pitch class {} is not in the scale", pitch.class.value()),
                        )
                    })
                })
            }
            PitchDelta::FrequencyRatio(ratio) => match ratio_to_semitones(ratio) {
                Some(semitones) => TransformReport::clean(map_notes(object, |note| {
                    let pitch = note.pitch.transpose(semitones);
                    note_with_pitch(note, pitch)
                })),
                None => TransformReport::with_diagnostic(
                    Music::PianoRoll(to_piano_roll(object)),
                    TransformDiagnostic::new(
                        TransformDiagnosticCode::InvalidRatio,
                        "transpose",
                        "frequency ratio must be positive",
                    ),
                ),
            },
            PitchDelta::Custom(map) => TransformReport::clean(map_notes(object, |note| {
                let pitch = map.map_pitch(note.pitch);
                note_with_pitch(note, pitch)
            })),
        }
    }
}

/// Named inversion axis carrying an explicit pitch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomPitchAxis {
    /// Display name of the axis.
    pub name: String,
    /// Pitch the inversion mirrors about.
    pub axis: Pitch,
}

impl CustomPitchAxis {
    /// Builds a named axis from a pitch.
    pub fn new(name: impl Into<String>, axis: Pitch) -> Self {
        Self {
            name: name.into(),
            axis,
        }
    }
}

/// Specification of the axis an [`InvertTransform`] mirrors about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PitchAxis {
    /// Mirror about a concrete pitch.
    Pitch(Pitch),
    /// Mirror pitch classes about a pitch class, keeping octaves.
    PitchClass(PitchClass),
    /// Mirror about a scale degree resolved at a given octave.
    ScaleDegree {
        /// Scale the degree belongs to.
        scale: Scale,
        /// One-based scale degree.
        degree: usize,
        /// Octave the resolved axis pitch sits in.
        octave: i16,
    },
    /// Mirror about the root note of a chord.
    ChordRoot(Chord),
    /// Mirror about a pitch interpreted as a frequency center.
    Frequency(Pitch),
    /// Mirror about a named [`CustomPitchAxis`].
    Custom(CustomPitchAxis),
}

/// Inversion transform that mirrors pitch about a [`PitchAxis`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvertTransform {
    /// Axis every note is mirrored about.
    pub axis: PitchAxis,
}

impl InvertTransform {
    /// Builds an inversion transform from an axis.
    pub fn new(axis: PitchAxis) -> Self {
        Self { axis }
    }

    /// Applies the inversion and returns just the music.
    pub fn apply(&self, object: &dyn MusicObject) -> Music {
        self.apply_report(object).music
    }

    /// Applies the inversion, returning the music and any diagnostics.
    pub fn apply_report(&self, object: &dyn MusicObject) -> TransformReport {
        match &self.axis {
            PitchAxis::PitchClass(axis) => TransformReport::clean(map_notes(object, |note| {
                let pitch = Pitch {
                    class: note.pitch.class.invert(*axis),
                    octave: note.pitch.octave,
                };
                note_with_pitch(note, pitch)
            })),
            axis => match resolve_axis(axis) {
                Some(resolved) => TransformReport::clean(map_notes(object, |note| {
                    let pitch = note.pitch.invert(resolved);
                    note_with_pitch(note, pitch)
                })),
                None => TransformReport::with_diagnostic(
                    Music::PianoRoll(to_piano_roll(object)),
                    TransformDiagnostic::new(
                        TransformDiagnosticCode::InvalidAxis,
                        "invert",
                        "inversion axis cannot be resolved",
                    ),
                ),
            },
        }
    }
}

/// Retrograde transform that reverses material under a [`RetrogradeMode`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetrogradeTransform {
    /// Mode controlling how reversed notes are placed.
    pub mode: RetrogradeMode,
}

impl RetrogradeTransform {
    /// Builds a retrograde transform with the given mode.
    pub fn new(mode: RetrogradeMode) -> Self {
        Self { mode }
    }

    /// Reverses the material and returns the music.
    pub fn apply(&self, object: &dyn MusicObject) -> Music {
        retrograde_with_mode(object, self.mode)
    }

    /// Reverses the material, returning a clean report (no diagnostics).
    pub fn apply_report(&self, object: &dyn MusicObject) -> TransformReport {
        TransformReport::clean(self.apply(object))
    }
}

impl Default for RetrogradeTransform {
    fn default() -> Self {
        Self {
            mode: RetrogradeMode::Cutout,
        }
    }
}

/// One `(input -> output)` breakpoint in a piecewise-linear time map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeMapPoint {
    /// Source time of the breakpoint.
    pub input: Time,
    /// Mapped destination time of the breakpoint.
    pub output: Time,
}

impl TimeMapPoint {
    /// Builds a breakpoint from input and output times.
    pub fn new(input: Time, output: Time) -> Self {
        Self { input, output }
    }
}

/// A warp anchor pairing a source time with its target time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarpMarker {
    /// Time in the source material.
    pub source: Time,
    /// Time it should land on after warping.
    pub target: Time,
}

impl WarpMarker {
    /// Builds a warp marker from source and target times.
    pub fn new(source: Time, target: Time) -> Self {
        Self { source, target }
    }
}

/// Strategy for stretching or warping material along the time axis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StretchPolicy {
    /// Scale time inversely to a tempo ratio (faster tempo, shorter time).
    TempoRatio(Time),
    /// Scale time directly by a time ratio.
    TimeRatio(Time),
    /// Scale time so the material fills a target duration.
    FitToDuration(Time),
    /// Warp time through an explicit piecewise-linear map.
    TimeMap(Vec<TimeMapPoint>),
    /// Warp time through a set of [`WarpMarker`] anchors.
    WarpMarkers(Vec<WarpMarker>),
}

impl StretchPolicy {
    /// Applies the stretch and returns just the music.
    pub fn apply(&self, object: &dyn MusicObject) -> Music {
        self.apply_report(object).music
    }

    /// Applies the stretch, returning the music and any diagnostics.
    pub fn apply_report(&self, object: &dyn MusicObject) -> TransformReport {
        match self {
            Self::TempoRatio(ratio) => match positive_ratio(*ratio) {
                Some(factor) => stretch_by_factor(object, factor.recip()),
                None => invalid_ratio_report(object, "stretch", "tempo ratio must be positive"),
            },
            Self::TimeRatio(ratio) => match positive_ratio(*ratio) {
                Some(factor) => stretch_by_factor(object, factor),
                None => invalid_ratio_report(object, "stretch", "time ratio must be positive"),
            },
            Self::FitToDuration(target) => {
                let current = object.duration();
                if current <= Time::from_integer(0) || *target <= Time::from_integer(0) {
                    invalid_ratio_report(
                        object,
                        "stretch",
                        "source and target duration must be positive",
                    )
                } else {
                    stretch_by_factor(object, *target / current)
                }
            }
            Self::TimeMap(points) => stretch_with_time_map(object, points),
            Self::WarpMarkers(markers) => {
                let points = markers
                    .iter()
                    .map(|marker| TimeMapPoint::new(marker.source, marker.target))
                    .collect::<Vec<_>>();
                stretch_with_time_map(object, &points)
            }
        }
    }
}

/// A single step in a [`TransformChain`], wrapping one transform kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformStep {
    /// Transpose step.
    Transpose(TransposeTransform),
    /// Inversion step.
    Invert(InvertTransform),
    /// Retrograde step.
    Retrograde(RetrogradeTransform),
    /// Time stretch step.
    Stretch(StretchPolicy),
    /// Pitch remap step.
    Remap(PitchRemap),
}

impl TransformStep {
    /// Applies this step, returning the music and any diagnostics.
    pub fn apply_report(&self, object: &dyn MusicObject) -> TransformReport {
        match self {
            Self::Transpose(transform) => transform.apply_report(object),
            Self::Invert(transform) => transform.apply_report(object),
            Self::Retrograde(transform) => transform.apply_report(object),
            Self::Stretch(policy) => policy.apply_report(object),
            Self::Remap(remap) => remap.apply_report(object),
        }
    }
}

/// An ordered pipeline of [`TransformStep`] values applied in sequence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransformChain {
    /// Steps applied left to right.
    pub steps: Vec<TransformStep>,
}

impl TransformChain {
    /// Builds a chain from an ordered list of steps.
    pub fn new(steps: Vec<TransformStep>) -> Self {
        Self { steps }
    }

    /// Applies the whole chain and returns just the music.
    pub fn apply(&self, object: &dyn MusicObject) -> Music {
        self.apply_report(object).music
    }

    /// Applies the whole chain, accumulating diagnostics from every step.
    pub fn apply_report(&self, object: &dyn MusicObject) -> TransformReport {
        let mut current = Music::PianoRoll(to_piano_roll(object));
        let mut diagnostics = Vec::new();
        for step in &self.steps {
            let report = step.apply_report(&current);
            current = report.music;
            diagnostics.extend(report.diagnostics);
        }
        TransformReport {
            music: current,
            diagnostics,
        }
    }
}

pub(crate) fn map_pitches_with_diagnostics(
    object: &dyn MusicObject,
    transform: &'static str,
    mut map: impl FnMut(Pitch) -> Result<Pitch, TransformDiagnostic>,
) -> TransformReport {
    let roll = to_piano_roll(object);
    let mut diagnostics = Vec::new();
    let items = roll
        .items
        .into_iter()
        .map(|mut item| {
            match map(item.note.pitch) {
                Ok(pitch) => item.note.pitch = pitch,
                Err(mut diagnostic) => {
                    diagnostic.transform = transform;
                    diagnostics.push(diagnostic);
                }
            }
            item
        })
        .collect();
    TransformReport {
        music: Music::PianoRoll(canonical_roll(items)),
        diagnostics,
    }
}

pub(crate) fn note_with_pitch(note: Note, pitch: Pitch) -> Note {
    Note { pitch, ..note }
}

fn resolve_axis(axis: &PitchAxis) -> Option<Pitch> {
    match axis {
        PitchAxis::Pitch(pitch) | PitchAxis::Frequency(pitch) => Some(*pitch),
        PitchAxis::PitchClass(_) => None,
        PitchAxis::ScaleDegree {
            scale,
            degree,
            octave,
        } => scale.pitch_at_degree(*degree).ok().map(|class| Pitch {
            class,
            octave: *octave,
        }),
        PitchAxis::ChordRoot(chord) => chord.notes.first().copied(),
        PitchAxis::Custom(axis) => Some(axis.axis),
    }
}

fn ratio_to_semitones(ratio: &Ratio<i64>) -> Option<i32> {
    if *ratio <= Ratio::from_integer(0) {
        return None;
    }
    let value = *ratio.numer() as f64 / *ratio.denom() as f64;
    Some((value.log2() * 12.0).round() as i32)
}

fn positive_ratio(ratio: Time) -> Option<Time> {
    (ratio > Time::from_integer(0)).then_some(ratio)
}

fn invalid_ratio_report(
    object: &dyn MusicObject,
    transform: &'static str,
    message: &'static str,
) -> TransformReport {
    TransformReport::with_diagnostic(
        Music::PianoRoll(to_piano_roll(object)),
        TransformDiagnostic::new(TransformDiagnosticCode::InvalidRatio, transform, message),
    )
}

fn stretch_by_factor(object: &dyn MusicObject, factor: Time) -> TransformReport {
    TransformReport::clean(Music::PianoRoll(canonical_roll(
        to_piano_roll(object)
            .items
            .into_iter()
            .map(|mut item| {
                item.onset *= factor;
                item.note.duration *= factor;
                item
            })
            .collect(),
    )))
}

fn stretch_with_time_map(object: &dyn MusicObject, points: &[TimeMapPoint]) -> TransformReport {
    match validate_time_map(points) {
        Ok(()) => {
            let roll = to_piano_roll(object);
            let mut diagnostics = Vec::new();
            let items = roll
                .items
                .into_iter()
                .map(|item| remap_timed_note(item, points, &mut diagnostics))
                .collect();
            TransformReport {
                music: Music::PianoRoll(canonical_roll(items)),
                diagnostics,
            }
        }
        Err(diagnostic) => {
            TransformReport::with_diagnostic(Music::PianoRoll(to_piano_roll(object)), diagnostic)
        }
    }
}

fn remap_timed_note(
    mut item: TimedNote,
    points: &[TimeMapPoint],
    diagnostics: &mut Vec<TransformDiagnostic>,
) -> TimedNote {
    let start = map_time(item.onset, points);
    let end = map_time(item.onset + item.note.duration, points);
    let duration = end - start;
    item.onset = start;
    if duration >= Time::from_integer(0) {
        item.note.duration = duration;
    } else {
        diagnostics.push(TransformDiagnostic::new(
            TransformDiagnosticCode::NonPositiveDuration,
            "stretch",
            "time map produced a negative duration",
        ));
    }
    item
}

fn validate_time_map(points: &[TimeMapPoint]) -> Result<(), TransformDiagnostic> {
    if points.len() < 2 {
        return Err(TransformDiagnostic::new(
            TransformDiagnosticCode::InvalidTimeMap,
            "stretch",
            "time map needs at least two points",
        ));
    }
    for window in points.windows(2) {
        if window[0].input >= window[1].input || window[0].output > window[1].output {
            return Err(TransformDiagnostic::new(
                TransformDiagnosticCode::InvalidTimeMap,
                "stretch",
                "time map points must increase by input and not reverse output",
            ));
        }
    }
    Ok(())
}

fn map_time(time: Time, points: &[TimeMapPoint]) -> Time {
    let (left, right) = segment_for(time, points);
    let input_span = right.input - left.input;
    let output_span = right.output - left.output;
    left.output + (time - left.input) * output_span / input_span
}

fn segment_for(time: Time, points: &[TimeMapPoint]) -> (&TimeMapPoint, &TimeMapPoint) {
    for window in points.windows(2) {
        if time <= window[1].input {
            return (&window[0], &window[1]);
        }
    }
    let last = points.len() - 1;
    (&points[last - 1], &points[last])
}
