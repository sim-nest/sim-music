use std::collections::BTreeMap;
use std::sync::Arc;

use sim_lib_music_core::{Articulation, Channel, Score, Time};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_scale::{PlayerScale, Scale};

use crate::tests::{quarter, strict_context, strict_plan, voice};
use crate::{
    RealizationContext, RealizerId, RowInstanceId, SerialEventId, SerialPlan, SerialRealization,
    SerialRealizer, SerialRealizerRegistry, SerialRenderOptions, SerialSpineKind, SerialSpineLabel,
    StrictEventSpec, StrictRealizationContext, default_realizer_registry, realize_strict,
    render_serial_piano_roll, render_serial_score, render_serial_staff,
    strict_chromatic_realizer_id,
};

#[test]
fn strict_realization_preserves_plan_and_serial_origin() {
    let plan = strict_plan();
    let realization = realize_strict(&plan, &strict_context()).expect("realization");
    assert_eq!(realization.plan(), &plan);
    assert_eq!(realization.ledger().entries().len(), 3);
    assert!(realization.ledger().is_preserved("serial/ordinal-order"));
    assert!(
        realization
            .ledger()
            .is_preserved("serial/chromatic-aggregate")
    );
    assert!(realization.notes().iter().all(|note| {
        plan.event(&note.event_id).is_some()
            && !note.origin.ordinals.is_empty()
            && note.origin.realizer_id == strict_chromatic_realizer_id()
    }));
    assert!(realization.events().iter().any(|event| event.is_rest));
}

#[test]
fn realizer_registry_lists_ids_in_sorted_order_and_replaces_explicitly() {
    let chromatic = Arc::new(crate::ChromaticSerialRealizer::default());
    let test_realizer = Arc::new(TestOnlyRealizer::new("realizer/test-b"));
    let alpha = Arc::new(TestOnlyRealizer::new("realizer/test-a"));
    let mut registry = SerialRealizerRegistry::new();
    registry.register(test_realizer).expect("first insert");
    registry.register(chromatic).expect("second insert");
    registry.register(alpha.clone()).expect("third insert");

    let ids = registry.ids();
    assert_eq!(
        ids.iter().map(RealizerId::as_str).collect::<Vec<_>>(),
        vec![
            "realizer/strict-chromatic",
            "realizer/test-a",
            "realizer/test-b",
        ]
    );

    let replaced = registry.replace(Arc::new(TestOnlyRealizer::new("realizer/test-a")));
    assert_eq!(
        replaced.expect("replacement").id().as_str(),
        "realizer/test-a"
    );
}

#[test]
fn custom_realizer_receives_open_context_data_without_production_registry_changes() {
    let plan = strict_plan();
    let mut context = strict_context();
    context
        .services
        .insert("serial-hint", Arc::new(String::from("service/live")));

    let mut registry = SerialRealizerRegistry::new();
    let realizer = Arc::new(TestOnlyRealizer::new("realizer/test-only"));
    registry.register(realizer.clone()).expect("insert");
    let realization = registry
        .realize(realizer.id(), &plan, &context)
        .expect("custom realization");

    let note = realization.notes().first().expect("note");
    assert_eq!(note.origin.realizer_id.as_str(), "realizer/test-only");
    assert_eq!(realization.ledger().entries().len(), 1);
    assert_eq!(
        realization.ledger().entries()[0].rule_id.as_str(),
        "realizer/test-only"
    );
}

