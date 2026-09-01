//! Provenance and route history for scientific and artistic listening sessions.

// conformance: listening tests prove stable identity, typed mappings, and route substitutions.

use std::str::FromStr;

use sim_kernel::{Error, Result};

/// One measured scalar series before it is mapped to sound.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredSeries {
    /// Stable content key supplied by the measurement owner.
    pub content_key: String,
    /// Physical or observational quantity represented by each sample.
    pub quantity_kind: String,
    /// Unit attached to the source values.
    pub unit: String,
    /// Ordered measured values.
    pub values: Vec<f64>,
}

/// Normalization applied before a measured series drives sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Normalization {
    /// Preserve source values without rescaling.
    None,
    /// Map the declared source interval onto `[0, 1]`.
    Linear {
        /// Lower endpoint of the measured source interval.
        source_min: f64,
        /// Upper endpoint of the measured source interval.
        source_max: f64,
    },
}

/// Policy for values outside the normalized target interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clipping {
    /// Reject values outside the interval.
    Reject,
    /// Clamp values and record that the mapping is lossy.
    Clamp,
}

/// Interpolation used between measured samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    /// Hold each sample until the next sample arrives.
    Step,
    /// Linearly interpolate adjacent samples.
    Linear,
}

/// Whether and how the source values can be recovered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InverseMapping {
    /// The named inverse recovers the measured values within the stated tolerance.
    Available {
        /// Stable name of the inverse operation.
        operation: String,
        /// Declared recovery tolerance and unit.
        tolerance: String,
    },
    /// No inverse exists; the reason is durable evidence rather than an implication.
    Lossy {
        /// Evidence explaining why the mapping cannot be inverted.
        reason: String,
    },
}

/// Audification receipt tying a sounding result to measured evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct AudificationMapping {
    /// Measured input and its scientific identity.
    pub input: MeasuredSeries,
    /// Quantity produced by the mapping, such as frequency or amplitude.
    pub output_quantity_kind: String,
    /// Unit of the mapped output.
    pub output_unit: String,
    /// Normalization applied to the input.
    pub normalization: Normalization,
    /// Out-of-range policy.
    pub clipping: Clipping,
    /// Between-sample interpolation.
    pub interpolation: Interpolation,
    /// Exact inverse or explicit loss evidence.
    pub inverse: InverseMapping,
}

/// Authored sonification which must never be presented as a physical result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtisticMapping {
    /// Stable input content keys used by the work.
    pub input_content_keys: Vec<String>,
    /// Stable keys of the authored choices applied to those inputs.
    pub authored_choice_keys: Vec<String>,
}

impl ArtisticMapping {
    /// The unambiguous category label persisted with every artistic mapping.
    pub const KIND: &'static str = "artistic-mapping";
}

/// Durable content of a listening session, independent of any playback route.
#[derive(Clone, Debug, PartialEq)]
pub struct ListeningContent {
    /// Score content key.
    pub score: String,
    /// Audio graph content key.
    pub graph: String,
    /// Instrument patch content key.
    pub patch: String,
    /// Tuning content key.
    pub tuning: String,
    /// Scientific mapping receipt, when this is audification.
    pub audification: Option<AudificationMapping>,
    /// Creative mapping receipt, when this is authored sonification.
    pub artistic_mapping: Option<ArtisticMapping>,
    /// Additional authored-choice content keys.
    pub choices: Vec<String>,
}

/// A playback target substitution known to the music owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListeningTarget {
    /// Deterministic render without a live device.
    OfflineRender,
    /// Media-edge execution host.
    MediaEdgeHost,
    /// Heavy DAW execution host.
    HeavyDawHost,
    /// Yamaha RX-V777 receiver.
    RxV777,
    /// Korg OASYS workstation.
    Oasys,
    /// Exact refusal retained when no adapter exists.
    Unsupported {
        /// Requested target that could not be resolved.
        requested: String,
        /// Durable refusal evidence.
        evidence: String,
    },
}

/// Append-only route lifecycle event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteEventKind {
    /// A route opened successfully.
    Open,
    /// A route closed.
    Close,
    /// A route failed with the recorded detail.
    Fail,
    /// A previously failed route reconnected.
    Reconnect,
    /// Playback substituted another target.
    Fallback,
}

