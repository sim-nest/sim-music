use sim_kernel::{Cx, DefaultFactory, EagerPolicy, ExportKind, Symbol};
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_scale::{Key, Mode};
use sim_lib_pitch_set::PitchClassMask;
use std::sync::Arc;

use crate::{
    ContextualPitch, ContextualSonanceOptions, ContextualSonanceRegistry, DuplicatePolicy,
    IntervalDifferenceMode, IntervalMergeMode, PitchDissonanceDialect, PitchDissonanceOptions,
    SonanceNormalization,
};
use crate::{PitchDissonanceRegistry, install_pitch_dissonance_lib};

#[test]
fn analysis_returns_one_score_per_model() {
    let registry = PitchDissonanceRegistry::new_with_builtins();
    let scores = registry.analyze_all(
        PitchClassMask::from_pitch_classes(&[
            PitchClass::C,
            PitchClass::E,
            PitchClass::G,
            PitchClass::AS,
        ]),
        &LabelContext {
            root: Some(PitchClass::C),
            key: Some(Key {
                tonic: PitchClass::C,
                mode: Mode::Major,
            }),
        },
    );
    assert_eq!(scores.len(), 4);
    assert!(scores.iter().all(|score| score.score.is_finite()));
    assert!(
        scores
            .iter()
            .all(|score| score.sonance.roughness_mass.is_finite())
    );
    assert!(
        scores
            .iter()
            .all(|score| score.sonance.evidence.normalization == "interval-class")
    );
}

#[test]
fn install_pitch_dissonance_lib_registers_builtin_models_as_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_pitch_dissonance_lib(&mut cx).unwrap();
    install_pitch_dissonance_lib(&mut cx).unwrap();

    let loaded = cx
        .registry()
        .lib(&Symbol::new("pitch-dissonance"))
        .expect("pitch dissonance lib");
    let model_exports = loaded
        .exports
        .iter()
        .filter(|record| record.kind == ExportKind::named("PitchDissonanceModel"))
        .count();
    assert_eq!(model_exports, 11);
    assert!(
        cx.registry()
            .value_by_symbol(&Symbol::qualified("pitch", "IntervalVectorModel"))
            .is_some()
    );
}

#[test]
fn tritone_density_uses_standard_tritone_and_names_legacy_dialect() {
    let registry = PitchDissonanceRegistry::new_with_builtins();
    let mask = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::FS]);
    let standard = registry.analyze_all_with_options(
        mask,
        &LabelContext::default(),
        PitchDissonanceOptions::standard(),
    );
    let legacy = registry.analyze_all_with_options(
        mask,
        &LabelContext::default(),
        PitchDissonanceOptions::legacy_tritone_ic5(),
    );

    let standard_tritone = standard
        .iter()
        .find(|score| score.model == "tritone-density")
        .unwrap();
    let legacy_tritone = legacy
        .iter()
        .find(|score| score.model == "tritone-density")
        .unwrap();

    assert_eq!(standard_tritone.sonance.normalized_density, 1.0);
    assert_eq!(standard_tritone.sonance.evidence.dialect, "standard");
    assert_eq!(legacy_tritone.sonance.normalized_density, 0.0);
    assert_eq!(
        legacy_tritone.sonance.evidence.dialect,
        "legacy-tritone-ic5"
    );
}

#[test]
fn interval_options_change_density_aggregation_without_collapsing_components() {
    let registry = PitchDissonanceRegistry::new_with_builtins();
    let mask = PitchClassMask::from_pitch_classes(&[
        PitchClass::C,
        PitchClass::DS,
        PitchClass::FS,
        PitchClass::A,
    ]);
    let scores = registry.analyze_all_with_options(
        mask,
        &LabelContext::default(),
        PitchDissonanceOptions {
            difference: IntervalDifferenceMode::DirectedSemitone,
            merge: IntervalMergeMode::MeanPairs,
            dialect: PitchDissonanceDialect::Standard,
        },
    );
    let interval = scores
        .iter()
        .find(|score| score.model == "interval-vector")
        .unwrap();

    assert_eq!(interval.sonance.evidence.normalization, "directed-semitone");
    assert_eq!(interval.sonance.evidence.aggregation, "mean-pairs");
    assert!(interval.sonance.roughness_mass > 0.0);
    assert!(interval.sonance.normalized_density > 0.0);
}

#[test]
fn contextual_registry_exposes_named_sonance_models() {
    let registry = ContextualSonanceRegistry::new_with_builtins();
    let names = registry.list();

    for name in [
        "commonality",
        "interval-vector",
        "leading",
        "motion",
        "pseudo-partial",
        "ratio",
        "roughness",
    ] {
        assert!(names.contains(&name));
    }
}

