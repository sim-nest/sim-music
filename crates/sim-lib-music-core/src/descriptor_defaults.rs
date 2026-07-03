use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_stream_core::RateContract;

use crate::{
    DeterminismPolicy, LaneDescriptor, LaneId, LaneKind, LaneTarget, MusicCapability,
    MusicComponentCategory, MusicComponentDescriptor, MusicParamDescriptor, MusicPortDescriptor,
    MusicPortDirection, MusicUnit,
};

/// Returns the descriptor for the Scales and Chords player.
pub fn scales_chords_player_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(player_descriptor(
        "scales-chords",
        "Scales and Chords",
        vec![LaneKind::Note, LaneKind::Performance],
        vec![LaneKind::Note, LaneKind::Trace],
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "root"),
        "Root",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("C".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "scale"),
        "Scale",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("major".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "lock-policy"),
        "Scale Lock",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("quantize".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "chord-type"),
        "Chord Type",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("scale-stack".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "note-count"),
        "Note Count",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("3".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "inversion"),
        "Inversion",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "voicing"),
        "Voicing",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("closed".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "velocity"),
        "Velocity",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("preserve".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the Dual Arpeggio player.
pub fn dual_arpeggio_player_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(player_descriptor(
        "dual-arpeggio",
        "Dual Arpeggio",
        vec![LaneKind::Note, LaneKind::Midi],
        vec![LaneKind::Note, LaneKind::Control, LaneKind::Trace],
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "direction"),
        "Direction",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("up".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "gate"),
        "Gate",
        MusicUnit::Ticks,
        RateContract::midi_tick(),
        Expr::String("90".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "mode"),
        "Mode",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("parallel".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "note-order"),
        "Note Order",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("pitch-ascending".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "rate"),
        "Rate",
        MusicUnit::Beats,
        RateContract::midi_tick(),
        Expr::String("1/8".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "octaves"),
        "Octaves",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("2".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "pattern-length"),
        "Pattern Length",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("8".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "split"),
        "Split",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("midi:60".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "tie-rest"),
        "Tie/Rest Pattern",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("play".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the Arpeggio Lab player.
pub fn arpeggio_lab_player_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(player_descriptor(
        "arpeggio-lab",
        "Arpeggio Lab",
        vec![LaneKind::Note, LaneKind::Performance],
        vec![LaneKind::Note, LaneKind::Control, LaneKind::Trace],
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "anchor-role"),
        "Anchor Role",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("lowest".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "movement-pattern"),
        "Movement Pattern",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("0,1,2".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "movement-transform"),
        "Movement Transform",
        MusicUnit::Semitone,
        RateContract::control(),
        Expr::String("T0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "rate"),
        "Rate",
        MusicUnit::Beats,
        RateContract::midi_tick(),
        Expr::String("1/8".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the Note Echo player.
pub fn note_echo_player_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(player_descriptor(
        "note-echo",
        "Note Echo",
        vec![LaneKind::Note, LaneKind::Midi],
        vec![LaneKind::Note, LaneKind::Midi, LaneKind::Trace],
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "delay"),
        "Delay",
        MusicUnit::Ticks,
        RateContract::midi_tick(),
        Expr::String("120".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "feedback"),
        "Feedback",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "feedback-count"),
        "Feedback Count",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("1".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "repeats"),
        "Repeats",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("1".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "velocity-decay"),
        "Velocity Decay",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "pitch-offset"),
        "Pitch Offset",
        MusicUnit::Semitone,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "scale-snap"),
        "Scale Snap",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("off".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "channel-policy"),
        "Channel Policy",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("preserve".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the Beat Map player.
pub fn beat_map_player_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(player_descriptor(
        "beat-map",
        "Beat Map",
        vec![LaneKind::Control],
        vec![
            LaneKind::Drum,
            LaneKind::Midi,
            LaneKind::Note,
            LaneKind::Trace,
        ],
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "x"),
        "X",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("50".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "y"),
        "Y",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("50".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "density"),
        "Density",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("45".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "complexity"),
        "Complexity",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("35".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "swing"),
        "Swing",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "fill"),
        "Fill",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "mirror-lanes"),
        "Mirror Lanes",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("false".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "seed"),
        "Seed",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "lane-routing"),
        "Lane Routing",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("gm".to_owned()),
    ))
    .with_determinism(DeterminismPolicy::Seeded)
    .with_implemented(true))
}

