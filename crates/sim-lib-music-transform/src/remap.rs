use std::collections::BTreeMap;

use sim_lib_music_core::{Music, MusicObject};
use sim_lib_pitch_chord::Chord;
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_scale::Scale;

use crate::{
    CallablePitchMap, TransformDiagnostic, TransformDiagnosticCode, TransformError,
    TransformReport, map_pitches_with_diagnostics, nearest_pitch_in_scale, note_with_pitch,
};

/// Microtonal tuning offset expressed in cents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuningRemap {
    /// Offset in cents (100 cents to a semitone).
    pub cents: i32,
}

impl TuningRemap {
    /// Builds a tuning remap from an offset in cents.
    pub fn new(cents: i32) -> Self {
        Self { cents }
    }

    /// Rounds the cents offset to the nearest whole semitone.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_transform::TuningRemap;
    ///
    /// assert_eq!(TuningRemap::new(150).semitone_delta(), 2);
    /// assert_eq!(TuningRemap::new(40).semitone_delta(), 0);
    /// ```
    pub fn semitone_delta(&self) -> i32 {
        (f64::from(self.cents) / 100.0).round() as i32
    }
}

/// Integer 2x3 affine matrix mapping `(degree, octave, 1)` to a new pitch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntMatrix {
    /// Coefficients producing the target scale degree.
    pub degree_row: [i32; 3],
    /// Coefficients producing the target octave.
    pub octave_row: [i32; 3],
    /// Shared divisor applied to both rows' results.
    pub divisor: i32,
}

impl IntMatrix {
    /// Builds a matrix from its degree row, octave row, and divisor.
    pub fn new(degree_row: [i32; 3], octave_row: [i32; 3], divisor: i32) -> Self {
        Self {
            degree_row,
            octave_row,
            divisor,
        }
    }

    /// Returns the identity matrix, which leaves degree and octave unchanged.
    pub fn identity() -> Self {
        Self::new([1, 0, 0], [0, 1, 0], 1)
    }
}

/// A pitch remapping strategy applied note by note across material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PitchRemap {
    /// Shift every pitch by a fixed number of semitones.
    Chromatic(i32),
    /// Transpose by scale degrees within `scale`.
    ScaleDegree {
        /// Scale that defines the diatonic steps.
        scale: Scale,
        /// Number of scale degrees to move.
        steps: i32,
    },
    /// Replace one pitch class with another, keeping the octave.
    PitchClass {
        /// Pitch class to match.
        from: PitchClass,
        /// Pitch class to substitute.
        to: PitchClass,
    },
    /// Remap MIDI drum keys via an explicit key-to-key table.
    DrumKey(BTreeMap<u8, u8>),
    /// Snap each pitch to the nearest tone of a chord built on `scale`.
    ChordTone {
        /// Scale the chord is drawn from.
        scale: Scale,
        /// Chord degree within the scale.
        degree: usize,
    },
    /// Apply a microtonal tuning offset.
    Tuning(TuningRemap),
    /// Offset each pitch by a per-degree vector of semitone offsets.
    Vector {
        /// Scale used to find each pitch's degree.
        scale: Scale,
        /// Per-degree semitone offsets, indexed cyclically.
        offsets: Vec<i32>,
    },
    /// Apply an integer affine [`IntMatrix`] over `(degree, octave)`.
    Matrix {
        /// Scale used to resolve degrees.
        scale: Scale,
        /// Transformation matrix.
        matrix: IntMatrix,
    },
    /// Apply a named [`CallablePitchMap`].
    Callable(CallablePitchMap),
}

impl PitchRemap {
    /// Applies the remap and returns just the music, discarding diagnostics.
    pub fn apply(&self, object: &dyn MusicObject) -> Result<Music, TransformError> {
        Ok(self.apply_report(object)?.music)
    }

