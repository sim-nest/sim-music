//! Generic integral-serial parameter tracks with independent phasing and exhaustion.

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::Debug;

use sim_lib_music_core::{Articulation, Time};
use sim_lib_serial_core::{AggregateRule, AlphabetId, SeriesTransform};
use thiserror::Error;

use crate::{ParameterAlphabet, ParameterError, ParameterSeries, ParameterValue};

/// Exhaustion policy for one parameter track when a plan outlives the source series.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Exhaustion {
    /// Wrap around the source series indefinitely.
    Cycle,
    /// Stop emitting once the source series is consumed and shorten the projection.
    Truncate,
    /// Stop emitting once the source series is consumed but retain explicit omitted plan ordinals.
    OneShot,
}

/// One emitted parameter value together with its source ordinal provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterStep<T: ParameterValue> {
    /// Zero-based position in the owning integral plan.
    pub plan_ordinal: usize,
    /// Zero-based source ordinal within the parameter series.
    pub parameter_ordinal: usize,
    /// Zero-based wrap count for cyclic reuse.
    pub cycle: usize,
    /// Typed parameter value emitted at this plan ordinal.
    pub value: T,
}

/// Projection of one parameter track against a requested plan length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterProjection<T: ParameterValue> {
    name: String,
    alphabet_id: AlphabetId,
    source_len: usize,
    phase: usize,
    exhaustion: Exhaustion,
    plan_len: usize,
    steps: Vec<ParameterStep<T>>,
    omitted_plan_ordinals: Vec<usize>,
}

impl<T: ParameterValue> ParameterProjection<T> {
    /// Returns the stable parameter name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the retained source alphabet identity.
    pub fn alphabet_id(&self) -> &AlphabetId {
        &self.alphabet_id
    }

    /// Returns the source series length before phasing or exhaustion.
    pub fn source_len(&self) -> usize {
        self.source_len
    }

    /// Returns the phase offset applied before the first emitted value.
    pub fn phase(&self) -> usize {
        self.phase
    }

    /// Returns the exhaustion policy used by this projection.
    pub const fn exhaustion(&self) -> Exhaustion {
        self.exhaustion
    }

    /// Returns the requested target plan length.
    pub fn plan_len(&self) -> usize {
        self.plan_len
    }

    /// Returns the emitted steps with their parameter ordinal ledger.
    pub fn steps(&self) -> &[ParameterStep<T>] {
        &self.steps
    }

    /// Returns plan ordinals that intentionally produced no value under one-shot exhaustion.
    pub fn omitted_plan_ordinals(&self) -> &[usize] {
        &self.omitted_plan_ordinals
    }
}

/// One generic named parameter track over an unchanged serial-core series.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterTrack<T: ParameterValue> {
    name: String,
    series: ParameterSeries<T>,
    phase: usize,
    exhaustion: Exhaustion,
}

impl<T: ParameterValue> ParameterTrack<T> {
    /// Constructs one exhaustive exactly-once track from a caller-declared value ladder.
    pub fn try_new(
        name: impl Into<String>,
        values: Vec<T>,
        exhaustion: Exhaustion,
    ) -> Result<Self, IntegralError> {
        let name = validate_parameter_name(name.into())?;
        Ok(Self {
            series: ParameterSeries::try_new(parameter_alphabet_id(&name), values)?,
            name,
            phase: 0,
            exhaustion,
        })
    }

    /// Constructs one track from a caller-declared aggregate rule.
    pub fn try_new_with_rule(
        name: impl Into<String>,
        rule: AggregateRule,
        values: Vec<T>,
        exhaustion: Exhaustion,
    ) -> Result<Self, IntegralError> {
        let name = validate_parameter_name(name.into())?;
        Ok(Self {
            series: ParameterSeries::try_new_with_rule(parameter_alphabet_id(&name), rule, values)?,
            name,
            phase: 0,
            exhaustion,
        })
    }

    /// Returns this track with an explicit phase offset.
    pub fn with_phase(mut self, phase: usize) -> Self {
        self.phase = phase;
        self
    }

    /// Returns the stable track name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the retained typed series.
    pub fn series(&self) -> &ParameterSeries<T> {
        &self.series
    }

    /// Returns the configured phase offset.
    pub fn phase(&self) -> usize {
        self.phase
    }

    /// Returns the configured exhaustion policy.
    pub const fn exhaustion(&self) -> Exhaustion {
        self.exhaustion
    }

