use sim_kernel::Symbol;

use crate::MusicError;

/// Stable string identifier for a lane.
///
/// Wraps the raw lane name used to group and order [`PlayEvent`](crate::PlayEvent)s.
///
/// # Examples
///
/// ```
/// use sim_lib_music_core::LaneId;
///
/// let id = LaneId::new("bass");
/// assert_eq!(id.as_ref(), "bass");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LaneId(
    /// The raw lane name.
    pub String,
);

impl LaneId {
    /// Builds a lane id from any string-like value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for LaneId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Category of content carried by a lane.
///
/// Each variant maps to a [`PlayEvent`](crate::PlayEvent) family and constrains
/// which [`LaneTarget`] a lane may bind to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LaneKind {
    /// Pitched note events.
    Note,
    /// Percussion / drum-hit events.
    Drum,
    /// Scale-degree events resolved against a scale.
    ScaleDegree,
    /// Raw MIDI events.
    Midi,
    /// Bare pitch events without duration or velocity.
    Pitch,
    /// Discrete control-change events.
    Control,
    /// Continuous automation events.
    Automation,
    /// Audio-frame events.
    Audio,
    /// Object-valued events.
    Object,
    /// Playable-reference events.
    Playable,
    /// Performance-intent events.
    Performance,
    /// Diagnostic message events.
    Diagnostic,
    /// Trace / debugging step events.
    Trace,
}

impl LaneKind {
    /// Returns the qualified `music/lane-kind` symbol for this kind.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("music/lane-kind", self.wire_label())
    }

    /// Returns the stable wire label used for serialization.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_core::LaneKind;
    ///
    /// assert_eq!(LaneKind::ScaleDegree.wire_label(), "scale-degree");
    /// ```
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Drum => "drum",
            Self::ScaleDegree => "scale-degree",
            Self::Midi => "midi",
            Self::Pitch => "pitch",
            Self::Control => "control",
            Self::Automation => "automation",
            Self::Audio => "audio",
            Self::Object => "object",
            Self::Playable => "playable",
            Self::Performance => "performance",
            Self::Diagnostic => "diagnostic",
            Self::Trace => "trace",
        }
    }
}

/// Destination a lane routes its events to.
///
/// The target restricts which [`LaneKind`]s may bind to a lane via
/// [`LaneDescriptor::new`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaneTarget {
    /// Routes to a named instrument.
    Instrument(Symbol),
    /// Routes to a named stream.
    Stream(Symbol),
    /// Routes to a named control surface.
    Control(Symbol),
    /// Has no routing target.
    None,
}

impl LaneTarget {
    /// Returns the symbol naming this target, or a sentinel for [`LaneTarget::None`].
    pub fn symbol(&self) -> Symbol {
        match self {
            Self::Instrument(symbol) | Self::Stream(symbol) | Self::Control(symbol) => {
                symbol.clone()
            }
            Self::None => Symbol::qualified("music/lane-target", "none"),
        }
    }

    fn accepts(&self, kind: LaneKind) -> bool {
        match kind {
            LaneKind::Note
            | LaneKind::Drum
            | LaneKind::ScaleDegree
            | LaneKind::Midi
            | LaneKind::Pitch
            | LaneKind::Object
            | LaneKind::Performance => {
                matches!(self, Self::Instrument(_) | Self::Stream(_))
            }
            LaneKind::Control | LaneKind::Automation => matches!(
                self,
                Self::Control(_) | Self::Instrument(_) | Self::Stream(_)
            ),
            LaneKind::Audio | LaneKind::Playable => matches!(self, Self::Stream(_)),
            LaneKind::Diagnostic | LaneKind::Trace => matches!(self, Self::None | Self::Stream(_)),
        }
    }
}

/// Describes a single lane: its identity, content kind, target, and order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneDescriptor {
    /// Stable identifier of the lane.
    pub id: LaneId,
    /// Category of content the lane carries.
    pub kind: LaneKind,
    /// Destination the lane routes events to.
    pub target: LaneTarget,
    /// Sort order of the lane relative to its peers.
    pub order: u32,
}

impl LaneDescriptor {
    /// Builds a descriptor, validating that `target` accepts `kind`.
    ///
    /// Returns `MusicError::InvalidLaneTarget` when the target cannot carry the
    /// given kind.
    pub fn new(
        id: LaneId,
        kind: LaneKind,
        target: LaneTarget,
        order: u32,
    ) -> Result<Self, MusicError> {
        if !target.accepts(kind) {
            return Err(MusicError::InvalidLaneTarget {
                lane: id.0,
                target: target.symbol().to_string(),
            });
        }
        Ok(Self {
            id,
            kind,
            target,
            order,
        })
    }
}

/// Sorts lanes deterministically by order, then id, then kind.
pub fn stable_lane_order(mut lanes: Vec<LaneDescriptor>) -> Vec<LaneDescriptor> {
    lanes.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    lanes
}
