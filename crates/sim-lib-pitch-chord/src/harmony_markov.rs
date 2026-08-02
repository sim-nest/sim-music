use std::cmp::Ordering;

use sim_lib_numbers_stats::{MarkovModel, MarkovPolicy, ModelReport, fit_markov};
use sim_lib_pitch_scale::{Key, Mode};
use sim_lib_pitch_set::PitchClassMask;

use crate::{
    ChordTemplate, HarmonyError, HarmonyEvaluationContext, HarmonyMetric, HarmonyMetricObservation,
    HarmonyMetricResolver, validate_id,
};

/// One domain-owned state projected from a key and realized chord template.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HarmonyTransitionState {
    key: Key,
    chord: PitchClassMask,
}

impl HarmonyTransitionState {
    /// Projects a key and chord template into a register-independent state.
    pub fn from_template(key: Key, chord: &ChordTemplate) -> Result<Self, HarmonyError> {
        Ok(Self {
            key,
            chord: chord.pitch_set()?,
        })
    }

    /// Returns the musical key retained by this state.
    pub fn key(self) -> Key {
        self.key
    }

    /// Returns the realized pitch-class set retained by this state.
    pub fn chord(self) -> PitchClassMask {
        self.chord
    }

    /// Returns the canonical domain-owned label used for stable model encoding.
    pub fn stable_label(&self) -> String {
        format!(
            "{}:{}:{:03x}",
            self.key.tonic.canonical_name(),
            self.key.mode.name(),
            self.chord.bits()
        )
    }
}

impl Ord for HarmonyTransitionState {
    fn cmp(&self, other: &Self) -> Ordering {
        state_key(*self).cmp(&state_key(*other))
    }
}

impl PartialOrd for HarmonyTransitionState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// One caller-provided chord progression and its declared key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarmonyCorpusSequence {
    /// Key paired with every chord state in this sequence.
    pub key: Key,
    /// Ordered chord templates; identifiers and voicings do not affect states.
    pub progression: Vec<ChordTemplate>,
}

impl HarmonyCorpusSequence {
    /// Builds a corpus sequence in one declared key.
    pub fn new(key: Key, progression: Vec<ChordTemplate>) -> Self {
        Self { key, progression }
    }

    fn states(&self) -> Result<Vec<HarmonyTransitionState>, HarmonyError> {
        self.progression
            .iter()
            .map(|chord| HarmonyTransitionState::from_template(self.key, chord))
            .collect()
    }
}

/// Fits the generic statistics model after projecting only music-owned states.
pub fn fit_harmony_markov(
    sequences: &[HarmonyCorpusSequence],
    policy: MarkovPolicy,
) -> Result<ModelReport<MarkovModel<HarmonyTransitionState>>, HarmonyError> {
    let states = sequences
        .iter()
        .map(HarmonyCorpusSequence::states)
        .collect::<Result<Vec<_>, _>>()?;
    fit_markov(&states, policy).map_err(markov_error)
}

/// Soft-metric adapter composing one learned model with an existing resolver.
pub struct LearnedTransitionResolver<'a> {
    model_id: String,
    key: Key,
    model: &'a MarkovModel<HarmonyTransitionState>,
    fallback: &'a dyn HarmonyMetricResolver,
}

impl<'a> LearnedTransitionResolver<'a> {
    /// Installs a named learned model beside a resolver for declared metrics.
    pub fn new(
        model_id: impl Into<String>,
        key: Key,
        model: &'a MarkovModel<HarmonyTransitionState>,
        fallback: &'a dyn HarmonyMetricResolver,
    ) -> Result<Self, HarmonyError> {
        let model_id = model_id.into();
        validate_id(&model_id)?;
        Ok(Self {
            model_id,
            key,
            model,
            fallback,
        })
    }
}

impl HarmonyMetricResolver for LearnedTransitionResolver<'_> {
    fn evaluate(
        &self,
        metric: &HarmonyMetric,
        context: HarmonyEvaluationContext<'_>,
    ) -> Result<HarmonyMetricObservation, HarmonyError> {
        let HarmonyMetric::LearnedTransition { model } = metric else {
            return self.fallback.evaluate(metric, context);
        };
        if model != &self.model_id {
            return Err(HarmonyError::UnknownMetricModel(model.clone()));
        }
        let [.., from, to] = context.progression else {
            return Ok(HarmonyMetricObservation {
                value: 0.0,
                facts: vec![
                    format!("model={}", self.model_id),
                    "transition=none".to_owned(),
                    provenance_fact(self.model),
                ],
            });
        };
        let from = HarmonyTransitionState::from_template(self.key, from)?;
        let to = HarmonyTransitionState::from_template(self.key, to)?;
        let probability = self
            .model
            .transition_probability(&from, &to)
            .map_err(markov_error)?;
        let count = self
            .model
            .transition_count(&from, &to)
            .map_err(markov_error)?;
        Ok(HarmonyMetricObservation {
            value: -probability.ln(),
            facts: vec![
                format!("model={}", self.model_id),
                format!("from={}", from.stable_label()),
                format!("to={}", to.stable_label()),
                format!("observed-count={count}"),
                format!("smoothed-probability={probability:.17}"),
                format!(
                    "additive-smoothing={:.17}",
                    self.model.policy().additive_smoothing
                ),
                provenance_fact(self.model),
            ],
        })
    }
}

fn state_key(state: HarmonyTransitionState) -> (u8, u8, u16) {
    (
        state.key.tonic.value(),
        mode_ordinal(state.key.mode),
        state.chord.bits(),
    )
}

fn mode_ordinal(mode: Mode) -> u8 {
    match mode {
        Mode::Major => 0,
        Mode::MinorNatural => 1,
        Mode::MinorHarmonic => 2,
        Mode::MinorMelodic => 3,
        Mode::Dorian => 4,
        Mode::Phrygian => 5,
        Mode::Lydian => 6,
        Mode::Mixolydian => 7,
        Mode::Aeolian => 8,
        Mode::Locrian => 9,
        Mode::WholeTone => 10,
        Mode::Diminished => 11,
        Mode::Chromatic => 12,
    }
}

fn provenance_fact(model: &MarkovModel<HarmonyTransitionState>) -> String {
    let corpus = &model.policy().corpus;
    format!(
        "corpus={},license={},hash={}",
        corpus.id, corpus.license, corpus.content_hash
    )
}

fn markov_error(error: impl std::fmt::Display) -> HarmonyError {
    HarmonyError::InvalidField {
        field: "learned-transition",
        reason: error.to_string(),
    }
}