/// One durable route event. Live handles are deliberately absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteEvent {
    /// Monotonic sequence within this session.
    pub sequence: u64,
    /// Route lifecycle transition.
    pub kind: RouteEventKind,
    /// Target involved in the transition.
    pub target: ListeningTarget,
    /// Exact diagnostic, adapter name, or substitution reason.
    pub detail: String,
}

/// Route-independent listening session with append-only delivery history.
#[derive(Clone, Debug, PartialEq)]
pub struct ListeningSession {
    /// Stable musical identity selected by the author or expedition.
    pub musical_id: String,
    /// Stable expedition identity, when scientific work owns the observation.
    pub expedition_id: Option<String>,
    /// Durable musical and mapping content.
    pub content: ListeningContent,
    route_events: Vec<RouteEvent>,
    live_handle: Option<String>,
}

impl ListeningSession {
    /// Creates a route-independent session.
    pub fn new(
        musical_id: impl Into<String>,
        expedition_id: Option<String>,
        content: ListeningContent,
    ) -> Result<Self> {
        let musical_id = non_empty(musical_id.into(), "musical id")?;
        if content.audification.is_some() == content.artistic_mapping.is_some() {
            return Err(Error::Eval(
                "listening content must contain exactly one mapping family".to_owned(),
            ));
        }
        Ok(Self {
            musical_id,
            expedition_id,
            content,
            route_events: Vec::new(),
            live_handle: None,
        })
    }

    /// Appends a route event, assigning the next monotonic sequence number.
    pub fn append_route_event(
        &mut self,
        kind: RouteEventKind,
        target: ListeningTarget,
        detail: impl Into<String>,
    ) -> Result<()> {
        let sequence = u64::try_from(self.route_events.len())
            .map_err(|_| Error::Eval("route event sequence overflow".to_owned()))?;
        self.route_events.push(RouteEvent {
            sequence,
            kind,
            target,
            detail: non_empty(detail.into(), "route event detail")?,
        });
        Ok(())
    }

    /// Replaces the ephemeral live handle without changing durable content.
    pub fn set_live_handle(&mut self, handle: Option<String>) {
        self.live_handle = handle;
    }

    /// Returns the append-only route history.
    pub fn route_events(&self) -> &[RouteEvent] {
        &self.route_events
    }

    /// Returns the current ephemeral route handle.
    pub fn live_handle(&self) -> Option<&str> {
        self.live_handle.as_deref()
    }

    /// Returns the route-independent identity projection.
    pub fn identity(&self) -> (&str, Option<&str>, &ListeningContent) {
        (
            &self.musical_id,
            self.expedition_id.as_deref(),
            &self.content,
        )
    }
}

/// Predicate accepted only by scientific audification listeners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScientificListeningPredicate {
    /// Require a reversible mapping.
    Reversible,
    /// Accept a mapping only when loss is explicit.
    LossDeclared,
}

impl FromStr for ScientificListeningPredicate {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "scientific:reversible" => Ok(Self::Reversible),
            "scientific:loss-declared" => Ok(Self::LossDeclared),
            _ => Err(Error::Eval(
                "not a scientific listening predicate".to_owned(),
            )),
        }
    }
}

/// Predicate accepted only by artistic-mapping listeners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtisticListeningPredicate {
    /// Require links to the input material.
    InputsLinked,
    /// Require authored choices to be named.
    ChoicesAttributed,
}

impl FromStr for ArtisticListeningPredicate {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "artistic:inputs-linked" => Ok(Self::InputsLinked),
            "artistic:choices-attributed" => Ok(Self::ChoicesAttributed),
            _ => Err(Error::Eval(
                "not an artistic listening predicate".to_owned(),
            )),
        }
    }
}

