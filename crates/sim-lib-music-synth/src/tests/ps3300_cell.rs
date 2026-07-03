use std::f32::consts::TAU;

use sim_kernel::Symbol;

use crate::{
    ComponentCapability, ComponentRegistryCategory, InstrumentWrapperCategory,
    default_audio_synth_registry,
    ps3300::{
        PS3300_KEY_COUNT, Ps3300NoteCell, Ps3300NoteCellSettings, Ps3300PerNoteEnvelopeSettings,
        Ps3300PerNoteVcaSettings, Ps3300PerNoteVcfSettings, Ps3300PolyArray,
        Ps3300PolyArraySettings, Ps3300ResonatorBandSettings, Ps3300ResonatorMode,
        Ps3300TripleResonator, Ps3300TripleResonatorSettings, ps3_per_key_cell_component_id,
        ps3_poly_array_component_id, ps3_resonator_component_id, ps3300_voice_cell_fixture_names,
        ps3300_voice_cell_module_ids,
    },
};

#[test]
fn ps3300_voice_cell_ids_and_fixture_names_are_recorded() {
    assert_eq!(
        ps3300_voice_cell_module_ids()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/module/ps3-per-key-cell",
            "audio-synth/module/ps3-poly-array",
            "audio-synth/module/ps3-resonator-bank",
        ]
    );
    assert_eq!(
        ps3300_voice_cell_fixture_names(),
        [
            "ps3300-ps3-per-key-cell-vcf-envelope-vca",
            "ps3300-ps3-poly-array-chord-cell-count",
            "ps3300-ps3-poly-array-gate-isolation",
            "ps3300-ps3-resonator-peaks",
            "ps3300-ps3-resonator-formant-sweep",
        ]
    );
}

#[test]
fn ps3300_voice_cell_registry_entries_are_exact_components() {
    let registry = default_audio_synth_registry();
    for id in [
        ps3_per_key_cell_component_id(),
        ps3_poly_array_component_id(),
        ps3_resonator_component_id(),
    ] {
        let entry = registry.get(&id).expect("PS-3300 voice-cell entry");
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert_eq!(entry.wrapper(), InstrumentWrapperCategory::FixedPolysynth);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));
        assert_eq!(entry.instantiate().unwrap().component_id(), id);
    }
}

#[test]
fn ps3300_single_note_cell_runs_vcf_envelope_and_vca() {
    let mut cell = Ps3300NoteCell::new(Ps3300NoteCellSettings {
        midi_key: 60,
        vcf: Ps3300PerNoteVcfSettings {
            cutoff_hz: 900.0,
            resonance: 0.4,
            keyboard_tracking_octaves: 0.5,
            envelope_depth_octaves: 1.0,
        },
        envelope: Ps3300PerNoteEnvelopeSettings {
            attack_s: 0.01,
            decay_s: 0.05,
            sustain: 0.5,
            release_s: 0.05,
        },
        vca: Ps3300PerNoteVcaSettings {
            level: 1.0,
            response_curve: 1.0,
        },
    });
    cell.set_sample_rate(1_000.0);

    let gated = (0..32)
        .map(|_| cell.next_sample(1.0, 0.0, true))
        .collect::<Vec<_>>();
    assert!(gated[0].envelope > 0.0);
    assert!(gated[31].envelope > gated[0].envelope);
    assert!(gated.iter().any(|frame| frame.filtered.abs() > 0.1));
    assert!(gated.iter().any(|frame| frame.output.abs() > 0.05));

    let release = (0..32)
        .map(|_| cell.next_sample(1.0, 0.0, false))
        .collect::<Vec<_>>();
    assert!(release.last().unwrap().envelope < gated.last().unwrap().envelope);
}

