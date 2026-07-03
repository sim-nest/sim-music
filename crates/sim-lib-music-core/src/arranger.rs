use sim_kernel::{Error, Result, Symbol};

use crate::{
    LaneDescriptor, LaneId, LaneKind, LaneTarget, Music, MusicError, Pitch, PitchClass, PlayEvent,
    Time,
};

/// Arrangement of playable material laid out across lanes on a timeline.
///
/// Holds the ordered set of [ArrangerPlacement]s plus the lane ids the
/// arrangement declares, and renders them into play events.
#[derive(Clone, Debug)]
pub struct Arranger {
    /// Placements that make up the arrangement, in declaration order.
    pub placements: Vec<ArrangerPlacement>,
    /// Lane ids the arrangement declares.
    pub lanes: Vec<LaneId>,
}

/// Single placement of playable material at a point on the arranger timeline.
///
/// Carries the source reference plus the per-placement transforms, stretch,
/// pitch remap, filter, and trace policy applied during rendering.
#[derive(Clone, Debug)]
pub struct ArrangerPlacement {
    /// Stable identifier used to attribute diagnostics and traces.
    pub id: Symbol,
    /// Playable material this placement renders.
    pub playable: PlayableRef,
    /// Onset of the placement on the arranger timeline.
    pub at: Time,
    /// Optional explicit duration used for clipping and fit-to-duration stretch.
    pub duration: Option<Time>,
    /// Lane the rendered notes are assigned to.
    pub lane: LaneId,
    /// Targets the lane drives.
    pub targets: Vec<LaneTarget>,
    /// Time-scaling policy applied to the placement.
    pub stretch: StretchPolicy,
    /// Ordered list of pitch and time transforms applied to the placement.
    pub transform: Vec<PlacementTransform>,
    /// Pitch remapping applied after transforms.
    pub remap_pitch: PitchRemap,
    /// Optional lane filter applied after pitch remapping.
    pub filter: Option<FilterRef>,
    /// Optional seed for deterministic placement behavior.
    pub seed: Option<u64>,
    /// Tracing verbosity for the placement.
    pub trace: TracePolicy,
}

/// Reference to the playable material a placement renders.
#[derive(Clone, Debug)]
pub enum PlayableRef {
    /// Inline music object owned by the placement.
    Inline(Box<Music>),
    /// Named reference resolved by the host at render time.
    Symbol(Symbol),
}

/// Pitch or time transform applied to a placement's notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementTransform {
    /// Transposes every pitch by a number of semitones.
    TransposeSemitones(i32),
    /// Transposes every pitch by a number of octaves.
    TransposeOctaves(i16),
    /// Inverts every pitch around a fixed pitch axis.
    InvertAroundPitch(Pitch),
    /// Inverts every pitch class around a fixed pitch-class axis.
    InvertAroundPitchClass(PitchClass),
    /// Reverses the placement in time.
    Retrograde,
}

/// Time-scaling policy applied to a placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StretchPolicy {
    /// Leaves timing unchanged.
    None,
    /// Scales timing by the reciprocal of the given tempo ratio.
    TempoRatio(Time),
    /// Scales timing directly by the given time ratio.
    TimeRatio(Time),
    /// Scales the placement to fill its declared duration.
    FitToDuration,
}

/// Pitch remapping applied after a placement's transforms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PitchRemap {
    /// Leaves pitches unchanged.
    None,
    /// Shifts every pitch by a fixed number of semitones.
    Chromatic(i32),
    /// Replaces one pitch class with another.
    PitchClass {
        /// Pitch class to match.
        from: PitchClass,
        /// Pitch class to substitute.
        to: PitchClass,
    },
    /// Maps source MIDI keys to target keys for drum lanes.
    DrumKey(Vec<(u8, u8)>),
    /// Scale-degree remap resolved by a host-provided resolver.
    ScaleDegree(Symbol),
    /// Chord-tone remap resolved by a host-provided resolver.
    ChordTone(Symbol),
    /// Tuning remap resolved by a host-provided resolver.
    Tuning(Symbol),
    /// Vector remap resolved by a host-provided resolver.
    Vector(Symbol),
    /// Matrix remap resolved by a host-provided resolver.
    Matrix(Symbol),
    /// Callable remap resolved by a host-provided resolver.
    Callable(Symbol),
}

/// Lane filter that keeps a placement only when its lane is retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterRef {
    /// Identifier reported in filter diagnostics.
    pub id: Symbol,
    /// Lanes the filter keeps; an empty list acts as identity.
    pub keep_lanes: Vec<LaneId>,
}

/// Tracing verbosity for a placement during rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TracePolicy {
    /// Emits no trace output.
    Off,
    /// Emits diagnostic-level trace output.
    Diagnostics,
    /// Emits full trace events.
    Full,
}

/// Result of rendering an [Arranger]: play events plus diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrangerRender {
    /// Stable-ordered play events produced by the arrangement.
    pub events: Vec<PlayEvent>,
    /// Diagnostics collected while rendering.
    pub diagnostics: Vec<ArrangerDiagnostic>,
}