    /// Applies a serial-core transform to this track without touching other tracks.
    pub fn transformed(
        &self,
        transform: &SeriesTransform<ParameterAlphabet<T>>,
    ) -> Result<Self, IntegralError> {
        Ok(Self {
            name: self.name.clone(),
            series: self.series.apply(transform)?,
            phase: self.phase,
            exhaustion: self.exhaustion,
        })
    }

    /// Projects this track against a requested integral-plan length.
    pub fn project(&self, plan_len: usize) -> ParameterProjection<T> {
        let source = self.series.order();
        let source_len = source.len();
        let mut steps = Vec::new();
        let mut omitted_plan_ordinals = Vec::new();

        if source_len == 0 {
            return ParameterProjection {
                name: self.name.clone(),
                alphabet_id: self.series.alphabet().id().clone(),
                source_len,
                phase: self.phase,
                exhaustion: self.exhaustion,
                plan_len,
                steps,
                omitted_plan_ordinals,
            };
        }

        let base_phase = match self.exhaustion {
            Exhaustion::Cycle => self.phase % source_len,
            Exhaustion::Truncate | Exhaustion::OneShot => self.phase,
        };
        for plan_ordinal in 0..plan_len {
            let absolute = base_phase + plan_ordinal;
            match self.exhaustion {
                Exhaustion::Cycle => {
                    let parameter_ordinal = absolute % source_len;
                    let cycle = absolute / source_len;
                    steps.push(ParameterStep {
                        plan_ordinal,
                        parameter_ordinal,
                        cycle,
                        value: source[parameter_ordinal].clone(),
                    });
                }
                Exhaustion::Truncate => {
                    if absolute >= source_len {
                        break;
                    }
                    steps.push(ParameterStep {
                        plan_ordinal,
                        parameter_ordinal: absolute,
                        cycle: 0,
                        value: source[absolute].clone(),
                    });
                }
                Exhaustion::OneShot => {
                    if absolute >= source_len {
                        omitted_plan_ordinals.push(plan_ordinal);
                        continue;
                    }
                    steps.push(ParameterStep {
                        plan_ordinal,
                        parameter_ordinal: absolute,
                        cycle: 0,
                        value: source[absolute].clone(),
                    });
                }
            }
        }

        ParameterProjection {
            name: self.name.clone(),
            alphabet_id: self.series.alphabet().id().clone(),
            source_len,
            phase: self.phase,
            exhaustion: self.exhaustion,
            plan_len,
            steps,
            omitted_plan_ordinals,
        }
    }
}

/// Source-ordinal ledger entry retained after one track is bound into a plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterOrdinalLedgerEntry {
    /// Zero-based position in the integral plan.
    pub plan_ordinal: usize,
    /// Zero-based source ordinal inside the parameter track.
    pub parameter_ordinal: usize,
    /// Zero-based wrap count for cyclic reuse.
    pub cycle: usize,
}

/// One typed bound track retained inside an [`IntegralPlan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundParameterTrack<T: ParameterValue> {
    track: ParameterTrack<T>,
    projection: ParameterProjection<T>,
}

impl<T: ParameterValue> BoundParameterTrack<T> {
    /// Returns the original typed track.
    pub fn track(&self) -> &ParameterTrack<T> {
        &self.track
    }

    /// Returns the projected values and omitted ordinals for this plan.
    pub fn projection(&self) -> &ParameterProjection<T> {
        &self.projection
    }

    /// Returns the source-ordinal ledger retained by the track binding.
    pub fn ordinal_ledger(&self) -> Vec<ParameterOrdinalLedgerEntry> {
        self.projection
            .steps()
            .iter()
            .map(|step| ParameterOrdinalLedgerEntry {
                plan_ordinal: step.plan_ordinal,
                parameter_ordinal: step.parameter_ordinal,
                cycle: step.cycle,
            })
            .collect()
    }
}

/// Type-erased view of one parameter track bound into an integral plan.
pub trait ErasedParameterBinding: Debug {
    /// Returns the stable parameter name.
    fn name(&self) -> &str;
    /// Returns the source alphabet identity retained by this binding.
    fn alphabet_id(&self) -> &AlphabetId;
    /// Returns the configured phase offset.
    fn phase(&self) -> usize;
    /// Returns the configured exhaustion policy.
    fn exhaustion(&self) -> Exhaustion;
    /// Returns the source series length.
    fn source_len(&self) -> usize;
    /// Returns the requested plan length.
    fn plan_len(&self) -> usize;
    /// Returns the retained ordinal ledger for emitted values.
    fn ordinal_ledger(&self) -> Vec<ParameterOrdinalLedgerEntry>;
    /// Returns one debug rendering per emitted value in plan order.
    fn debug_values(&self) -> Vec<String>;
    /// Returns one debug rendering for every omitted one-shot plan ordinal.
    fn omitted_plan_ordinals(&self) -> &[usize];
    /// Returns a downcast hook for typed access.
    fn as_any(&self) -> &dyn Any;
}

