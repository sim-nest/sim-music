use sim_lib_music_core::Music;
use sim_lib_pitch_core::Pitch;

/// Classification of a problem a transform encountered while running.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TransformDiagnosticCode {
    /// An inversion axis could not be resolved to a concrete pitch.
    InvalidAxis,
    /// A remap matrix had a non-positive divisor or out-of-range output.
    InvalidMatrix,
    /// A frequency or time ratio was not positive.
    InvalidRatio,
    /// A time map was malformed (too few points or non-monotonic).
    InvalidTimeMap,
    /// A pitch lacked the MIDI key a drum-key remap requires.
    MissingMidiKey,
    /// A time map produced a negative note duration.
    NonPositiveDuration,
    /// A pitch class fell outside the scale a remap needs.
    PitchOutOfScale,
    /// A mapping was requested that the transform cannot perform.
    UnsupportedMapping,
}

/// A single diagnostic emitted by a transform, tagged with its origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformDiagnostic {
    /// Machine-readable classification of the problem.
    pub code: TransformDiagnosticCode,
    /// Name of the transform that produced the diagnostic.
    pub transform: &'static str,
    /// Human-readable description of the problem.
    pub message: String,
}

impl TransformDiagnostic {
    /// Builds a diagnostic from its code, originating transform, and message.
    pub fn new(
        code: TransformDiagnosticCode,
        transform: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            transform,
            message: message.into(),
        }
    }
}

/// Result of a transform: the produced music plus any diagnostics raised.
#[derive(Clone, Debug)]
pub struct TransformReport {
    /// The transformed musical material.
    pub music: Music,
    /// Diagnostics gathered while producing [`Self::music`].
    pub diagnostics: Vec<TransformDiagnostic>,
}

impl TransformReport {
    /// Wraps music with no diagnostics.
    pub fn clean(music: Music) -> Self {
        Self {
            music,
            diagnostics: Vec::new(),
        }
    }

    /// Wraps music with a single diagnostic.
    pub fn with_diagnostic(music: Music, diagnostic: TransformDiagnostic) -> Self {
        Self {
            music,
            diagnostics: vec![diagnostic],
        }
    }

    /// Returns whether the report carries any diagnostics.
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

/// Named pitch map that applies a fixed chromatic offset to each pitch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallablePitchMap {
    /// Display name of the map.
    pub name: String,
    /// Chromatic offset in semitones applied to each pitch.
    pub semitone_delta: i32,
}

impl CallablePitchMap {
    /// Builds a named pitch map with the given semitone offset.
    pub fn new(name: impl Into<String>, semitone_delta: i32) -> Self {
        Self {
            name: name.into(),
            semitone_delta,
        }
    }

    /// Applies the map's offset to a pitch.
    pub fn map_pitch(&self, pitch: Pitch) -> Pitch {
        pitch.transpose(self.semitone_delta)
    }
}