#[test]
fn ps3300_poly_array_counts_chord_cells_and_isolates_gates() {
    let mut array = Ps3300PolyArray::new(Ps3300PolyArraySettings {
        section_level: 1.0,
        first_midi_key: 36,
        key_count: PS3300_KEY_COUNT,
    });
    array.set_sample_rate(1_000.0);
    assert_eq!(array.cell_count(), PS3300_KEY_COUNT);
    assert_eq!(
        Ps3300PolyArray::new(Ps3300PolyArraySettings {
            section_level: 1.0,
            first_midi_key: 127,
            key_count: PS3300_KEY_COUNT,
        })
        .cell_count(),
        1
    );

    let chord = [48, 52, 55, 60];
    for _ in 0..24 {
        array.next_chord(0.8, &chord);
    }
    let frame = array.next_chord(0.8, &chord);
    assert_eq!(frame.cell_count, PS3300_KEY_COUNT);
    assert_eq!(frame.active_count, chord.len());
    assert!(frame.mixed.abs() > 0.0);

    let active_outputs = frame
        .cell_outputs
        .iter()
        .filter(|(key, output)| chord.contains(key) && output.abs() > 0.0001)
        .count();
    let inactive_leaks = frame
        .cell_outputs
        .iter()
        .filter(|(key, output)| !chord.contains(key) && output.abs() > 0.0001)
        .count();
    assert_eq!(active_outputs, chord.len());
    assert_eq!(inactive_leaks, 0);
}

#[test]
fn ps3300_triple_resonator_emphasizes_recorded_peaks() {
    let settings = resonator_settings(Ps3300ResonatorMode::Parallel);
    let sample_rate = 48_000.0;
    let low = resonator_energy(settings, 720.0, sample_rate);
    let off = resonator_energy(settings, 360.0, sample_rate);
    let high = resonator_energy(settings, 2_880.0, sample_rate);

    assert!(low > off * 2.0, "low peak {low} off {off}");
    assert!(high > off * 1.4, "high peak {high} off {off}");
}

#[test]
fn ps3300_resonator_formant_sweep_is_deterministic() {
    let settings = resonator_settings(Ps3300ResonatorMode::Series);
    let mut first = Ps3300TripleResonator::new(settings);
    let mut second = Ps3300TripleResonator::new(settings);
    first.set_sample_rate(48_000.0);
    second.set_sample_rate(48_000.0);

    let cv = [-0.25, 0.0, 0.25, 0.5, 0.25, 0.0];
    let first_run = cv
        .iter()
        .enumerate()
        .map(|(index, cv)| first.next_sample(sine_sample(index, 720.0, 48_000.0), *cv))
        .collect::<Vec<_>>();
    let second_run = cv
        .iter()
        .enumerate()
        .map(|(index, cv)| second.next_sample(sine_sample(index, 720.0, 48_000.0), *cv))
        .collect::<Vec<_>>();

    assert_eq!(
        round_outputs(&first_run),
        round_outputs(&second_run),
        "formant sweep should be repeatable"
    );
    assert!(first_run[3].centers_hz[0] > first_run[1].centers_hz[0]);
}

fn resonator_settings(mode: Ps3300ResonatorMode) -> Ps3300TripleResonatorSettings {
    Ps3300TripleResonatorSettings {
        mode,
        bands: [
            Ps3300ResonatorBandSettings::new(720.0, 10.0, 1.0),
            Ps3300ResonatorBandSettings::new(1_440.0, 10.0, 0.9),
            Ps3300ResonatorBandSettings::new(2_880.0, 10.0, 0.8),
        ],
        cv_depth_octaves: 1.0,
        level: 1.0,
    }
}

fn resonator_energy(
    settings: Ps3300TripleResonatorSettings,
    frequency_hz: f32,
    sample_rate_hz: f32,
) -> f32 {
    let mut resonator = Ps3300TripleResonator::new(settings);
    resonator.set_sample_rate(sample_rate_hz);
    let mut energy = 0.0;
    for index in 0..4_096 {
        let output = resonator.next_sample(sine_sample(index, frequency_hz, sample_rate_hz), 0.0);
        if index > 512 {
            energy += output.output * output.output;
        }
    }
    energy / 3_584.0
}

fn sine_sample(index: usize, frequency_hz: f32, sample_rate_hz: f32) -> f32 {
    (TAU * frequency_hz * index as f32 / sample_rate_hz).sin()
}

fn round_outputs(frames: &[crate::ps3300::Ps3300ResonatorFrame]) -> Vec<(i32, i32, i32, i32)> {
    frames
        .iter()
        .map(|frame| {
            (
                (frame.output * 100_000.0).round() as i32,
                (frame.centers_hz[0] * 10.0).round() as i32,
                (frame.centers_hz[1] * 10.0).round() as i32,
                (frame.centers_hz[2] * 10.0).round() as i32,
            )
        })
        .collect()
}
