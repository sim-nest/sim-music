use sim_kernel::{Cx, DefaultFactory, EagerPolicy, ExportKind, Symbol};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_scale::{Key, Mode};
use sim_lib_pitch_set::PitchClassMask;
use std::sync::Arc;

use crate::{
    IntervalDifferenceMode, IntervalMergeMode, PitchDissonanceDialect, PitchDissonanceOptions,
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
    assert_eq!(model_exports, 4);
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
