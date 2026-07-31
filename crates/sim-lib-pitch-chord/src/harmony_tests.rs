use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_ratio::PitchRatio;
use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_set::PitchClassMask;

use crate::{
    ChordPalette, ChordTemplate, CoreHarmonyMetricResolver, CountRange, Fingering,
    HarmonyConstraint, HarmonyEvaluationContext, HarmonyMetric, HarmonyPredicate, HarmonyProgram,
    HarmonyRenderProfile, HarmonyRuleSet, PaletteAlgebra, TemplateChain, VoicingChange,
    VoicingChangePalette, Weighted, evaluate_harmony,
};

fn chord(id: &str, classes: &[PitchClass]) -> ChordTemplate {
    ChordTemplate::from_pitch_classes(id, classes.to_vec(), 4)
}

fn mask(classes: &[PitchClass]) -> PitchClassMask {
    PitchClassMask::from_pitch_classes(classes)
}

fn render_profile() -> HarmonyRenderProfile {
    HarmonyRenderProfile {
        id: "catalog-organ-trumpet".to_owned(),
        chord_transpose: 48,
        melody_transpose: 60,
        duration_multiplier: 4,
        chord_program: 19,
        melody_program: 56,
        tempo_bpm: 60,
        time_signature: (4, 4),
    }
}

#[test]
fn chord_sources_palette_algebra_and_program_round_trip_as_expression_data() {
    let c = ChordTemplate::from_symbol("c-major", "C", 4).with_ratios(vec![
        PitchRatio::unison(),
        PitchRatio::new(5, 4).unwrap(),
        PitchRatio::new(3, 2).unwrap(),
    ]);
    let g = ChordTemplate::from_scale_degrees(
        "g-scale-stack",
        Scale::major(PitchClass::C),
        vec![5, 7, 9],
        4,
    );
    let f = ChordTemplate::from_pitch_set(
        "f-pitch-set",
        mask(&[PitchClass::F, PitchClass::A, PitchClass::C]),
        PitchClass::F,
        4,
    );
    let cadence = TemplateChain::new("cadence", vec![f.clone(), g.clone(), c.clone()]).unwrap();
    let explicit =
        ChordPalette::explicit("catalog", vec![c.clone(), g.clone()], vec![cadence]).unwrap();
    assert_eq!(
        ChordPalette::from_expr(&explicit.to_expr()).unwrap(),
        explicit
    );

    let left = ChordPalette::explicit("left", vec![f], Vec::new()).unwrap();
    let right = ChordPalette::explicit("right", vec![g], Vec::new()).unwrap();
    let alternative =
        ChordPalette::alternative("alternative", &[left.clone(), right.clone()]).unwrap();
    let chained = ChordPalette::chain("chain", &[left.clone(), right.clone()]).unwrap();
    let transposed = ChordPalette::transpose("transposed", &left, &[0, 2, 4]).unwrap();
    for palette in [alternative, chained, transposed] {
        assert_eq!(
            ChordPalette::from_expr(&palette.to_expr()).unwrap(),
            palette
        );
    }

    let rules = all_catalog_rules();
    let changes = VoicingChangePalette::from_chord_palette("changes", &explicit, 12).unwrap();
    let program = HarmonyProgram {
        id: "catalog-harmony".to_owned(),
        palette: explicit,
        rules,
        voicing_changes: changes,
        render: render_profile(),
    };
    assert_eq!(
        HarmonyProgram::from_expr(&program.to_expr()).unwrap(),
        program
    );
}

#[test]
fn all_catalog_harmony_and_template_filters_are_serializable() {
    let rules = all_catalog_rules();
    assert_eq!(rules.hard.len(), 23);
    assert_eq!(rules.soft.len(), 6);
    let palette = ChordPalette::explicit(
        "one",
        vec![chord("c", &[PitchClass::C, PitchClass::E, PitchClass::G])],
        Vec::new(),
    )
    .unwrap();
    let program = HarmonyProgram {
        id: "all-rules".to_owned(),
        palette,
        rules,
        voicing_changes: VoicingChangePalette::empty("none").unwrap(),
        render: render_profile(),
    };
    let decoded = HarmonyProgram::from_expr(&program.to_expr()).unwrap();
    assert_eq!(decoded.rules, program.rules);
}

#[test]
fn voicing_changes_keep_duplicate_voices_and_reject_bad_fingering_bounds() {
    let source = chord(
        "source",
        &[PitchClass::C, PitchClass::E, PitchClass::G, PitchClass::C],
    );
    let target = chord(
        "target",
        &[PitchClass::F, PitchClass::A, PitchClass::C, PitchClass::F],
    );
    let change = VoicingChange::between("source-to-target", &source, &target, 12).unwrap();

    assert_eq!(change.leading.indices.len(), 4);
    let mut sorted = change.leading.indices.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![0, 1, 2, 3]);
    assert_eq!(change.apply(&target).unwrap().len(), 4);
    assert!(Fingering::new(vec![0, 4], 4).is_err());
}