/// Returns the descriptor for the Euclidean Drums player.
pub fn euclid_player_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(player_descriptor(
        "euclid",
        "Euclidean Drums",
        vec![LaneKind::Control],
        vec![
            LaneKind::Drum,
            LaneKind::Midi,
            LaneKind::Note,
            LaneKind::Trace,
        ],
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "pulses"),
        "Pulses",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("4".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "steps"),
        "Steps",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("16".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "rotation"),
        "Rotation",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "accent"),
        "Accent",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "lane-output"),
        "Lane Output",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("drum".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the Drum Key Map player.
pub fn drum_key_map_player_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(player_descriptor(
        "drum-key-map",
        "Drum Key Map",
        vec![LaneKind::Drum, LaneKind::Midi, LaneKind::Note],
        vec![
            LaneKind::Drum,
            LaneKind::Midi,
            LaneKind::Note,
            LaneKind::Trace,
        ],
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "kit"),
        "Kit",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("gm-standard".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "alias-policy"),
        "Alias Policy",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("gm-aliases".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "lane-routing"),
        "Lane Routing",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("preserve".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the default instrument.
pub fn default_instrument_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(MusicComponentDescriptor::new(
        Symbol::qualified("music/instrument", "default"),
        "Default Instrument",
        MusicComponentCategory::Instrument,
        RateContract::midi_tick(),
    )
    .with_capability(MusicCapability::Playable)
    .with_capability(MusicCapability::Renderable)
    .with_events(
        vec![LaneKind::Note, LaneKind::Midi, LaneKind::Control],
        vec![LaneKind::Audio],
    )
    .with_port(
        MusicPortDescriptor::new(
            Symbol::qualified("music/port", "instrument-in"),
            "Instrument In",
            MusicPortDirection::Input,
            RateContract::midi_tick(),
        )
        .with_events(vec![LaneKind::Note, LaneKind::Midi], Vec::new()),
    )
    .with_port(
        MusicPortDescriptor::new(
            Symbol::qualified("music/port", "audio-out"),
            "Audio Out",
            MusicPortDirection::Output,
            RateContract::sample_exact(Some(48_000)),
        )
        .with_events(Vec::new(), vec![LaneKind::Audio]),
    ))
}

/// Returns the descriptor for the keyboard performance source.
pub fn keyboard_performance_source_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(MusicComponentDescriptor::new(
        Symbol::qualified("music/performance-source", "keyboard"),
        "Keyboard Performance Source",
        MusicComponentCategory::Control,
        RateContract::midi_tick(),
    )
    .with_capability(MusicCapability::PerformanceSource)
    .with_capability(MusicCapability::Playable)
    .with_events(Vec::new(), vec![LaneKind::Performance, LaneKind::Note])
    .with_determinism(DeterminismPolicy::LiveInput))
}

/// Returns the descriptor for the Tempo LFO modulator.
pub fn tempo_lfo_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(MusicComponentDescriptor::new(
        Symbol::qualified("music/modulator", "tempo-lfo"),
        "Tempo LFO",
        MusicComponentCategory::Control,
        RateContract::control(),
    )
    .with_capability(MusicCapability::Modulator)
    .with_capability(MusicCapability::Oscillator)
    .with_events(Vec::new(), vec![LaneKind::Control])
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "frequency"),
        "Frequency",
        MusicUnit::Hertz,
        RateContract::control(),
        Expr::String("1".to_owned()),
    ))
    .with_implemented(true))
}

fn player_descriptor(
    name: &'static str,
    label: &'static str,
    accepted: Vec<LaneKind>,
    output: Vec<LaneKind>,
) -> Result<MusicComponentDescriptor> {
    Ok(MusicComponentDescriptor::new(
        Symbol::qualified("music/player-family", name),
        label,
        MusicComponentCategory::PlayerFamily,
        RateContract::midi_tick(),
    )
    .with_capability(MusicCapability::Player)
    .with_capability(MusicCapability::Playable)
    .with_capability(MusicCapability::Renderable)
    .with_events(accepted.clone(), output.clone())
    .with_port(
        MusicPortDescriptor::new(
            Symbol::qualified("music/port", format!("{name}-in")),
            "Input",
            MusicPortDirection::Input,
            RateContract::midi_tick(),
        )
        .with_events(accepted, Vec::new()),
    )
    .with_port(
        MusicPortDescriptor::new(
            Symbol::qualified("music/port", format!("{name}-out")),
            "Output",
            MusicPortDirection::Output,
            RateContract::midi_tick(),
        )
        .with_events(Vec::new(), output),
    )
    .with_lane(
        LaneDescriptor::new(
            LaneId::new(format!("{name}-output")),
            LaneKind::Note,
            LaneTarget::Instrument(Symbol::qualified("music/target", "default")),
            0,
        )
        .map_err(|err| Error::Eval(err.to_string()))?,
    )
    .with_implemented(false))
}
