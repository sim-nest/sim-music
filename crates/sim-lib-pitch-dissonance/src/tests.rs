use sim_kernel::{Cx, DefaultFactory, EagerPolicy, ExportKind, Symbol};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_scale::{Key, Mode};
use sim_lib_pitch_set::PitchClassMask;
use std::sync::Arc;

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
