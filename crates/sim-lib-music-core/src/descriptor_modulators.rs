use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_stream_core::RateContract;

use crate::{
    DeterminismPolicy, LaneDescriptor, LaneId, LaneKind, LaneTarget, MusicCapability,
    MusicComponentCategory, MusicComponentDescriptor, MusicParamDescriptor, MusicPortDescriptor,
    MusicPortDirection, MusicUnit,
};

/// Returns the descriptor for the LFO modulator.
pub fn lfo_modulator_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(modulator_descriptor(
        "lfo",
        "LFO Modulator",
        RateContract::control(),
        DeterminismPolicy::Deterministic,
    )?
    .with_capability(MusicCapability::Oscillator)
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "frequency"),
        "Frequency",
        MusicUnit::Hertz,
        RateContract::control(),
        Expr::String("1".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "shape"),
        "Shape",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("sine".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "target"),
        "Target",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("music/control/modulation".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the envelope modulator.
pub fn envelope_modulator_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(modulator_descriptor(
        "envelope",
        "Envelope Modulator",
        RateContract::midi_tick(),
        DeterminismPolicy::Deterministic,
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "attack"),
        "Attack",
        MusicUnit::Beats,
        RateContract::midi_tick(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "sustain"),
        "Sustain",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("1".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "target"),
        "Target",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("music/control/modulation".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the oscillator modulator.
pub fn oscillator_modulator_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(modulator_descriptor(
        "oscillator",
        "Oscillator Modulator",
        RateContract::sample_exact(Some(48_000)),
        DeterminismPolicy::Deterministic,
    )?
    .with_capability(MusicCapability::Oscillator)
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "frequency"),
        "Frequency",
        MusicUnit::Hertz,
        RateContract::control(),
        Expr::String("440".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "amplitude"),
        "Amplitude",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("1".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "target"),
        "Target",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("music/control/modulation".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the random walk modulator.
pub fn random_walk_modulator_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(modulator_descriptor(
        "random-walk",
        "Random Walk Modulator",
        RateContract::midi_tick(),
        DeterminismPolicy::Seeded,
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "seed"),
        "Seed",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("0".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "step"),
        "Step",
        MusicUnit::Percent,
        RateContract::control(),
        Expr::String("0.1".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "target"),
        "Target",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("music/control/modulation".to_owned()),
    ))
    .with_implemented(true))
}

/// Returns the descriptor for the automation curve modulator.
pub fn automation_curve_modulator_descriptor() -> Result<MusicComponentDescriptor> {
    Ok(modulator_descriptor(
        "automation-curve",
        "Automation Curve Modulator",
        RateContract::midi_tick(),
        DeterminismPolicy::Deterministic,
    )?
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "curve"),
        "Curve",
        MusicUnit::None,
        RateContract::midi_tick(),
        Expr::String("0:0,480:1".to_owned()),
    ))
    .with_param(MusicParamDescriptor::new(
        Symbol::qualified("music/player-param", "target"),
        "Target",
        MusicUnit::None,
        RateContract::control(),
        Expr::String("music/control/modulation".to_owned()),
    ))
    .with_implemented(true))
}

fn modulator_descriptor(
    name: &'static str,
    label: &'static str,
    rate: RateContract,
    determinism: DeterminismPolicy,
) -> Result<MusicComponentDescriptor> {
    Ok(MusicComponentDescriptor::new(
        Symbol::qualified("music/modulator", name),
        label,
        MusicComponentCategory::Control,
        rate,
    )
    .with_capability(MusicCapability::Modulator)
    .with_capability(MusicCapability::Playable)
    .with_capability(MusicCapability::Renderable)
    .with_events(Vec::new(), vec![LaneKind::Control])
    .with_determinism(determinism)
    .with_port(
        MusicPortDescriptor::new(
            Symbol::qualified("music/port", format!("{name}-control-out")),
            "Control Out",
            MusicPortDirection::Output,
            rate,
        )
        .with_events(Vec::new(), vec![LaneKind::Control]),
    )
    .with_lane(
        LaneDescriptor::new(
            LaneId::new(format!("{name}-control")),
            LaneKind::Control,
            LaneTarget::Control(Symbol::qualified("music/control", name)),
            0,
        )
        .map_err(|err| Error::Eval(err.to_string()))?,
    ))
}
