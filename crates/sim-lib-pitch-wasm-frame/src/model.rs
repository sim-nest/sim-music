use thiserror::Error;

use sim_codec::encode_string_literal;
use sim_lib_pitch_core::{Pitch, PitchError, parse_pitch};
use sim_lib_pitch_dissonance::{PitchDissonanceRegistry, PitchDissonanceScore};
use sim_lib_pitch_namer::{ClusterLabel, LabelContext, NamerRegistry};
use sim_lib_pitch_scale::{Key, Mode};
use sim_lib_pitch_set::PitchClassMask;
use sim_lib_pitch_shapes::{encode_pitch_class_mask, pitch_class_mask_class_symbol};

/// Error returned when analyzing pitch text fails.
#[derive(Debug, Error)]
pub enum PitchWasmError {
    /// A pitch token could not be parsed.
    #[error(transparent)]
    Pitch(#[from] PitchError),
    /// The input contained no pitches.
    #[error("no pitches were provided")]
    EmptyInput,
}

/// A frame-safe view of one [`ClusterLabel`]: its school, text, and any diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchLabelView {
    /// The naming school that produced the label.
    pub school: String,
    /// The label text.
    pub text: String,
    /// An optional diagnostic explaining a degraded label.
    pub diagnostic: Option<String>,
}

/// A frame-safe view of one dissonance model's score.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchDissonanceView {
    /// The name of the model that produced the score.
    pub model: String,
    /// The computed dissonance score.
    pub score: f64,
}

/// A complete analysis report for a set of pitches, ready to serialize across a
/// frame boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchDemoReport {
    /// The input pitches as canonical strings.
    pub pitches: Vec<String>,
    /// The canonical pitch-class mask as a citizen read-construct form.
    pub canonical_mask: String,
    /// The labels produced by every naming school.
    pub labels: Vec<PitchLabelView>,
    /// The scores produced by every dissonance model.
    pub dissonance: Vec<PitchDissonanceView>,
}

/// Analyzes whitespace- or comma-separated pitch text, defaulting the key context
/// to the major key on the first pitch.
///
/// Returns [`PitchWasmError::EmptyInput`] if no pitches are present, or a pitch
/// parse error for malformed tokens.
pub fn analyze_pitch_text(input: &str) -> Result<PitchDemoReport, PitchWasmError> {
    let pitches = parse_pitch_text(input)?;
    Ok(build_report(&pitches, default_context(&pitches)))
}

/// Analyzes pitch text using an explicit `key` context for the key-relative naming
/// schools.
///
/// Returns [`PitchWasmError::EmptyInput`] if no pitches are present, or a pitch
/// parse error for malformed tokens.
pub fn analyze_pitch_text_with_key(
    input: &str,
    key: Option<Key>,
) -> Result<PitchDemoReport, PitchWasmError> {
    let pitches = parse_pitch_text(input)?;
    let context = LabelContext {
        root: pitches.first().map(|pitch| pitch.class),
        key,
    };
    Ok(build_report(&pitches, context))
}

fn parse_pitch_text(input: &str) -> Result<Vec<Pitch>, PitchWasmError> {
    let pitches = input
        .split(|ch: char| ch.is_whitespace() || [',', ';'].contains(&ch))
        .filter(|part| !part.is_empty())
        .map(parse_pitch)
        .collect::<Result<Vec<_>, _>>()?;
    if pitches.is_empty() {
        return Err(PitchWasmError::EmptyInput);
    }
    Ok(pitches)
}

fn default_context(pitches: &[Pitch]) -> LabelContext {
    let root = pitches.first().map(|pitch| pitch.class);
    let key = root.map(|tonic| Key {
        tonic,
        mode: Mode::Major,
    });
    LabelContext { root, key }
}

fn build_report(pitches: &[Pitch], context: LabelContext) -> PitchDemoReport {
    let mask = pitch_mask(pitches);
    let namers = NamerRegistry::new_with_builtins();
    let dissonance = PitchDissonanceRegistry::new_with_builtins();
    PitchDemoReport {
        pitches: pitches.iter().map(display_pitch).collect(),
        canonical_mask: pitch_class_mask_citizen(mask.normalize()),
        labels: namers
            .label_all(mask, &context)
            .into_iter()
            .map(label_view)
            .collect(),
        dissonance: dissonance
            .analyze_all(mask, &context)
            .into_iter()
            .map(dissonance_view)
            .collect(),
    }
}

fn pitch_mask(pitches: &[Pitch]) -> PitchClassMask {
    PitchClassMask::from_pitch_classes(&pitches.iter().map(|pitch| pitch.class).collect::<Vec<_>>())
}

fn label_view(label: ClusterLabel) -> PitchLabelView {
    PitchLabelView {
        school: label.school.to_string(),
        text: label.text,
        diagnostic: label.meta.diagnostic,
    }
}

fn dissonance_view(score: PitchDissonanceScore) -> PitchDissonanceView {
    PitchDissonanceView {
        model: score.model.to_owned(),
        score: score.score,
    }
}

fn display_pitch(pitch: &Pitch) -> String {
    format!("{}{}", pitch.class.canonical_name(), pitch.octave)
}

fn pitch_class_mask_citizen(mask: PitchClassMask) -> String {
    let form = encode_pitch_class_mask(mask);
    format!(
        "#({} v1 {})",
        pitch_class_mask_class_symbol(),
        encode_string_literal(&form)
    )
}
