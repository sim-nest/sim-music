//! Open serial adaptation over landed modal pitch maps and non-pitch spines.

use std::collections::BTreeMap;

use sim_lib_pitch_dissonance::{
    ContextualPitch, ContextualSonanceOptions, ContextualSonanceRegistry,
};
use sim_lib_pitch_scale::PlayerScale;

use crate::chromatic::realize_chromatic_with_id;
use crate::pitch_map::{PitchMap, PitchMapPolicy};
use crate::spine::{
    SerialSonanceContext, SerialSpineEntry, SerialSpineKind, SerialSpineLabel, SerialSpineReport,
    aggregate_identity, collect_collisions, collect_repeated_degrees,
};
use crate::{
    EvidenceId, InvariantLedger, InvariantLedgerEntry, InvariantStatus, RealizationContext,
    RealizedSerialNote, RealizerId, SerialRealization, SerialRealizer, StrictRealizationError,
    WaiverId,
};

fn build_pitch_map(scale: &PlayerScale, policy: PitchMapPolicy) -> PitchMap {
    let mut image = vec![None; usize::from(sim_lib_pitch_core::OctaveSpace::twelve_tone().len())];
    for class in scale.pitch_classes() {
        image[usize::from(class.value())] = Some(i32::from(class.value()));
    }
    PitchMap::new(
        sim_lib_pitch_core::OctaveSpace::twelve_tone(),
        image,
        policy,
    )
    .expect("twelve-tone map")
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModalConfig {
    id: RealizerId,
    kind: SerialSpineKind,
    policy: PitchMapPolicy,
}

impl ModalConfig {
    fn new(id: &str, kind: SerialSpineKind, policy: PitchMapPolicy) -> Self {
        Self {
            id: RealizerId::new(id).expect("built-in modal realizer id"),
            kind,
            policy,
        }
    }
}

fn modal_realize(
    config: &ModalConfig,
    plan: &crate::SerialPlan,
    context: &RealizationContext,
) -> Result<SerialRealization, StrictRealizationError> {
    let scale = context
        .effective_modal_scale()
        .ok_or_else(|| StrictRealizationError::MissingModalScale(config.id.clone()))?;
    let pitch_map = build_pitch_map(&scale, config.policy);
    let base = realize_chromatic_with_id(&config.id, plan, context)?;

    let mut adapted_notes = Vec::<RealizedSerialNote>::with_capacity(base.notes().len());
    let mut entries = Vec::<SerialSpineEntry>::with_capacity(base.notes().len());
    for note in base.notes() {
        let result = pitch_map
            .map_pitch(note.note.pitch)
            .map_err(|error| StrictRealizationError::PitchMap(error.to_string()))?;
        let degree = scale.degree_of(result.pitch.class);
        let semitone_delta =
            i16::from(result.pitch.class.value()) - i16::from(note.note.pitch.class.value());
        let label = match config.kind {
            SerialSpineKind::DegreeCycle => {
                SerialSpineLabel::Degree(degree.expect("nearest policy lands inside the scale"))
            }
            SerialSpineKind::NearestScaleTone => SerialSpineLabel::LandedPitch(result.pitch),
            SerialSpineKind::MarkedChromaticInflection => SerialSpineLabel::ChromaticInflection {
                degree: degree.expect("nearest policy lands inside the scale"),
                semitone_delta,
            },
            SerialSpineKind::NonPitchSpine => SerialSpineLabel::OrdinalToken {
                ordinal: note.origin.source_ordinal.clone(),
                note_index: note.note_index,
            },
        };
        let mut adapted = note.clone();
        adapted.note.pitch = result.pitch;
        adapted_notes.push(adapted);
        entries.push(SerialSpineEntry {
            event_id: note.event_id.clone(),
            ordinal: note.origin.source_ordinal.clone(),
            note_index: note.note_index,
            onset: note.onset,
            source_pitch: note.note.pitch,
            landed_pitch: result.pitch,
            modal_degree: degree,
            modal_member: scale.contains(result.pitch.class),
            witness: result.witness,
            label,
        });
    }

    let sonance_context = collect_sonance_context(
        &adapted_notes,
        context
            .contextual_sonance
            .unwrap_or_else(ContextualSonanceOptions::standard),
    );
    let ordinal_order = entries
        .iter()
        .map(|entry| entry.ordinal.clone())
        .collect::<Vec<_>>();
    let collisions = collect_collisions(&entries);
    let repeated_degrees = collect_repeated_degrees(&entries);
    let aggregate_identity = aggregate_identity(&entries);
    let out_of_mode = entries
        .iter()
        .filter(|entry| !entry.modal_member)
        .map(|entry| entry.event_id.clone())
        .collect::<Vec<_>>();
    let pitch_changes = entries
        .iter()
        .filter(|entry| entry.source_pitch != entry.landed_pitch)
        .map(|entry| entry.event_id.clone())
        .collect::<Vec<_>>();
    let ledger = build_modal_ledger(config, base.notes().len(), &aggregate_identity);
    let spine_report = SerialSpineReport {
        realizer_id: config.id.clone(),
        kind: config.kind.clone(),
        scale,
        entries,
        collisions,
        repeated_degrees,
        out_of_mode,
        pitch_changes,
        aggregate_identity,
        ordinal_order,
        sonance_context,
    };
    Ok(SerialRealization::new_with_spine(
        base.plan().clone(),
        base.events().to_vec(),
        adapted_notes,
        ledger,
        Some(spine_report),
    ))
}

fn build_modal_ledger(
    config: &ModalConfig,
    note_count: usize,
    aggregate_identity: &crate::ChromaticAggregateIdentity,
) -> InvariantLedger<RealizerId> {
    let aggregate_status = if aggregate_identity.preserved {
        InvariantStatus::Preserved
    } else {
        InvariantStatus::Relaxed {
            waiver: WaiverId::new("waiver/modal-chromatic-aggregate").expect("waiver id"),
        }
    };
    InvariantLedger::new(vec![
        InvariantLedgerEntry::new(
            config.id.clone(),
            "serial ordinal order remains identical to the planned order",
            format!(
                "modal adaptation kept {} sounded ordinals in planned order",
                note_count
            ),
            InvariantStatus::Preserved,
            vec![EvidenceId::new("evidence/modal-ordinal-order").expect("evidence id")],
            None,
        )
        .with_invariant_id("serial/ordinal-order"),
        InvariantLedgerEntry::new(
            config.id.clone(),
            "the chromatic aggregate remains identical after adaptation",
            if aggregate_identity.preserved {
                "landed modal realization preserved the source aggregate exactly".to_owned()
            } else {
                format!(
                    "landed modal realization lost source classes {:?}",
                    aggregate_identity.lost_source_classes
                )
            },
            aggregate_status,
            vec![EvidenceId::new("evidence/modal-chromatic-aggregate").expect("evidence id")],
            (!aggregate_identity.preserved)
                .then(|| WaiverId::new("waiver/modal-chromatic-aggregate").expect("waiver id")),
        )
        .with_invariant_id("serial/chromatic-aggregate"),
        InvariantLedgerEntry::new(
            config.id.clone(),
            "modal membership, pitch identity, aggregate identity, ordinal order, and sonance remain inspectable independently",
            format!(
                "serial spine report retained {} adapted sounding notes",
                note_count
            ),
            InvariantStatus::Preserved,
            vec![EvidenceId::new("evidence/modal-spine-report").expect("evidence id")],
            None,
        ),
    ])
}

fn collect_sonance_context(
    notes: &[RealizedSerialNote],
    options: ContextualSonanceOptions,
) -> Vec<SerialSonanceContext> {
    let registry = ContextualSonanceRegistry::new_with_builtins();
    let mut by_event = BTreeMap::<crate::SerialEventId, Vec<&RealizedSerialNote>>::new();
    for note in notes {
        by_event
            .entry(note.event_id.clone())
            .or_default()
            .push(note);
    }
    let ordered = by_event.into_iter().collect::<Vec<_>>();
    ordered
        .windows(2)
        .map(|window| {
            let (from_event, from_notes) = &window[0];
            let (to_event, to_notes) = &window[1];
            let from = from_notes
                .iter()
                .enumerate()
                .map(|(index, note)| ContextualPitch {
                    id: format!("{from_event}/{index}"),
                    voice: Some(note.voice.as_str().to_owned()),
                    pitch: note.note.pitch,
                    amplitude: f64::from(note.note.velocity.max(1)),
                })
                .collect::<Vec<_>>();
            let to = to_notes
                .iter()
                .enumerate()
                .map(|(index, note)| ContextualPitch {
                    id: format!("{to_event}/{index}"),
                    voice: Some(note.voice.as_str().to_owned()),
                    pitch: note.note.pitch,
                    amplitude: f64::from(note.note.velocity.max(1)),
                })
                .collect::<Vec<_>>();
            SerialSonanceContext {
                from_event: from_event.clone(),
                to_event: to_event.clone(),
                report: registry.compare_all(&from, &to, options),
            }
        })
        .collect()
}

macro_rules! modal_realizer {
    ($name:ident, $doc:literal, $id:literal, $kind:expr, $policy:expr) => {
        #[doc = $doc]
        #[derive(Clone, Debug)]
        pub struct $name {
            config: ModalConfig,
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    config: ModalConfig::new($id, $kind, $policy),
                }
            }
        }

        impl SerialRealizer for $name {
            fn id(&self) -> &RealizerId {
                &self.config.id
            }

            fn realize(
                &self,
                plan: &crate::SerialPlan,
                context: &RealizationContext,
            ) -> Result<SerialRealization, StrictRealizationError> {
                modal_realize(&self.config, plan, context)
            }
        }
    };
}

modal_realizer!(
    ModalDegreeCycleRealizer,
    "Built-in realizer that lands chromatic source notes onto a modal degree cycle.",
    "realizer/modal-degree-cycle",
    SerialSpineKind::DegreeCycle,
    PitchMapPolicy::Nearest
);
modal_realizer!(
    NearestScaleToneRealizer,
    "Built-in realizer that lands each source note on the nearest pitch in the selected scale.",
    "realizer/modal-nearest-scale-tone",
    SerialSpineKind::NearestScaleTone,
    PitchMapPolicy::Nearest
);
modal_realizer!(
    MarkedChromaticInflectionRealizer,
    "Built-in realizer that lands notes in the scale and records chromatic inflection deltas.",
    "realizer/modal-marked-chromatic-inflection",
    SerialSpineKind::MarkedChromaticInflection,
    PitchMapPolicy::Nearest
);
modal_realizer!(
    NonPitchSpineRealizer,
    "Built-in realizer that lands notes in the scale while preserving a non-pitch ordinal spine.",
    "realizer/modal-non-pitch-spine",
    SerialSpineKind::NonPitchSpine,
    PitchMapPolicy::Nearest
);
