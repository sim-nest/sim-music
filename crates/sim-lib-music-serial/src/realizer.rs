//! Open serial realizer identities, context, and registry-facing traits.

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use sim_lib_music_core::{Articulation, Channel, Time};
use sim_lib_pitch_scale::Scale;
use sim_lib_sound_tuning::Tuning;

use crate::{ErasedParameterBinding, SerialEventId, SerialPlan, SerialRealization, VoiceId};

fn validate_id(kind: &'static str, value: impl Into<String>) -> Result<String, String> {
    let value = value.into();
    if value.trim().is_empty() {
        return Err(format!("{kind} cannot be empty"));
    }
    if value
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.')))
    {
        return Err(format!(
            "{kind} must use ASCII letters, digits, /, -, _, or ."
        ));
    }
    Ok(value)
}

/// Stable identity for one registered serial realizer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RealizerId(String);

impl RealizerId {
    /// Creates a validated stable identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        Ok(Self(validate_id("realizer-id", value)?))
    }

    /// Returns the stable wire text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for RealizerId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// How one planned event should sound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventSound {
    /// Sound one note per ordinal.
    Notes,
    /// Occupy time silently.
    Rest,
}

/// How octave placement is derived for one event's ordinals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrictPitchLayout {
    /// MIDI-style octave register, where `4` means the `C4..B4` octave.
    pub register: i8,
    /// Per-ordinal octave displacements relative to `register`.
    pub octave_displacements: Vec<i8>,
}

impl StrictPitchLayout {
    /// Places every ordinal in the same octave register.
    pub fn in_register(register: i8) -> Self {
        Self {
            register,
            octave_displacements: Vec::new(),
        }
    }
}

/// Explicit tie behavior for one event.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TiePolicy {
    /// Re-articulate normally.
    None,
    /// Sustain the same pitches into the next same-voice event and suppress its attack.
    IntoNext,
}

/// How explicit simultaneous groups should be rendered.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SimultaneousRenderPolicy {
    /// Keep every simultaneous group at one exact onset and advance by the longest member.
    PreserveOnset,
}

/// Explicit realization choices for one planned event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrictEventSpec {
    /// Sounding or silent behavior.
    pub sound: EventSound,
    /// Pitch placement policy.
    pub pitch_layout: StrictPitchLayout,
    /// Exact occupied duration.
    pub duration: Time,
    /// MIDI velocity.
    pub velocity: u8,
    /// MIDI channel.
    pub channel: Channel,
    /// Articulation to apply when the event sounds.
    pub articulation: Articulation,
    /// Explicit tie behavior.
    pub tie: TiePolicy,
}

impl StrictEventSpec {
    /// Constructs a sounding note spec with explicit pitch/timing/dynamic choices.
    pub fn notes(
        register: i8,
        duration: Time,
        velocity: u8,
        channel: Channel,
        articulation: Articulation,
    ) -> Self {
        Self {
            sound: EventSound::Notes,
            pitch_layout: StrictPitchLayout::in_register(register),
            duration,
            velocity,
            channel,
            articulation,
            tie: TiePolicy::None,
        }
    }

    /// Constructs a silent span with explicit timing.
    pub fn rest(duration: Time) -> Self {
        Self {
            sound: EventSound::Rest,
            pitch_layout: StrictPitchLayout::in_register(4),
            duration,
            velocity: 0,
            channel: Channel::new(0).expect("MIDI channel zero is valid"),
            articulation: Articulation::Normal,
            tie: TiePolicy::None,
        }
    }
}

/// Inclusive register policy for one voice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterBounds {
    /// Lowest allowed octave register.
    pub lowest: i8,
    /// Highest allowed octave register.
    pub highest: i8,
}

/// Generic voice-level realization policy retained in open context data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceBounds {
    /// Maximum simultaneous note count allowed for the voice, if any.
    pub max_notes_per_event: Option<usize>,
}