#[test]
fn contextual_compare_matches_lisp_specimen_and_retains_identity() {
    let registry = ContextualSonanceRegistry::new_with_builtins();
    let from = voiced_notes(&[("s", "C4"), ("a", "E4"), ("b", "G4")]);
    let to = voiced_notes(&[("s", "B3"), ("a", "D4"), ("b", "G4")]);
    let report = registry.compare_named(
        &["roughness", "commonality", "leading", "ratio"],
        &from,
        &to,
        ContextualSonanceOptions {
            duplicates: DuplicatePolicy::Retain,
            normalization: SonanceNormalization::PerPair,
            ..ContextualSonanceOptions::standard()
        },
    );

    assert_eq!(report.components.len(), 4);
    assert_eq!(report.from.ids, ["s:C4", "a:E4", "b:G4"]);
    assert_eq!(report.to.ids, ["s:B3", "a:D4", "b:G4"]);
    assert!(report.total_score().is_finite());
    assert!(
        report
            .components
            .iter()
            .all(|component| component.sonance.evidence.normalization == "per-pair")
    );
}

#[test]
fn duplicate_notes_are_not_hidden_by_pitch_class_sets() {
    let registry = ContextualSonanceRegistry::new_with_builtins();
    let from = vec![
        ContextualPitch::unvoiced("c4-1", "C4".parse::<Pitch>().unwrap()),
        ContextualPitch::unvoiced("c4-2", "C4".parse::<Pitch>().unwrap()),
    ];
    let to = vec![ContextualPitch::unvoiced(
        "c4-only",
        "C4".parse::<Pitch>().unwrap(),
    )];
    let retained = registry.compare_named(
        &["commonality"],
        &from,
        &to,
        ContextualSonanceOptions::standard(),
    );
    let collapsed = registry.compare_named(
        &["commonality"],
        &from,
        &to,
        ContextualSonanceOptions {
            duplicates: DuplicatePolicy::Collapse,
            ..ContextualSonanceOptions::standard()
        },
    );

    assert_eq!(retained.from.ids, ["c4-1", "c4-2"]);
    assert!(retained.components[0].sonance.normalized_density > 0.0);
    assert_eq!(collapsed.components[0].sonance.normalized_density, 0.0);
}

#[test]
fn contextual_invariants_cover_permutation_octave_zero_amplitude_and_continuity() {
    let registry = ContextualSonanceRegistry::new_with_builtins();
    let options = ContextualSonanceOptions::standard();
    let c_major = voiced_notes(&[("s", "C4"), ("a", "E4"), ("b", "G4")]);
    let c_major_permuted = voiced_notes(&[("b", "G4"), ("s", "C4"), ("a", "E4")]);
    let c_major_octave = voiced_notes(&[("s", "C5"), ("a", "E5"), ("b", "G5")]);
    let mut silent = c_major.clone();
    silent[1].amplitude = 0.0;

    let base = registry.compare_named(&["interval-vector", "ratio"], &c_major, &c_major, options);
    let permuted = registry.compare_named(
        &["interval-vector", "ratio"],
        &c_major_permuted,
        &c_major_permuted,
        options,
    );
    let octave = registry.compare_named(
        &["interval-vector", "ratio"],
        &c_major_octave,
        &c_major_octave,
        options,
    );
    let zero = registry.compare_named(&["roughness"], &silent, &silent, options);
    let continuity = registry.compare_named(
        &["leading", "motion"],
        &c_major,
        &voiced_notes(&[("a", "D4"), ("b", "G4"), ("s", "B3")]),
        options,
    );

    assert_eq!(base.components[0].sonance.roughness_mass, 0.0);
    assert_eq!(permuted.components[0].sonance.roughness_mass, 0.0);
    assert_eq!(octave.components[0].sonance.roughness_mass, 0.0);
    assert!(zero.components[0].score.is_finite());
    assert!(
        continuity.components[0]
            .sonance
            .evidence
            .provenance
            .iter()
            .any(|fact| fact == "paired-voices=3")
    );
}

fn voiced_notes(notes: &[(&str, &str)]) -> Vec<ContextualPitch> {
    notes
        .iter()
        .map(|(voice, spelling)| ContextualPitch {
            id: format!("{voice}:{spelling}"),
            voice: Some((*voice).to_owned()),
            pitch: spelling.parse::<Pitch>().unwrap(),
            amplitude: 1.0,
        })
        .collect()
}