    /// Applies the remap, returning the music together with any diagnostics.
    pub fn apply_report(
        &self,
        object: &dyn MusicObject,
    ) -> Result<TransformReport, TransformError> {
        match self {
            Self::Chromatic(semitones) => {
                Ok(TransformReport::clean(crate::map_notes(object, |note| {
                    let pitch = note.pitch.transpose(*semitones);
                    note_with_pitch(note, pitch)
                })?))
            }
            Self::ScaleDegree { scale, steps } => {
                map_pitches_with_diagnostics(object, "pitch-remap", |pitch| {
                    scale
                        .transpose_diatonic(pitch, *steps)
                        .map_err(|_| out_of_scale_diagnostic("pitch-remap", pitch, "scale remap"))
                })
            }
            Self::PitchClass { from, to } => {
                Ok(TransformReport::clean(crate::map_notes(object, |note| {
                    let pitch = if note.pitch.class == *from {
                        Pitch {
                            class: *to,
                            octave: note.pitch.octave,
                        }
                    } else {
                        note.pitch
                    };
                    note_with_pitch(note, pitch)
                })?))
            }
            Self::DrumKey(map) => map_pitches_with_diagnostics(object, "pitch-remap", |pitch| {
                let Some(key) = pitch.to_midi() else {
                    return Err(TransformDiagnostic::new(
                        TransformDiagnosticCode::MissingMidiKey,
                        "pitch-remap",
                        "drum-key remap needs a MIDI key",
                    ));
                };
                Ok(map
                    .get(&key)
                    .map(|mapped| Pitch::from_midi(*mapped))
                    .unwrap_or(pitch))
            }),
            Self::ChordTone { scale, degree } => {
                map_pitches_with_diagnostics(object, "pitch-remap", |pitch| {
                    nearest_pitch_in_chord(pitch, *scale, *degree)
                })
            }
            Self::Tuning(tuning) => Ok(TransformReport::clean(crate::map_notes(object, |note| {
                let pitch = note.pitch.transpose(tuning.semitone_delta());
                note_with_pitch(note, pitch)
            })?)),
            Self::Vector { scale, offsets } => {
                map_pitches_with_diagnostics(object, "pitch-remap", |pitch| {
                    vector_remap(pitch, *scale, offsets)
                })
            }
            Self::Matrix { scale, matrix } => {
                map_pitches_with_diagnostics(object, "pitch-remap", |pitch| {
                    matrix_remap(pitch, *scale, matrix)
                })
            }
            Self::Callable(map) => Ok(TransformReport::clean(crate::map_notes(object, |note| {
                let pitch = map.map_pitch(note.pitch);
                note_with_pitch(note, pitch)
            })?)),
        }
    }
}

fn vector_remap(pitch: Pitch, scale: Scale, offsets: &[i32]) -> Result<Pitch, TransformDiagnostic> {
    if offsets.is_empty() {
        return Err(TransformDiagnostic::new(
            TransformDiagnosticCode::UnsupportedMapping,
            "pitch-remap",
            "vector remap needs at least one offset",
        ));
    }
    let degree = scale
        .degree_of(pitch.class)
        .ok_or_else(|| out_of_scale_diagnostic("pitch-remap", pitch, "vector remap"))?;
    let offset = offsets[(degree - 1) % offsets.len()];
    Ok(pitch.transpose(offset))
}

fn matrix_remap(
    pitch: Pitch,
    scale: Scale,
    matrix: &IntMatrix,
) -> Result<Pitch, TransformDiagnostic> {
    if matrix.divisor <= 0 {
        return Err(TransformDiagnostic::new(
            TransformDiagnosticCode::InvalidMatrix,
            "pitch-remap",
            "matrix divisor must be positive",
        ));
    }
    let degree = scale
        .degree_of(pitch.class)
        .ok_or_else(|| out_of_scale_diagnostic("pitch-remap", pitch, "matrix remap"))?
        as i32;
    let input = [degree, i32::from(pitch.octave), 1];
    let target_degree = dot(matrix.degree_row, input) / matrix.divisor;
    if target_degree <= 0 {
        return Err(TransformDiagnostic::new(
            TransformDiagnosticCode::InvalidMatrix,
            "pitch-remap",
            "matrix remap produced a non-positive scale degree",
        ));
    }
    let target_octave = dot(matrix.octave_row, input) / matrix.divisor;
    let octave = i16::try_from(target_octave).map_err(|_| {
        TransformDiagnostic::new(
            TransformDiagnosticCode::InvalidMatrix,
            "pitch-remap",
            "matrix remap produced an octave outside the supported range",
        )
    })?;
    let target_degree = usize::try_from(target_degree).map_err(|_| {
        TransformDiagnostic::new(
            TransformDiagnosticCode::InvalidMatrix,
            "pitch-remap",
            "matrix remap produced a scale degree outside the supported range",
        )
    })?;
    let class = scale.pitch_at_degree(target_degree).map_err(|_| {
        TransformDiagnostic::new(
            TransformDiagnosticCode::InvalidMatrix,
            "pitch-remap",
            "matrix remap produced a non-positive scale degree",
        )
    })?;
    Ok(Pitch { class, octave })
}

fn nearest_pitch_in_chord(
    pitch: Pitch,
    scale: Scale,
    degree: usize,
) -> Result<Pitch, TransformDiagnostic> {
    let chord = Chord::chord_tones_in(scale, degree, pitch.octave).map_err(|_| {
        TransformDiagnostic::new(
            TransformDiagnosticCode::UnsupportedMapping,
            "pitch-remap",
            "chord-tone remap needs a one-based scale degree",
        )
    })?;
    Ok(chord
        .pitches()
        .into_iter()
        .min_by_key(|candidate| {
            (
                (candidate.semitone() - pitch.semitone()).abs(),
                candidate.semitone(),
            )
        })
        .unwrap_or_else(|| nearest_pitch_in_scale(pitch, &scale)))
}

fn out_of_scale_diagnostic(
    transform: &'static str,
    pitch: Pitch,
    action: &'static str,
) -> TransformDiagnostic {
    TransformDiagnostic::new(
        TransformDiagnosticCode::PitchOutOfScale,
        transform,
        format!("{action} cannot place pitch class {}", pitch.class.value()),
    )
}

fn dot(row: [i32; 3], input: [i32; 3]) -> i32 {
    row.into_iter()
        .zip(input)
        .map(|(left, right)| left * right)
        .sum()
}