/// One type-erased auxiliary service available to realizers.
pub trait RealizationService: Any + Send + Sync {
    /// Returns a downcast hook for typed access.
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + Send + Sync> RealizationService for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open service bag passed through realization context data.
#[derive(Clone, Default)]
pub struct RealizationServices {
    entries: BTreeMap<String, Arc<dyn RealizationService>>,
}

impl RealizationServices {
    /// Creates an empty service bag.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or replaces one named service.
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        service: Arc<dyn RealizationService>,
    ) -> Option<Arc<dyn RealizationService>> {
        self.entries.insert(name.into(), service)
    }

    /// Returns one typed service by stable name.
    pub fn get<T: Any + Send + Sync>(&self, name: &str) -> Option<&T> {
        self.entries
            .get(name)
            .and_then(|service| service.as_ref().as_any().downcast_ref::<T>())
    }

    /// Returns the stable service names in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().map(String::as_str).collect()
    }
}

impl std::fmt::Debug for RealizationServices {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RealizationServices")
            .field("names", &self.names())
            .finish()
    }
}

impl PartialEq for RealizationServices {
    fn eq(&self, other: &Self) -> bool {
        self.names() == other.names()
    }
}

impl Eq for RealizationServices {}

/// Complete explicit choices and open services required to realize one serial plan.
#[derive(Clone)]
pub struct RealizationContext {
    /// One explicit strict spec per planned event.
    pub specs: BTreeMap<SerialEventId, StrictEventSpec>,
    /// Policy for explicit simultaneous groups.
    pub simultaneous_policy: SimultaneousRenderPolicy,
    /// Optional target scale retained for realizers that need scale awareness.
    pub scale: Option<Scale>,
    /// Optional target tuning retained for pitch/frequency-aware realizers.
    pub tuning: Option<Arc<dyn Tuning>>,
    /// Optional per-voice register policy.
    pub register_bounds: BTreeMap<VoiceId, RegisterBounds>,
    /// Optional per-voice realization limits.
    pub voice_bounds: BTreeMap<VoiceId, VoiceBounds>,
    /// Optional typed parameter tracks exposed by stable name.
    pub parameter_tracks: BTreeMap<String, Arc<dyn ErasedParameterBinding>>,
    /// Open auxiliary services keyed by stable name.
    pub services: RealizationServices,
}

impl RealizationContext {
    /// Builds a context from explicit specs using the default simultaneous policy.
    pub fn new(specs: BTreeMap<SerialEventId, StrictEventSpec>) -> Self {
        Self {
            specs,
            simultaneous_policy: SimultaneousRenderPolicy::PreserveOnset,
            scale: None,
            tuning: None,
            register_bounds: BTreeMap::new(),
            voice_bounds: BTreeMap::new(),
            parameter_tracks: BTreeMap::new(),
            services: RealizationServices::default(),
        }
    }
}

impl std::fmt::Debug for RealizationContext {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RealizationContext")
            .field("specs", &self.specs)
            .field("simultaneous_policy", &self.simultaneous_policy)
            .field("scale", &self.scale)
            .field("tuning", &self.tuning.as_ref().map(|tuning| tuning.name()))
            .field("register_bounds", &self.register_bounds)
            .field("voice_bounds", &self.voice_bounds)
            .field(
                "parameter_tracks",
                &self.parameter_tracks.keys().collect::<Vec<_>>(),
            )
            .field("services", &self.services)
            .finish()
    }
}

/// Backwards-compatible alias for the former strict-only context name.
pub type StrictRealizationContext = RealizationContext;

/// Open serial realizer component registered by stable id.
pub trait SerialRealizer: Send + Sync {
    /// Returns the stable realizer identity.
    fn id(&self) -> &RealizerId;

    /// Realizes one immutable serial plan using the supplied context.
    fn realize(
        &self,
        plan: &SerialPlan,
        context: &RealizationContext,
    ) -> Result<SerialRealization, crate::StrictRealizationError>;
}