#[test]
fn modal_degree_cycle_realizers_preserve_plan_and_relax_chromatic_aggregate() {
    let plan = strict_plan();
    let registry = default_realizer_registry();
    let mut dorian_context = strict_context();
    dorian_context.modal_scale = Some(PlayerScale::from_scale(Scale::dorian(PitchClass::C)));
    let mut lydian_context = strict_context();
    lydian_context.modal_scale = Some(PlayerScale::from_scale(Scale::lydian(PitchClass::C)));

    let dorian = registry
        .realize_named("realizer/modal-degree-cycle", &plan, &dorian_context)
        .expect("dorian modal realization");
    let lydian = registry
        .realize_named("realizer/modal-degree-cycle", &plan, &lydian_context)
        .expect("lydian modal realization");

    assert_eq!(dorian.plan(), lydian.plan());
    assert_ne!(dorian.sounding_pitches(), lydian.sounding_pitches());
    assert!(dorian.ledger().is_preserved("serial/ordinal-order"));
    assert!(dorian.ledger().is_relaxed("serial/chromatic-aggregate"));

    let report = dorian.spine_report().expect("spine report");
    assert_eq!(report.kind, SerialSpineKind::DegreeCycle);
    assert!(!report.modal_membership().is_empty());
    assert!(!report.pitch_identity().is_empty());
    assert!(
        !report
            .chromatic_aggregate_identity()
            .lost_source_classes
            .is_empty()
    );
    assert_eq!(report.ordinal_order().len(), dorian.notes().len());
    assert!(!report.sonance_context().is_empty());
}

#[test]
fn modal_realizers_support_marked_and_non_pitch_spines_plus_custom_scale() {
    let plan = strict_plan();
    let registry = default_realizer_registry();
    let mut custom_context = strict_context();
    custom_context.modal_scale =
        Some(PlayerScale::custom(PitchClass::C, vec![0, 1, 4, 5, 7, 8, 10]).expect("custom scale"));

    let marked = registry
        .realize_named(
            "realizer/modal-marked-chromatic-inflection",
            &plan,
            &custom_context,
        )
        .expect("marked realization");
    let marked_report = marked.spine_report().expect("marked spine report");
    assert_eq!(
        marked_report.kind,
        SerialSpineKind::MarkedChromaticInflection
    );
    assert!(marked_report.entries.iter().any(|entry| {
        matches!(
            entry.label,
            SerialSpineLabel::ChromaticInflection {
                semitone_delta,
                ..
            } if semitone_delta != 0
        )
    }));

    let non_pitch = registry
        .realize_named("realizer/modal-non-pitch-spine", &plan, &custom_context)
        .expect("non-pitch realization");
    let non_pitch_report = non_pitch.spine_report().expect("non-pitch report");
    assert_eq!(non_pitch_report.kind, SerialSpineKind::NonPitchSpine);
    assert!(
        non_pitch_report
            .entries
            .iter()
            .all(|entry| { matches!(entry.label, SerialSpineLabel::OrdinalToken { .. }) })
    );

    let nearest = registry
        .realize_named("realizer/modal-nearest-scale-tone", &plan, &custom_context)
        .expect("nearest realization");
    let nearest_report = nearest.spine_report().expect("nearest report");
    assert_eq!(nearest_report.kind, SerialSpineKind::NearestScaleTone);
    assert!(
        nearest_report
            .entries
            .iter()
            .all(|entry| { matches!(entry.label, SerialSpineLabel::LandedPitch(_)) })
    );
}

#[test]
fn strict_realization_renders_chords_crossings_and_equal_pitch_multiplicity() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let staff = render_serial_staff(&realization).expect("staff");
    let roll = render_serial_piano_roll(&realization).expect("roll");

    assert_eq!(staff.voices.len(), 3);
    assert!(
        staff
            .voices
            .iter()
            .find(|staff_voice| staff_voice.id == voice("voice/high"))
            .expect("high voice")
            .notes
            .iter()
            .any(|note| note.note.pitch.to_midi() == Some(48))
    );
    assert!(
        staff
            .voices
            .iter()
            .find(|staff_voice| staff_voice.id == voice("voice/low"))
            .expect("low voice")
            .notes
            .iter()
            .any(|note| note.note.pitch.to_midi() == Some(80))
    );

    let tied = realization
        .notes()
        .iter()
        .find(|note| note.event_id == SerialEventId::new("event/tie-a").unwrap())
        .expect("tied note");
    assert_eq!(tied.note.duration, Time::new(1, 2));
    assert!(
        realization
            .notes()
            .iter()
            .all(|note| note.event_id != SerialEventId::new("event/tie-b").unwrap())
    );

    assert!(roll.note_slices().into_iter().any(|slice| {
        slice
            .notes
            .iter()
            .filter(|note| note.timed.note.pitch.to_midi() == Some(71))
            .count()
            == 2
    }));
}

