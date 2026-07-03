use sim_kernel::{Cx, DefaultFactory, EagerPolicy, ExportKind, Symbol};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_scale::{Key, Mode};
use sim_lib_pitch_set::PitchClassMask;
use std::sync::Arc;

use crate::{LabelContext, NamerRegistry, NamingSchool, install_pitch_namer_lib};

#[test]
fn label_all_returns_one_result_per_builtin_school() {
    let registry = NamerRegistry::new_with_builtins();
    let labels = registry.label_all(
        PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]),
        &LabelContext {
            root: Some(PitchClass::C),
            key: Some(Key {
                tonic: PitchClass::C,
                mode: Mode::Major,
            }),
        },
    );
    assert_eq!(labels.len(), 5);
}

#[test]
fn translation_preserves_canonical_pitch_class_mask() {
    let registry = NamerRegistry::new_with_builtins();
    let mask = PitchClassMask::from_pitch_classes(&[
        PitchClass::C,
        PitchClass::E,
        PitchClass::G,
        PitchClass::AS,
    ]);
    let source = registry.label_all(
        mask,
        &LabelContext {
            root: Some(PitchClass::C),
            key: Some(Key {
                tonic: PitchClass::F,
                mode: Mode::Major,
            }),
        },
    );
    let translated = registry
        .translate(&source[0], NamingSchool::Jazz)
        .expect("built-in school");
    assert_eq!(translated.meta.canonical_mask, mask.normalize());
}

#[test]
fn roman_without_key_reports_diagnostic() {
    let registry = NamerRegistry::new_with_builtins();
    let roman = registry
        .label_all(
            PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]),
            &LabelContext {
                root: Some(PitchClass::C),
                key: None,
            },
        )
        .into_iter()
        .find(|label| label.school == NamingSchool::FunctionalRoman)
        .expect("roman label");
    assert_eq!(
        roman.meta.diagnostic.as_deref(),
        Some("key context required")
    );
}

#[test]
fn install_pitch_namer_lib_registers_builtin_namers_as_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_pitch_namer_lib(&mut cx).unwrap();
    install_pitch_namer_lib(&mut cx).unwrap();

    let loaded = cx
        .registry()
        .lib(&Symbol::new("pitch-namer"))
        .expect("pitch namer lib");
    let namer_exports = loaded
        .exports
        .iter()
        .filter(|record| record.kind == ExportKind::named("ClusterNamer"))
        .count();
    assert_eq!(namer_exports, 5);
    assert!(
        cx.registry()
            .value_by_symbol(&Symbol::qualified("pitch", "ForteNamer"))
            .is_some()
    );
}
