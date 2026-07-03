use std::sync::Arc;
use std::time::Duration;

use super::*;
use sim_kernel::{DefaultFactory, EagerPolicy, Symbol};
use sim_lib_sound_core::Frequency;

#[test]
fn builtins_render_non_empty_tones() {
    let builtins = vec![
        pure_sine(),
        sawtooth(6),
        square(6),
        triangle(6),
        organ_pipe(&[1.0, 2.0, 3.0]),
        karplus_strong(0.8),
        fm_pair(2.0, 1.5),
        bell_inharmonic(&[1.0, 2.7, 5.8]),
    ];
    for timbre in builtins {
        let tone = timbre.render(Frequency(220.0), Duration::from_secs(1));
        assert!(!tone.partials.is_empty());
    }
}

#[test]
fn filters_change_partial_amplitude() {
    let timbre = sawtooth(4).with_filter(Filter::LowPass {
        cutoff: Frequency(300.0),
        q: 0.7,
    });
    let tone = timbre.render(Frequency(220.0), Duration::from_secs(1));
    assert!(tone.partials[1].amplitude.0 < 0.5);
}

#[test]
fn install_sound_timbre_lib_registers_builtin_timbres() {
    let mut cx = sim_kernel::Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_sound_timbre_lib(&mut cx).expect("install");
    install_sound_timbre_lib(&mut cx).expect("install");
    assert!(
        cx.resolve_value(&Symbol::qualified("sound", "PureSine"))
            .is_ok()
    );
    assert!(
        cx.resolve_value(&Symbol::qualified("sound", "TimbreRegistry"))
            .is_ok()
    );
}