impl<T: ParameterValue> ErasedParameterBinding for BoundParameterTrack<T> {
    fn name(&self) -> &str {
        self.track.name()
    }

    fn alphabet_id(&self) -> &AlphabetId {
        self.projection.alphabet_id()
    }

    fn phase(&self) -> usize {
        self.track.phase()
    }

    fn exhaustion(&self) -> Exhaustion {
        self.track.exhaustion()
    }

    fn source_len(&self) -> usize {
        self.projection.source_len()
    }

    fn plan_len(&self) -> usize {
        self.projection.plan_len()
    }

    fn ordinal_ledger(&self) -> Vec<ParameterOrdinalLedgerEntry> {
        BoundParameterTrack::ordinal_ledger(self)
    }

    fn debug_values(&self) -> Vec<String> {
        self.projection
            .steps()
            .iter()
            .map(|step| format!("{:?}", step.value))
            .collect()
    }

    fn omitted_plan_ordinals(&self) -> &[usize] {
        self.projection.omitted_plan_ordinals()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Inspectable integral-serial plan with separately bound parameter tracks.
#[derive(Debug)]
pub struct IntegralPlan {
    length: usize,
    parameters: BTreeMap<String, Box<dyn ErasedParameterBinding>>,
}

impl IntegralPlan {
    /// Constructs an empty plan that projects every bound track against `length`.
    pub fn new(length: usize) -> Self {
        Self {
            length,
            parameters: BTreeMap::new(),
        }
    }

    /// Returns the target plan length used by every parameter binding.
    pub fn length(&self) -> usize {
        self.length
    }

    /// Binds one typed parameter track while preserving its independent ordinal ledger.
    pub fn bind_parameter<T: ParameterValue>(
        &mut self,
        track: ParameterTrack<T>,
    ) -> Result<(), IntegralError> {
        if self.parameters.contains_key(track.name()) {
            return Err(IntegralError::DuplicateTrack(track.name().to_owned()));
        }
        let projection = track.project(self.length);
        let name = track.name().to_owned();
        self.parameters
            .insert(name, Box::new(BoundParameterTrack { track, projection }));
        Ok(())
    }

    /// Returns one type-erased bound parameter by stable name.
    pub fn parameter(&self, name: &str) -> Option<&dyn ErasedParameterBinding> {
        self.parameters.get(name).map(Box::as_ref)
    }

    /// Returns one typed bound parameter when `name` and `T` both match.
    pub fn typed_parameter<T: ParameterValue>(
        &self,
        name: &str,
    ) -> Option<&BoundParameterTrack<T>> {
        self.parameters
            .get(name)
            .and_then(|binding| binding.as_any().downcast_ref::<BoundParameterTrack<T>>())
    }

    /// Returns the stable parameter names in sorted order.
    pub fn parameter_names(&self) -> Vec<&str> {
        self.parameters.keys().map(String::as_str).collect()
    }
}

/// Track alias for exact duration values.
pub type DurationTrack = ParameterTrack<Time>;
/// Track alias for MIDI-style dynamic values.
pub type DynamicsTrack = ParameterTrack<u8>;
/// Track alias for MIDI-style register placement.
pub type RegisterTrack = ParameterTrack<i8>;
/// Track alias for articulation values from music-core.
pub type ArticulationTrack = ParameterTrack<Articulation>;
/// Track alias for caller-declared timbre labels.
pub type TimbreTrack = ParameterTrack<String>;

/// Failure while constructing or binding integral parameter tracks.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum IntegralError {
    /// The parameter name was empty or used invalid characters.
    #[error("invalid parameter name {0:?}")]
    InvalidParameterName(String),
    /// The track tried to reuse a name already bound in the plan.
    #[error("parameter track {0} is already bound")]
    DuplicateTrack(String),
    /// Constructing or transforming the generic parameter series failed.
    #[error(transparent)]
    Parameter(#[from] ParameterError),
}

fn validate_parameter_name(name: String) -> Result<String, IntegralError> {
    if name.trim().is_empty() {
        return Err(IntegralError::InvalidParameterName(name));
    }
    if name
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.')))
    {
        return Err(IntegralError::InvalidParameterName(name));
    }
    Ok(name)
}

fn parameter_alphabet_id(name: &str) -> String {
    format!("parameter/{name}-v1")
}
