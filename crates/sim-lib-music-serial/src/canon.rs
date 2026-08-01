//! Canon construction with explicit symmetry and realization metadata.

use std::collections::BTreeMap;

use sim_lib_music_core::{Articulation, Channel, Time};
use sim_lib_pitch_serial::RowForm;
use thiserror::Error;

use crate::{
    EventPlacement, PlannedSerialEvent, RowInstanceId, SerialEventId, SerialOrigin, SerialPlan,
    SerialRole, StrictEventSpec, StructuralLicense, VoiceId,
};

/// Symmetry policy required of one canon.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CanonSymmetryRequirement {
    /// No additional symmetry check.
    None,
    /// Later voices must present the retrograde of the first voice's row classes.
    RetrogradeAnswer,
    /// Voice onsets must mirror around the outer edges of the canon.
    PalindromicVoiceOffsets,
}

/// Voice-level realization parameters that do not create new row events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonOrchestration {
    /// MIDI channel for the realized voice.
    pub channel: Channel,
    /// Articulation for the realized voice.
    pub articulation: Articulation,
    /// Optional timbral label retained outside the row plan.
    pub timbre: Option<String>,
    /// Optional orchestration role retained outside the row plan.
    pub orchestration: Option<String>,
}

/// One canonical voice specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonVoiceSpec {
    /// Row instance identity for this voice.
    pub row_id: RowInstanceId,
    /// Row form presented by this voice.
    pub form: RowForm,
    /// Stable voice identity.
    pub voice: VoiceId,
    /// Voice offset relative to the canon onset.
    pub voice_offset: Time,
    /// MIDI-style octave register.
    pub register: i8,
    /// Duration per row ordinal.
    pub duration: Time,
    /// Realization parameters retained outside the structural plan.
    pub orchestration: CanonOrchestration,
}

/// One planned canon request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonSpec {
    /// Stable event-id prefix.
    pub event_prefix: String,
    /// Absolute canon onset.
    pub onset: Time,
    /// Structural rationale shared by all voices.
    pub rationale: String,
    /// Structural reading that licenses the canon.
    pub license: StructuralLicense,
    /// Required symmetry.
    pub requirement: CanonSymmetryRequirement,
    /// Voices participating in the canon.
    pub voices: Vec<CanonVoiceSpec>,
}

/// One realized canon event specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonRealizationEvent {
    /// Planned event identity.
    pub event_id: SerialEventId,
    /// Exact onset assigned by the canon builder.
    pub onset: Time,
    /// Realization spec excluding onset.
    pub spec: StrictEventSpec,
}

/// One canon voice profile kept outside the row-event graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonVoiceProfile {
    /// Stable voice identity.
    pub voice: VoiceId,
    /// Row form assigned to the voice.
    pub form: RowForm,
    /// Voice offset relative to the canon onset.
    pub voice_offset: Time,
    /// Realization-only orchestration parameters.
    pub orchestration: CanonOrchestration,
}

/// Symmetry evidence attached to one built canon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonSymmetryCertificate {
    /// Requirement that was evaluated.
    pub requirement: CanonSymmetryRequirement,
    /// Whether the requirement held.
    pub satisfied: bool,
    /// Human-readable explanation of the result.
    pub explanation: String,
}

/// Complete canon output with immutable row events plus realization metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonDeployment {
    /// Structural plan containing only row events.
    pub plan: SerialPlan,
    /// Event-level onset/register/duration realization parameters.
    pub realization: Vec<CanonRealizationEvent>,
    /// Voice-level timbre and orchestration metadata.
    pub voices: Vec<CanonVoiceProfile>,
    /// Symmetry evidence for the canon.
    pub symmetry: CanonSymmetryCertificate,
}

/// Failure while constructing one canon.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CanonError {
    /// The request omitted every voice.
    #[error("canon requires at least one voice")]
    EmptyVoices,
    /// One voice named a non-positive duration.
    #[error("canon voice {0} must use a strictly positive duration")]
    NonPositiveDuration(VoiceId),
    /// The requested symmetry requirement was not satisfied.
    #[error("{0}")]
    Symmetry(String),
    /// Building the immutable serial plan failed.
    #[error("canon plan failed: {0}")]
    Plan(String),
}

