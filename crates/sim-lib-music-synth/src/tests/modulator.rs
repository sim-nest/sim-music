use sim_kernel::Symbol;
use sim_lib_music_core::{
    Channel, LaneId, NoteEvent, Pitch, PlayContext, PlayEvent, Playable, TimeRange,
    default_music_component_registry,
};

use crate::{
    AdsrSettings, AutomationCurve, AutomationPoint, LfoSettings, ModulationChain,
    ModulationOperator, ModulationRate, ModulationTargetPath, ModulatorConfig, ModulatorPlayable,
    ModulatorSource, OscillatorKind, RandomWalkSettings, default_audio_synth_registry,
};

fn cx(start: i64, end: i64) -> PlayContext {
    PlayContext::new(TimeRange::from_ticks(start, end, 480).expect("range"))
}

fn note_at(ticks: i64, midi: u8) -> PlayEvent {
    PlayEvent::Note(NoteEvent {
        lane_id: LaneId::new("notes"),
        time: sim_lib_music_core::Tick { ticks, tpq: 480 },
        duration: sim_lib_music_core::Tick {
            ticks: 120,
            tpq: 480,
        },
        pitch: Pitch::from_midi(midi),
        velocity: 96,
        channel: Channel::new(0).expect("channel"),
    })
}

fn source_config(source: ModulatorSource, rate: ModulationRate) -> ModulatorConfig {
    ModulatorConfig::new(
        Symbol::qualified("test/modulator", rate.as_str()),
        source,
        ModulationTargetPath::control("amount"),
        rate,
    )
}

#[test]
fn modulator_playable_reports_descriptor_stream_and_freeze() {
    for (source, rate) in [
        (
            ModulatorSource::Lfo(LfoSettings {
                depth: 0.5,
                rate_hz: 1.0,
                ..LfoSettings::default()
            }),
            ModulationRate::Control,
        ),
        (
            ModulatorSource::Envelope(AdsrSettings::default()),
            ModulationRate::Tick,
        ),
        (
            ModulatorSource::Oscillator {
                kind: OscillatorKind::Sine,
                frequency_hz: 4.0,
                amplitude: 0.75,
            },
            ModulationRate::Audio,
        ),
        (
            ModulatorSource::RandomWalk(RandomWalkSettings::default()),
            ModulationRate::Step,
        ),
        (
            ModulatorSource::AutomationCurve(AutomationCurve::new(vec![
                AutomationPoint::new(0, 0.0),
                AutomationPoint::new(480, 1.0),
            ])),
            ModulationRate::PerNote,
        ),
    ] {
        let playable = ModulatorPlayable::new(source_config(source, rate));
        let mut context = cx(0, 480);
        context.upstream = vec![note_at(120, 64)];

        let descriptor = playable.describe().expect("descriptor");
        assert_eq!(descriptor.clock_domain, rate.clock_domain());
        assert_eq!(descriptor.lanes.len(), 1);

        let stream = playable.render_range(&context).expect("stream");
        assert_eq!(stream.metadata().clock(), &rate.clock_domain().symbol());

        let frozen = playable.freeze(&context).expect("freeze");
        assert_eq!(frozen.descriptor.clock_domain, rate.clock_domain());
        assert!(!frozen.events.is_empty());
    }
}

#[test]
fn modulation_rates_convert_to_expected_tick_outputs() {
    let source = ModulatorSource::AutomationCurve(AutomationCurve::new(vec![
        AutomationPoint::new(0, 0.0),
        AutomationPoint::new(480, 1.0),
    ]));
    let context = cx(0, 480);
    let counts = [
        (ModulationRate::Audio, 480),
        (ModulationRate::Control, 24),
        (ModulationRate::Tick, 480),
        (ModulationRate::Step, 4),
    ];

    for (rate, expected) in counts {
        let playable = ModulatorPlayable::new(source_config(source.clone(), rate));
        assert_eq!(
            playable.render_samples(&context).len(),
            expected,
            "{rate:?}"
        );
    }

    let mut per_note = context.clone();
    per_note.upstream = vec![note_at(0, 60), note_at(240, 67), note_at(480, 72)];
    let playable = ModulatorPlayable::new(source_config(source, ModulationRate::PerNote));
    let ticks = playable
        .render_samples(&per_note)
        .into_iter()
        .map(|sample| sample.tick.ticks)
        .collect::<Vec<_>>();
    assert_eq!(ticks, vec![0, 240]);
}