#[test]
fn palette_transposition_deduplicates_symmetric_pitch_sets() {
    let augmented = ChordPalette::explicit(
        "augmented",
        vec![chord(
            "aug",
            &[PitchClass::C, PitchClass::E, PitchClass::GS],
        )],
        Vec::new(),
    )
    .unwrap();
    let expanded =
        ChordPalette::transpose("augmented/octave", &augmented, &(0..12).collect::<Vec<_>>())
            .unwrap();

    assert_eq!(expanded.entries.len(), 4);
    assert!(matches!(expanded.algebra, PaletteAlgebra::Transpose { .. }));
}

#[test]
fn hard_legality_and_soft_score_are_separate_and_keep_per_rule_evidence() {
    let progression = vec![ChordTemplate::from_symbol("c", "C", 4).with_ratios(vec![
        PitchRatio::unison(),
        PitchRatio::new(5, 4).unwrap(),
        PitchRatio::new(3, 2).unwrap(),
    ])];
    let rules = HarmonyRuleSet {
        hard: vec![
            HarmonyConstraint::new(
                "needs-four-pitches",
                HarmonyPredicate::DistinctPitchClasses {
                    count: CountRange::new(4, 4).unwrap(),
                },
            ),
            HarmonyConstraint::new("observed-depth", HarmonyPredicate::ObserveDepth),
        ],
        soft: vec![
            Weighted::new("pitch-count", 10.0, HarmonyMetric::DistinctPitchClasses),
            Weighted::new(
                "ratio-cost",
                1.0,
                HarmonyMetric::RatioComplexity {
                    exponent_milli: 2_000,
                },
            ),
        ],
    };
    let result = evaluate_harmony(
        &rules,
        HarmonyEvaluationContext::progression(&[mask(&[PitchClass::C])], &progression),
        &CoreHarmonyMetricResolver,
    )
    .unwrap();

    assert!(!result.legal);
    assert_eq!(result.hard.len(), 2);
    assert_eq!(result.hard[0].rule_id, "needs-four-pitches");
    assert!(!result.hard[0].passed);
    assert!(result.hard[1].passed);
    assert_eq!(result.soft.len(), 2);
    assert_eq!(result.soft[0].weighted_score, 30.0);
    assert!(result.score > result.soft[0].weighted_score);
}

#[test]
fn empty_prefixes_are_safe_and_scale_windows_enforce_both_directions() {
    let empty_rules = HarmonyRuleSet {
        hard: vec![
            HarmonyConstraint::new("melody", HarmonyPredicate::MelodyInChord),
            HarmonyConstraint::new(
                "type-distance",
                HarmonyPredicate::MinimumTypeDistance { distance: 1 },
            ),
        ],
        soft: Vec::new(),
    };
    let empty = evaluate_harmony(
        &empty_rules,
        HarmonyEvaluationContext::progression(&[mask(&[PitchClass::C])], &[]),
        &CoreHarmonyMetricResolver,
    )
    .unwrap();
    assert!(empty.legal);
    assert!(empty.hard.iter().all(|evidence| evidence.passed));

    let diatonic = vec![
        chord("c", &[PitchClass::C, PitchClass::E, PitchClass::G]),
        chord("f", &[PitchClass::F, PitchClass::A, PitchClass::C]),
        chord("g", &[PitchClass::G, PitchClass::B, PitchClass::D]),
    ];
    let rules = HarmonyRuleSet {
        hard: vec![
            HarmonyConstraint::new(
                "inside",
                HarmonyPredicate::InsideScaleWindow {
                    scale: Scale::major(PitchClass::C),
                    length: 3,
                },
            ),
            HarmonyConstraint::new(
                "outside",
                HarmonyPredicate::OutsideScaleWindow {
                    scale: Scale::major(PitchClass::C),
                    length: 3,
                },
            ),
        ],
        soft: Vec::new(),
    };
    let result = evaluate_harmony(
        &rules,
        HarmonyEvaluationContext::progression(&[], &diatonic),
        &CoreHarmonyMetricResolver,
    )
    .unwrap();
    assert!(result.hard[0].passed);
    assert!(!result.hard[1].passed);
}