/// Diagnostic raised while rendering a placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrangerDiagnostic {
    /// Identifier of the placement that produced the diagnostic.
    pub placement_id: Symbol,
    /// Time on the arranger timeline the diagnostic refers to.
    pub at: Time,
    /// Human-readable diagnostic message.
    pub message: String,
}

impl Arranger {
    /// Builds an arranger from placements and lane ids, validating each placement.
    pub fn new(placements: Vec<ArrangerPlacement>, lanes: Vec<LaneId>) -> Result<Self> {
        let arranger = Self { placements, lanes };
        arranger.validate()?;
        Ok(arranger)
    }

    fn validate(&self) -> Result<()> {
        for placement in &self.placements {
            placement.validate()?;
        }
        Ok(())
    }
}

impl ArrangerPlacement {
    /// Builds a placement from a string id, playable, and onset using defaults.
    pub fn new(id: impl Into<String>, playable: PlayableRef, at: Time) -> Result<Self> {
        Self::with_symbol_id(Symbol::new(id.into()), playable, at)
    }

    /// Builds a placement from a symbol id, playable, and onset using defaults.
    pub fn with_symbol_id(id: Symbol, playable: PlayableRef, at: Time) -> Result<Self> {
        let placement = Self {
            id,
            playable,
            at,
            duration: None,
            lane: LaneId::new("notes"),
            targets: vec![LaneTarget::Instrument(Symbol::qualified(
                "music/target",
                "default",
            ))],
            stretch: StretchPolicy::None,
            transform: Vec::new(),
            remap_pitch: PitchRemap::None,
            filter: None,
            seed: None,
            trace: TracePolicy::Off,
        };
        placement.validate()?;
        Ok(placement)
    }

    /// Sets the placement's explicit duration, rejecting negative values.
    pub fn with_duration(mut self, duration: Time) -> Result<Self> {
        ensure_non_negative_kernel(duration, "arranger placement duration")?;
        self.duration = Some(duration);
        Ok(self)
    }

    /// Sets the lane the placement's notes are assigned to.
    pub fn with_lane(mut self, lane: LaneId) -> Self {
        self.lane = lane;
        self
    }

    /// Replaces the placement's targets with a single target.
    pub fn with_target(mut self, target: LaneTarget) -> Self {
        self.targets = vec![target];
        self
    }

    /// Replaces the placement's targets with the given list.
    pub fn with_targets(mut self, targets: Vec<LaneTarget>) -> Self {
        self.targets = targets;
        self
    }

    /// Sets the placement's stretch policy.
    pub fn with_stretch(mut self, stretch: StretchPolicy) -> Self {
        self.stretch = stretch;
        self
    }

    /// Replaces the placement's transform list.
    pub fn with_transform(mut self, transform: Vec<PlacementTransform>) -> Self {
        self.transform = transform;
        self
    }

    /// Sets the placement's pitch remap.
    pub fn with_pitch_remap(mut self, remap_pitch: PitchRemap) -> Self {
        self.remap_pitch = remap_pitch;
        self
    }

    /// Sets the placement's lane filter.
    pub fn with_filter(mut self, filter: FilterRef) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Sets the placement's deterministic seed.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Sets the placement's trace policy.
    pub fn with_trace(mut self, trace: TracePolicy) -> Self {
        self.trace = trace;
        self
    }

    pub(crate) fn validate(&self) -> Result<()> {
        ensure_non_negative_kernel(self.at, "arranger placement onset")?;
        if let Some(duration) = self.duration {
            ensure_non_negative_kernel(duration, "arranger placement duration")?;
        }
        if self.targets.is_empty() {
            return Err(Error::Eval(
                "arranger placement must have at least one target".to_owned(),
            ));
        }
        for target in &self.targets {
            LaneDescriptor::new(self.lane.clone(), LaneKind::Note, target.clone(), 0)
                .map_err(music_err)?;
        }
        Ok(())
    }
}

impl PlayableRef {
    /// Builds an inline reference that owns the given music object.
    pub fn inline(music: Music) -> Self {
        Self::Inline(Box::new(music))
    }

    /// Builds a symbolic reference resolved by the host at render time.
    pub fn symbol(symbol: Symbol) -> Self {
        Self::Symbol(symbol)
    }
}

impl FilterRef {
    /// Builds a filter from an identifier and the lanes it keeps.
    pub fn new(id: Symbol, keep_lanes: Vec<LaneId>) -> Self {
        Self { id, keep_lanes }
    }
}

pub(crate) fn ensure_non_negative_kernel(value: Time, context: &str) -> Result<()> {
    if value < Time::from_integer(0) {
        Err(Error::Eval(format!("{context} cannot be negative")))
    } else {
        Ok(())
    }
}

pub(crate) fn music_err(err: MusicError) -> Error {
    Error::Eval(err.to_string())
}