fn non_empty(value: String, label: &str) -> Result<String> {
    if value.trim().is_empty() {
        Err(Error::Eval(format!("{label} must not be empty")))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> ListeningContent {
        ListeningContent {
            score: "score:ice-core-17".to_owned(),
            graph: "graph:stereo".to_owned(),
            patch: "patch:sine".to_owned(),
            tuning: "tuning:12edo".to_owned(),
            audification: Some(AudificationMapping {
                input: MeasuredSeries {
                    content_key: "series:temperature".to_owned(),
                    quantity_kind: "temperature".to_owned(),
                    unit: "kelvin".to_owned(),
                    values: vec![260.0, 261.5, 263.0],
                },
                output_quantity_kind: "frequency".to_owned(),
                output_unit: "hertz".to_owned(),
                normalization: Normalization::Linear {
                    source_min: 250.0,
                    source_max: 280.0,
                },
                clipping: Clipping::Reject,
                interpolation: Interpolation::Linear,
                inverse: InverseMapping::Available {
                    operation: "linear-denormalize".to_owned(),
                    tolerance: "1e-12 kelvin".to_owned(),
                },
            }),
            artistic_mapping: None,
            choices: vec!["choice:register-4".to_owned()],
        }
    }

    #[test]
    fn route_churn_preserves_musical_and_expedition_identity() {
        let mut session = ListeningSession::new(
            "music:ice-core",
            Some("expedition:2026-north".to_owned()),
            content(),
        )
        .unwrap();
        let identity = session.identity();
        let expected = (
            identity.0.to_owned(),
            identity.1.map(str::to_owned),
            identity.2.clone(),
        );

        for (kind, target, detail) in [
            (
                RouteEventKind::Open,
                ListeningTarget::MediaEdgeHost,
                "edge opened",
            ),
            (
                RouteEventKind::Fail,
                ListeningTarget::RxV777,
                "adapter unavailable",
            ),
            (
                RouteEventKind::Fallback,
                ListeningTarget::OfflineRender,
                "render substituted",
            ),
            (
                RouteEventKind::Close,
                ListeningTarget::OfflineRender,
                "render complete",
            ),
            (
                RouteEventKind::Reconnect,
                ListeningTarget::HeavyDawHost,
                "DAW restored",
            ),
            (
                RouteEventKind::Open,
                ListeningTarget::Oasys,
                "workstation opened",
            ),
        ] {
            session.append_route_event(kind, target, detail).unwrap();
        }
        session.set_live_handle(Some("ephemeral:device-42".to_owned()));

        assert_eq!(
            session.identity(),
            (expected.0.as_str(), expected.1.as_deref(), &expected.2)
        );
        assert_eq!(session.route_events().len(), 6);
        assert_eq!(session.route_events()[5].sequence, 5);
    }

    #[test]
    fn scientific_and_artistic_predicates_cannot_decode_as_each_other() {
        assert!(
            "scientific:reversible"
                .parse::<ScientificListeningPredicate>()
                .is_ok()
        );
        assert!(
            "scientific:reversible"
                .parse::<ArtisticListeningPredicate>()
                .is_err()
        );
        assert!(
            "artistic:choices-attributed"
                .parse::<ArtisticListeningPredicate>()
                .is_ok()
        );
        assert!(
            "artistic:choices-attributed"
                .parse::<ScientificListeningPredicate>()
                .is_err()
        );
    }

    #[test]
    fn artistic_mapping_is_labelled_and_linked_to_inputs_and_choices() {
        let mapping = ArtisticMapping {
            input_content_keys: vec!["series:temperature".to_owned()],
            authored_choice_keys: vec!["choice:orchestration".to_owned()],
        };
        let mut artistic = content();
        artistic.audification = None;
        artistic.artistic_mapping = Some(mapping.clone());
        let session = ListeningSession::new("music:art", None, artistic).unwrap();

        assert_eq!(ArtisticMapping::KIND, "artistic-mapping");
        assert_eq!(session.content.artistic_mapping, Some(mapping));
    }

    #[test]
    fn unsupported_target_retains_exact_evidence() {
        let target = ListeningTarget::Unsupported {
            requested: "studio-orbit".to_owned(),
            evidence: "no registered EvalFabric site or offline adapter".to_owned(),
        };
        let mut session = ListeningSession::new("music:ice-core", None, content()).unwrap();
        session
            .append_route_event(RouteEventKind::Fail, target.clone(), "substitution refused")
            .unwrap();
        assert_eq!(session.route_events()[0].target, target);
    }
}
