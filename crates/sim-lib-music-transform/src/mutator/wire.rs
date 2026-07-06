use std::num::ParseIntError;

use sim_lib_music_core::Time;
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_scale::{Mode, Scale};

use super::{MutationOp, PatternMutatorError};

pub(super) fn op_wire(op: &MutationOp) -> String {
    match op {
        MutationOp::Reverse => "reverse".to_owned(),
        MutationOp::Rotate { steps } => format!("rotate:{steps}"),
        MutationOp::Transpose { semitones } => format!("transpose:{semitones}"),
        MutationOp::Invert { axis } => format!("invert:{}", axis.semitone()),
        MutationOp::ShuffleWithinBeat { beat } => format!("shuffle:{}", time_wire(*beat)),
        MutationOp::Thin { keep_percent } => format!("thin:{keep_percent}"),
        MutationOp::Thicken { semitones } => format!("thicken:{semitones}"),
        MutationOp::VelocityRemap { low, high } => format!("velocity:{low}:{high}"),
        MutationOp::RhythmDisplace { offset } => format!("rhythm:{}", time_wire(*offset)),
        MutationOp::ScaleConform { scale } => {
            format!("scale:{}:{}", scale.tonic.0, scale.mode.name())
        }
    }
}

pub(super) fn parse_op(value: &str) -> Result<MutationOp, PatternMutatorError> {
    let mut parts = value.split(':');
    match parts.next().ok_or(PatternMutatorError::InvalidWire)? {
        "reverse" => Ok(MutationOp::Reverse),
        "rotate" => Ok(MutationOp::Rotate {
            steps: parse_required(parts.next())?,
        }),
        "transpose" => Ok(MutationOp::Transpose {
            semitones: parse_required(parts.next())?,
        }),
        "invert" => Ok(MutationOp::Invert {
            axis: Pitch::from_semitone(parse_required(parts.next())?),
        }),
        "shuffle" => Ok(MutationOp::ShuffleWithinBeat {
            beat: parse_time(parts.next().ok_or(PatternMutatorError::InvalidWire)?)?,
        }),
        "thin" => Ok(MutationOp::Thin {
            keep_percent: parse_required(parts.next())?,
        }),
        "thicken" => Ok(MutationOp::Thicken {
            semitones: parse_required(parts.next())?,
        }),
        "velocity" => Ok(MutationOp::VelocityRemap {
            low: parse_required(parts.next())?,
            high: parse_required(parts.next())?,
        }),
        "rhythm" => Ok(MutationOp::RhythmDisplace {
            offset: parse_time(parts.next().ok_or(PatternMutatorError::InvalidWire)?)?,
        }),
        "scale" => {
            let tonic = parse_required(parts.next())?;
            let mode = parse_mode(parts.next().ok_or(PatternMutatorError::InvalidWire)?)?;
            let tonic = PitchClass::new(tonic)
                .map_err(|_| PatternMutatorError::InvalidPitchClass(tonic))?;
            Ok(MutationOp::ScaleConform {
                scale: Scale::new(tonic, mode),
            })
        }
        _ => Err(PatternMutatorError::InvalidWire),
    }
}

fn parse_required<T: std::str::FromStr>(value: Option<&str>) -> Result<T, PatternMutatorError>
where
    PatternMutatorError: From<<T as std::str::FromStr>::Err>,
{
    value
        .ok_or(PatternMutatorError::InvalidWire)?
        .parse()
        .map_err(PatternMutatorError::from)
}

pub(super) fn parse_number<T: std::str::FromStr>(value: &str) -> Result<T, PatternMutatorError>
where
    PatternMutatorError: From<<T as std::str::FromStr>::Err>,
{
    value.parse().map_err(PatternMutatorError::from)
}

impl From<ParseIntError> for PatternMutatorError {
    fn from(_: ParseIntError) -> Self {
        Self::InvalidNumber
    }
}

fn time_wire(time: Time) -> String {
    format!("{}/{}", time.numer(), time.denom())
}

fn parse_time(value: &str) -> Result<Time, PatternMutatorError> {
    let (numer, denom) = value
        .split_once('/')
        .ok_or(PatternMutatorError::InvalidWire)?;
    Ok(Time::new(parse_number(numer)?, parse_number(denom)?))
}

fn parse_mode(value: &str) -> Result<Mode, PatternMutatorError> {
    match value {
        "major" => Ok(Mode::Major),
        "minor-natural" => Ok(Mode::MinorNatural),
        "minor-harmonic" => Ok(Mode::MinorHarmonic),
        "minor-melodic" => Ok(Mode::MinorMelodic),
        "dorian" => Ok(Mode::Dorian),
        "phrygian" => Ok(Mode::Phrygian),
        "lydian" => Ok(Mode::Lydian),
        "mixolydian" => Ok(Mode::Mixolydian),
        "aeolian" => Ok(Mode::Aeolian),
        "locrian" => Ok(Mode::Locrian),
        "whole-tone" => Ok(Mode::WholeTone),
        "diminished" => Ok(Mode::Diminished),
        "chromatic" => Ok(Mode::Chromatic),
        _ => Err(PatternMutatorError::InvalidMode(value.to_owned())),
    }
}
