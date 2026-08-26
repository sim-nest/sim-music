use std::sync::Arc;
use std::time::Duration;

use super::*;
use sim_kernel::{DefaultFactory, EagerPolicy, Symbol};
use sim_lib_discrete_search::{SearchControl, SearchStatus};
use sim_lib_sound_core::{Amplitude, Frequency, PartialTag, Phase};

// conformance: timbre reuse covers additive, filtered, Karplus-Strong, and FM synthesis.

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
fn harmonic_and_undertone_expansions_keep_tags() {
    let harmonic =
        harmonic_expansion(3, 0.5, 0.25).render(Frequency(100.0), Duration::from_secs(1));
    assert_eq!(harmonic.partials[0].tag, PartialTag::Harmonic(1));
    assert_eq!(harmonic.partials[2].frequency, Frequency(300.0));
    assert!(harmonic.partials[1].phase.0 > harmonic.partials[0].phase.0);

    let undertone =
        undertone_expansion(3, 0.5, 0.0).render(Frequency(300.0), Duration::from_secs(1));
    assert_eq!(undertone.partials[1].tag, PartialTag::Undertone(2));
    assert_eq!(undertone.partials[1].frequency, Frequency(150.0));
}

#[test]
fn sampled_timbres_declare_pitch_policy() {
    let partials = vec![
        SampledPartial {
            ratio: 1.0,
            amplitude: Amplitude(1.0),
            phase: Phase(0.0),
            tag: PartialTag::Source,
        },
        SampledPartial {
            ratio: 2.0,
            amplitude: Amplitude(0.5),
            phase: Phase(0.5),
            tag: PartialTag::Harmonic(2),
        },
    ];
    let reject = sampled_timbre(
        Frequency(220.0),
        &partials,
        SampleInterpolation::Linear,
        SamplePitchPolicy::Reject,
    );
    assert_eq!(
        reject.try_render(Frequency(440.0), Duration::from_secs(1)),
        Err(TimbreRenderError::SamplePitchRejected)
    );

    let resample = sampled_timbre(
        Frequency(220.0),
        &partials,
        SampleInterpolation::Linear,
        SamplePitchPolicy::Resample,
    );
    let tone = resample
        .try_render(Frequency(440.0), Duration::from_secs(1))
        .expect("render");
    assert_eq!(tone.partials[0].frequency, Frequency(440.0));
    assert!(tone.partials.len() > partials.len());
}

#[test]
fn timbre_cache_is_caller_owned_and_byte_bounded() {
    let timbre = sawtooth(8);
    let mut cache = TimbreCache::new(512);
    let first = timbre
        .render_cached(Frequency(220.0), Duration::from_millis(20), &mut cache)
        .expect("first render");
    let second = timbre
        .render_cached(Frequency(220.0), Duration::from_millis(20), &mut cache)
        .expect("cached render");
    assert_eq!(first, second);
    assert!(cache.used_bytes() <= cache.max_bytes);
    assert_eq!(cache.len(), 1);
}

#[test]
fn enumerate_timbres_uses_search_control_bounds() {
    let family = TimbreFamily::new(
        "bench",
        vec![
            TimbreRecipe::PureSine,
            TimbreRecipe::Sawtooth { partials: 3 },
            TimbreRecipe::Square { partials: 3 },
        ],
    );
    let run = enumerate_timbres(&family, SearchControl::default().with_max_results(2));
    assert_eq!(run.outputs.len(), 2);
    assert_eq!(run.receipt.status, SearchStatus::Partial);
    assert_eq!(run.receipt.reason.as_deref(), Some("result bound reached"));
}

#[test]
fn layer_merge_policy_combines_coincident_partials() {
    let tone = pure_sine()
        .layer_with_policy(
            pure_sine(),
            0.25,
            MergePolicy::SumCoincidentPreferLoudestPhase,
        )
        .render(Frequency(220.0), Duration::from_millis(20));
    assert_eq!(tone.partials.len(), 1);
    assert!((tone.partials[0].amplitude.0 - 1.0).abs() < 1.0e-9);
}

#[test]
fn install_sound_timbre_lib_registers_builtin_timbres() {
    let mut cx = sim_kernel::Cx::new(
        Arc::new(EagerPolicy),
        Arc::new(DefaultFactory),
        sim_kernel::HandleSeed::new(0xd03c_8c7e_ac40_242a),
    );
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