#[test]
fn modulation_targets_resolve_player_and_instrument_parameters() {
    let music_registry = default_music_component_registry();
    let synth_registry = default_audio_synth_registry();

    assert!(
        ModulationTargetPath::player_parameter("amount").resolves_player_parameter(&music_registry)
    );
    assert!(
        ModulationTargetPath::instrument_parameter("amp-gain")
            .resolves_instrument_parameter(&synth_registry)
    );
    assert!(!ModulationTargetPath::control("free").resolves_player_parameter(&music_registry));
}

#[test]
fn random_walk_output_is_seeded_and_bounded() {
    let source = ModulatorSource::RandomWalk(RandomWalkSettings {
        start: 0.0,
        step: 0.25,
        min: -0.5,
        max: 0.5,
    });
    let context = cx(0, 120);

    let first =
        ModulatorPlayable::new(source_config(source.clone(), ModulationRate::Tick).with_seed(7))
            .render_samples(&context);
    let second =
        ModulatorPlayable::new(source_config(source.clone(), ModulationRate::Tick).with_seed(7))
            .render_samples(&context);
    let third = ModulatorPlayable::new(source_config(source, ModulationRate::Tick).with_seed(8))
        .render_samples(&context);

    assert_eq!(first, second);
    assert_ne!(first, third);
    assert!(first.iter().all(|sample| sample.value >= -0.5));
    assert!(first.iter().all(|sample| sample.value <= 0.5));
}

#[test]
fn modulation_chain_composes_math_and_time_operators() {
    let source = ModulatorConfig::new(
        Symbol::qualified("test/modulator", "chain"),
        ModulatorSource::AutomationCurve(AutomationCurve::new(vec![
            AutomationPoint::new(0, 0.0),
            AutomationPoint::new(1, 1.0),
            AutomationPoint::new(2, 0.0),
            AutomationPoint::new(3, 1.0),
        ])),
        ModulationTargetPath::control("chain"),
        ModulationRate::Tick,
    );
    let chain = ModulationChain::new(
        source,
        vec![
            ModulationOperator::Sum(0.25),
            ModulationOperator::Multiply(2.0),
            ModulationOperator::SampleHold { samples: 2 },
            ModulationOperator::Quantize { step: 0.5 },
            ModulationOperator::Smooth { amount: 1.0 },
            ModulationOperator::Clip { min: 0.0, max: 1.0 },
            ModulationOperator::Lag { step: 0.5 },
        ],
    );

    let values = chain
        .render_samples(&cx(0, 4))
        .into_iter()
        .map(|sample| sample.value)
        .collect::<Vec<_>>();

    assert_eq!(values, vec![0.5, 0.5, 0.5, 0.5]);
}

#[test]
fn modulator_can_target_another_players_parameter() {
    let target = ModulationTargetPath::player_parameter("amount");
    let playable = ModulatorPlayable::new(ModulatorConfig::new(
        Symbol::qualified("test/modulator", "player-param"),
        ModulatorSource::AutomationCurve(AutomationCurve::new(vec![
            AutomationPoint::new(0, 0.25),
            AutomationPoint::new(20, 0.5),
        ])),
        target.clone(),
        ModulationRate::Control,
    ));
    let frozen = playable.freeze(&cx(0, 40)).expect("freeze");

    assert!(target.resolves_player_parameter(&default_music_component_registry()));
    assert!(frozen.events.iter().all(|event| match event {
        PlayEvent::Control(control) => control.control == target.control_symbol(),
        _ => false,
    }));
}
