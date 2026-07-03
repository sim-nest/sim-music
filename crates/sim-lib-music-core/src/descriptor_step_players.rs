use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_stream_core::RateContract;

use crate::{
    DeterminismPolicy, LaneDescriptor, LaneId, LaneKind, LaneTarget, MusicCapability,
    MusicComponentCategory, MusicComponentDescriptor, MusicParamDescriptor, MusicPortDescriptor,
    MusicPortDirection, MusicUnit,
};

/// Returns the descriptor for the Chord Sequencer player.
pub fn chord_sequencer_player_descriptor() -> Result<MusicComponentDescriptor> {
    let descriptor = player_descriptor(
        "chord-sequencer",
        "Chord Sequencer",
        vec![LaneKind::Note, LaneKind::Control, LaneKind::Performance],
        vec![
            LaneKind::Note,
            LaneKind::Playable,
            LaneKind::Control,
            LaneKind::Trace,
        ],
    )?;
    Ok(with_player_params(
        descriptor,
        [
            (
                "key",
                "Key",
                MusicUnit::None,
                RateContract::control(),
                "C major",
            ),
            (
                "progression-slots",
                "Progression Slots",
                MusicUnit::None,
                RateContract::control(),
                "[]",
            ),
            (
                "slot-duration",
                "Slot Duration",
                MusicUnit::Ticks,
                RateContract::control(),
                "480",
            ),
            (
                "trigger-mode",
                "Trigger Mode",
                MusicUnit::None,
                RateContract::control(),
                "explicit-or-scale-degree",
            ),
            (
                "voicing",
                "Voicing",
                MusicUnit::None,
                RateContract::control(),
                "closed",
            ),
            (
                "suggestion-count",
                "Suggestion Count",
                MusicUnit::None,
                RateContract::control(),
                "6",
            ),
        ],
    )
    .with_determinism(DeterminismPolicy::Deterministic)
    .with_implemented(true))
}

/// Returns the descriptor for the Bassline Generator player.
pub fn bassline_player_descriptor() -> Result<MusicComponentDescriptor> {
    let descriptor = player_descriptor(
        "bassline-generator",
        "Bassline Generator",
        vec![LaneKind::Note, LaneKind::Playable, LaneKind::Control],
        vec![LaneKind::Note, LaneKind::Control, LaneKind::Trace],
    )?;
    Ok(with_player_params(
        descriptor,
        [
            ("key", "Key", MusicUnit::None, RateContract::control(), "C"),
            (
                "scale",
                "Scale",
                MusicUnit::None,
                RateContract::control(),
                "major",
            ),
            (
                "chord-follow",
                "Chord Follow",
                MusicUnit::None,
                RateContract::control(),
                "enabled",
            ),
            (
                "density",
                "Density",
                MusicUnit::Percent,
                RateContract::control(),
                "65",
            ),
            (
                "octave-range",
                "Octave Range",
                MusicUnit::None,
                RateContract::control(),
                "2..3",
            ),
            (
                "note-length",
                "Note Length",
                MusicUnit::Ticks,
                RateContract::midi_tick(),
                "90",
            ),
            (
                "accent",
                "Accent",
                MusicUnit::Percent,
                RateContract::control(),
                "24",
            ),
            (
                "slide",
                "Slide",
                MusicUnit::Percent,
                RateContract::control(),
                "0",
            ),
            (
                "ghost-notes",
                "Ghost Notes",
                MusicUnit::Percent,
                RateContract::control(),
                "0",
            ),
            (
                "seed",
                "Seed",
                MusicUnit::None,
                RateContract::control(),
                "0",
            ),
        ],
    )
    .with_determinism(DeterminismPolicy::Seeded)
    .with_implemented(true))
}

/// Returns the descriptor for the PolyStep Sequencer player.
pub fn polystep_player_descriptor() -> Result<MusicComponentDescriptor> {
    let descriptor = player_descriptor(
        "polystep",
        "PolyStep Sequencer",
        vec![LaneKind::Note, LaneKind::Performance, LaneKind::Control],
        vec![LaneKind::Note, LaneKind::Control, LaneKind::Trace],
    )?;
    Ok(with_player_params(
        descriptor,
        [
            (
                "lane-count",
                "Lane Count",
                MusicUnit::None,
                RateContract::control(),
                "2",
            ),
            (
                "lane-length",
                "Lane Length",
                MusicUnit::None,
                RateContract::control(),
                "16",
            ),
            (
                "direction",
                "Direction",
                MusicUnit::None,
                RateContract::control(),
                "forward",
            ),
            (
                "step-program",
                "Step Program",
                MusicUnit::None,
                RateContract::control(),
                "[]",
            ),
            (
                "gate",
                "Gate",
                MusicUnit::Ticks,
                RateContract::midi_tick(),
                "90",
            ),
            (
                "velocity",
                "Velocity",
                MusicUnit::Percent,
                RateContract::control(),
                "96",
            ),
            (
                "probability",
                "Probability",
                MusicUnit::Percent,
                RateContract::control(),
                "100",
            ),
            (
                "ratchet",
                "Ratchet",
                MusicUnit::None,
                RateContract::control(),
                "1",
            ),
            (
                "tie",
                "Tie",
                MusicUnit::None,
                RateContract::control(),
                "false",
            ),
            (
                "slide",
                "Slide",
                MusicUnit::None,
                RateContract::control(),
                "false",
            ),
            (
                "step-record",
                "Step Record",
                MusicUnit::None,
                RateContract::control(),
                "off",
            ),
            (
                "target-routing",
                "Target Routing",
                MusicUnit::None,
                RateContract::control(),
                "per-lane",
            ),
            (
                "seed",
                "Seed",
                MusicUnit::None,
                RateContract::control(),
                "0",
            ),
        ],
    )
    .with_determinism(DeterminismPolicy::Seeded)
    .with_implemented(true))
}