#[test]
fn strict_render_keeps_trailing_rest_duration_for_existing_voice() {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let note_id = SerialEventId::new("event/opening").expect("event id");
    let rest_id = SerialEventId::new("event/trailing-rest").expect("event id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), crate::tests::op25_form());
    let events = [
        crate::tests::event(
            "event/opening",
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            "voice/high",
        ),
        crate::tests::event("event/trailing-rest", &[11], "voice/high"),
    ]
    .into_iter()
    .map(|event| (event.id.clone(), event))
    .collect();
    let plan =
        SerialPlan::try_new(rows, events, [(note_id.clone(), rest_id.clone())]).expect("plan");
    let channel = Channel::new(0).expect("channel");
    let context = StrictRealizationContext::new(
        [
            (
                note_id,
                StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
            ),
            (rest_id, StrictEventSpec::rest(Time::new(3, 4))),
        ]
        .into_iter()
        .collect(),
    );
    let realization = realize_strict(&plan, &context).expect("realization");
    let staff = render_serial_staff(&realization).expect("staff");
    let high_voice = staff
        .voices
        .iter()
        .find(|staff_voice| staff_voice.id == voice("voice/high"))
        .expect("high voice");
    assert_eq!(high_voice.duration, Time::from_integer(1));
}

#[test]
fn strict_realization_wraps_the_canonical_piano_roll_in_a_score() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let score = render_serial_score(&realization, &SerialRenderOptions::default()).expect("score");
    assert_eq!(score.tempo_bpm, 60);
    let Score { body, .. } = score;
    let sim_lib_music_core::Music::PianoRoll(roll) = body else {
        panic!("score should render to piano roll");
    };
    assert_eq!(roll.lanes.len(), 3);
    assert!(roll.items.len() >= realization.notes().len());
}

#[derive(Clone, Debug)]
struct TestOnlyRealizer {
    id: RealizerId,
}

impl TestOnlyRealizer {
    fn new(id: &str) -> Self {
        Self {
            id: RealizerId::new(id).expect("realizer id"),
        }
    }
}

impl SerialRealizer for TestOnlyRealizer {
    fn id(&self) -> &RealizerId {
        &self.id
    }

    fn realize(
        &self,
        plan: &SerialPlan,
        context: &RealizationContext,
    ) -> Result<SerialRealization, crate::StrictRealizationError> {
        assert!(context.scale.is_some());
        assert!(context.tuning.is_some());
        assert_eq!(
            context
                .register_bounds
                .get(&voice("voice/high"))
                .expect("register bound")
                .highest,
            6
        );
        assert_eq!(
            context
                .voice_bounds
                .get(&voice("voice/high"))
                .expect("voice bound")
                .max_notes_per_event,
            Some(2)
        );
        assert_eq!(
            context
                .services
                .get::<String>("serial-hint")
                .expect("service"),
            "service/live"
        );
        realize_strict(plan, context).map(|mut realization| {
            let first = realization
                .notes()
                .first()
                .expect("note")
                .origin
                .realizer_id
                .clone();
            if first == self.id {
                return realization;
            }
            let cloned = realization.clone();
            let events = cloned.events().to_vec();
            let notes = cloned
                .notes()
                .iter()
                .cloned()
                .map(|mut note| {
                    note.origin.realizer_id = self.id.clone();
                    note
                })
                .collect();
            let ledger = crate::InvariantLedger::new(vec![crate::InvariantLedgerEntry::new(
                self.id.clone(),
                "test-only registry registration received the full open realization context",
                "custom realizer observed scale, tuning, bounds, and services",
                crate::InvariantStatus::Preserved,
                vec![crate::EvidenceId::new("evidence/test-only").expect("evidence id")],
                None,
            )]);
            realization = SerialRealization::new(plan.clone(), events, notes, ledger);
            realization
        })
    }
}