#[test]
fn variation_compares_chord_values_and_template_rules_reject_bad_connections() {
    let c = chord("c", &[PitchClass::C, PitchClass::E, PitchClass::G]);
    let c_copy = chord("c-copy", &[PitchClass::C, PitchClass::E, PitchClass::G]);
    let progression = vec![c.clone(), c_copy];
    let disconnected = vec![
        TemplateChain::new(
            "first",
            vec![
                c,
                chord("g", &[PitchClass::G, PitchClass::B, PitchClass::D]),
            ],
        )
        .unwrap(),
        TemplateChain::new(
            "second",
            vec![
                chord("f", &[PitchClass::F, PitchClass::A, PitchClass::C]),
                chord("c2", &[PitchClass::C, PitchClass::E, PitchClass::G]),
            ],
        )
        .unwrap(),
    ];
    let rules = HarmonyRuleSet {
        hard: vec![
            HarmonyConstraint::new(
                "variation",
                HarmonyPredicate::PeriodicVariation { period: 1 },
            ),
            HarmonyConstraint::new("connect", HarmonyPredicate::TemplatesConnect),
        ],
        soft: Vec::new(),
    };
    let result = evaluate_harmony(
        &rules,
        HarmonyEvaluationContext::progression(&[], &progression).with_templates(&disconnected),
        &CoreHarmonyMetricResolver,
    )
    .unwrap();

    assert!(!result.hard[0].passed);
    assert!(!result.hard[1].passed);
}

fn all_catalog_rules() -> HarmonyRuleSet {
    let c = mask(&[PitchClass::C, PitchClass::E, PitchClass::G]);
    let exact_three = CountRange::new(3, 3).unwrap();
    let two_or_three = CountRange::new(2, 3).unwrap();
    let hard = vec![
        ("always", HarmonyPredicate::Always),
        ("melody-in-chord", HarmonyPredicate::MelodyInChord),
        (
            "position-is",
            HarmonyPredicate::ChordAt {
                position: 0,
                chord: c,
            },
        ),
        (
            "first-is",
            HarmonyPredicate::ChordAt {
                position: 0,
                chord: c,
            },
        ),
        (
            "last-is",
            HarmonyPredicate::ChordAt {
                position: -1,
                chord: c,
            },
        ),
        (
            "not-position-is",
            HarmonyPredicate::ChordEverywhereExcept {
                position: 2,
                chord: c,
            },
        ),
        (
            "not-position-is-not",
            HarmonyPredicate::ChordOnlyAt {
                position: -1,
                chord: c,
            },
        ),
        ("at", HarmonyPredicate::AtPosition { position: 3 }),
        (
            "distinct-note-count",
            HarmonyPredicate::DistinctPitchClasses { count: exact_three },
        ),
        (
            "common-notes",
            HarmonyPredicate::CommonNotes {
                count: two_or_three,
            },
        ),
        (
            "common-note-rhythm",
            HarmonyPredicate::CommonNotePattern {
                counts: vec![3, 2, 3, 1],
            },
        ),
        (
            "minimum-chord-distance",
            HarmonyPredicate::MinimumChordDistance { distance: 2 },
        ),
        (
            "maximum-chord-distance",
            HarmonyPredicate::MaximumChordDistance { distance: 4 },
        ),
        (
            "minimum-type-distance",
            HarmonyPredicate::MinimumTypeDistance { distance: 1 },
        ),
        (
            "chord-variation",
            HarmonyPredicate::PeriodicVariation { period: 4 },
        ),
        (
            "chord-commonality",
            HarmonyPredicate::PeriodicCommonality {
                period: 4,
                count: two_or_three,
            },
        ),
        (
            "minimum-scale-duration",
            HarmonyPredicate::InsideScaleWindow {
                scale: Scale::major(PitchClass::C),
                length: 4,
            },
        ),
        (
            "maximum-scale-duration",
            HarmonyPredicate::OutsideScaleWindow {
                scale: Scale::major(PitchClass::C),
                length: 8,
            },
        ),
        ("template-length", HarmonyPredicate::TemplateLength),
        ("will-connect", HarmonyPredicate::TemplatesConnect),
        (
            "template-melody-in-chord",
            HarmonyPredicate::TemplateMelodyInChord,
        ),
        ("log-depth", HarmonyPredicate::ObserveDepth),
        (
            "boolean-composition",
            HarmonyPredicate::All(vec![
                HarmonyPredicate::Always,
                HarmonyPredicate::Any(vec![
                    HarmonyPredicate::AtPosition { position: 0 },
                    HarmonyPredicate::Not(Box::new(HarmonyPredicate::AtPosition { position: 1 })),
                ]),
            ]),
        ),
    ]
    .into_iter()
    .map(|(id, predicate)| HarmonyConstraint::new(id, predicate))
    .collect();
    let soft = vec![
        Weighted::new("distinct-score", 1.0, HarmonyMetric::DistinctPitchClasses),
        Weighted::new("common-score", -1.0, HarmonyMetric::CommonNotes),
        Weighted::new("leading-score", 1.0, HarmonyMetric::VoiceLeading),
        Weighted::new(
            "dissonance-score",
            1.0,
            HarmonyMetric::PitchDissonance {
                model: "interval-vector".to_owned(),
            },
        ),
        Weighted::new(
            "sonance-score",
            -1.0,
            HarmonyMetric::ContextualSonance {
                model: "commonality".to_owned(),
            },
        ),
        Weighted::new(
            "ratio-score",
            1.0,
            HarmonyMetric::RatioComplexity {
                exponent_milli: 2_000,
            },
        ),
    ];
    HarmonyRuleSet { hard, soft }
}