/// Returns the descriptor for the Quad Note Generator player.
pub fn quad_note_player_descriptor() -> Result<MusicComponentDescriptor> {
    let descriptor = player_descriptor(
        "quad-note-generator",
        "Quad Note Generator",
        vec![LaneKind::Note, LaneKind::Playable, LaneKind::Control],
        vec![LaneKind::Note, LaneKind::Control, LaneKind::Trace],
    )?;
    Ok(with_player_params(
        descriptor,
        [
            (
                "stream-count",
                "Stream Count",
                MusicUnit::None,
                RateContract::control(),
                "4",
            ),
            (
                "scale-lock",
                "Scale Lock",
                MusicUnit::None,
                RateContract::control(),
                "on",
            ),
            (
                "pitch-range",
                "Pitch Range",
                MusicUnit::None,
                RateContract::control(),
                "C3..C6",
            ),
            (
                "density",
                "Density",
                MusicUnit::Percent,
                RateContract::control(),
                "50",
            ),
            (
                "rhythm-distribution",
                "Rhythm Distribution",
                MusicUnit::None,
                RateContract::control(),
                "even",
            ),
            (
                "velocity-range",
                "Velocity Range",
                MusicUnit::None,
                RateContract::control(),
                "72..104",
            ),
            (
                "stream-seed",
                "Stream Seed",
                MusicUnit::None,
                RateContract::control(),
                "0",
            ),
            ("key", "Key", MusicUnit::None, RateContract::control(), "C"),
            (
                "scale",
                "Scale",
                MusicUnit::None,
                RateContract::control(),
                "major",
            ),
            (
                "harmonic-relation",
                "Harmonic Relation",
                MusicUnit::None,
                RateContract::control(),
                "thirds",
            ),
            (
                "master-seed",
                "Master Seed",
                MusicUnit::None,
                RateContract::control(),
                "0",
            ),
        ],
    )
    .with_determinism(DeterminismPolicy::Seeded)
    .with_implemented(true))
}

/// Returns the descriptor for the Pattern Mutator player.
pub fn pattern_mutator_player_descriptor() -> Result<MusicComponentDescriptor> {
    let descriptor = player_descriptor(
        "pattern-mutator",
        "Pattern Mutator",
        vec![LaneKind::Note, LaneKind::Playable, LaneKind::Control],
        vec![LaneKind::Note, LaneKind::Control, LaneKind::Trace],
    )?;
    Ok(with_player_params(
        descriptor,
        [
            (
                "mutation-ops",
                "Mutation Ops",
                MusicUnit::None,
                RateContract::control(),
                "reverse,rotate,transpose,invert,shuffle,thin,thicken,velocity,rhythm,scale",
            ),
            (
                "amount",
                "Amount",
                MusicUnit::Percent,
                RateContract::control(),
                "100",
            ),
            (
                "seed",
                "Seed",
                MusicUnit::None,
                RateContract::control(),
                "0",
            ),
            (
                "lock-set",
                "Lock Set",
                MusicUnit::None,
                RateContract::control(),
                "[]",
            ),
            (
                "scale-conform",
                "Scale Conform",
                MusicUnit::None,
                RateContract::control(),
                "C major",
            ),
        ],
    )
    .with_determinism(DeterminismPolicy::Seeded)
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

fn player_param(
    name: &'static str,
    label: &'static str,
    unit: MusicUnit,
    rate: RateContract,
    default: &'static str,
) -> MusicParamDescriptor {
    MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", name),
        label,
        unit,
        rate,
        Expr::String(default.to_owned()),
    )
}

fn with_player_params<const N: usize>(
    mut descriptor: MusicComponentDescriptor,
    params: [(
        &'static str,
        &'static str,
        MusicUnit,
        RateContract,
        &'static str,
    ); N],
) -> MusicComponentDescriptor {
    for (name, label, unit, rate, default) in params {
        descriptor = descriptor.with_param(player_param(name, label, unit, rate, default));
    }
    descriptor
}