/// Builds one canon with explicit symmetry and realization metadata.
pub fn build_canon(spec: CanonSpec) -> Result<CanonDeployment, CanonError> {
    if spec.voices.is_empty() {
        return Err(CanonError::EmptyVoices);
    }
    for voice in &spec.voices {
        if voice.duration <= Time::from_integer(0) {
            return Err(CanonError::NonPositiveDuration(voice.voice.clone()));
        }
    }

    let symmetry = validate_symmetry(&spec)?;
    if !symmetry.satisfied {
        return Err(CanonError::Symmetry(symmetry.explanation.clone()));
    }

    let mut rows = BTreeMap::new();
    let mut events = BTreeMap::new();
    let mut precedence = Vec::new();
    let mut realization = Vec::new();
    let mut voices = Vec::with_capacity(spec.voices.len());

    for voice_spec in &spec.voices {
        rows.insert(voice_spec.row_id.clone(), voice_spec.form.clone());
        voices.push(CanonVoiceProfile {
            voice: voice_spec.voice.clone(),
            form: voice_spec.form.clone(),
            voice_offset: voice_spec.voice_offset,
            orchestration: voice_spec.orchestration.clone(),
        });
        let mut previous = None::<SerialEventId>;
        for ordinal in 0..12usize {
            let event_id = SerialEventId::new(format!("{}/{}", spec.event_prefix, events.len()))
                .map_err(|error| CanonError::Plan(error.to_string()))?;
            let event = PlannedSerialEvent {
                id: event_id.clone(),
                ordinals: vec![crate::OrdinalRef::new(voice_spec.row_id.clone(), ordinal)],
                role: SerialRole::Structural,
                origin: SerialOrigin::Structural {
                    rationale: spec.rationale.clone(),
                },
                voice: voice_spec.voice.clone(),
                placement: EventPlacement::independent(),
                parents: Vec::new(),
                licenses: vec![spec.license.clone()],
            };
            events.insert(event_id.clone(), event);
            if let Some(previous_id) = previous.as_ref() {
                precedence.push((previous_id.clone(), event_id.clone()));
            }
            previous = Some(event_id.clone());
            realization.push(CanonRealizationEvent {
                event_id,
                onset: spec.onset
                    + voice_spec.voice_offset
                    + (voice_spec.duration * i64::try_from(ordinal).expect("ordinal fits i64")),
                spec: StrictEventSpec::notes(
                    voice_spec.register,
                    voice_spec.duration,
                    88,
                    voice_spec.orchestration.channel,
                    voice_spec.orchestration.articulation,
                ),
            });
        }
    }

    let plan = SerialPlan::try_new(rows, events, precedence)
        .map_err(|error| CanonError::Plan(error.to_string()))?;
    Ok(CanonDeployment {
        plan,
        realization,
        voices,
        symmetry,
    })
}

fn validate_symmetry(spec: &CanonSpec) -> Result<CanonSymmetryCertificate, CanonError> {
    let certificate = match spec.requirement {
        CanonSymmetryRequirement::None => CanonSymmetryCertificate {
            requirement: CanonSymmetryRequirement::None,
            satisfied: true,
            explanation: "no symmetry requirement requested".to_owned(),
        },
        CanonSymmetryRequirement::RetrogradeAnswer => {
            let Some(subject) = spec.voices.first() else {
                return Err(CanonError::EmptyVoices);
            };
            let satisfied = spec.voices.iter().skip(1).all(|voice| {
                voice.form.classes().iter().copied().eq(subject
                    .form
                    .classes()
                    .iter()
                    .rev()
                    .copied())
            });
            CanonSymmetryCertificate {
                requirement: CanonSymmetryRequirement::RetrogradeAnswer,
                satisfied,
                explanation: if satisfied {
                    "every answer voice preserves the first voice as an exact retrograde".to_owned()
                } else {
                    "retrograde-answer requirement failed: at least one answer voice is not the subject retrograde".to_owned()
                },
            }
        }
        CanonSymmetryRequirement::PalindromicVoiceOffsets => {
            let offsets = spec
                .voices
                .iter()
                .map(|voice| voice.voice_offset)
                .collect::<Vec<_>>();
            let satisfied = offsets.iter().eq(offsets.iter().rev());
            CanonSymmetryCertificate {
                requirement: CanonSymmetryRequirement::PalindromicVoiceOffsets,
                satisfied,
                explanation: if satisfied {
                    "voice offsets form a palindrome".to_owned()
                } else {
                    "palindromic-voice-offset requirement failed".to_owned()
                },
            }
        }
    };
    Ok(certificate)
}
