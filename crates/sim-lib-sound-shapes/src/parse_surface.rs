use sim_lib_sound_bridge::{BridgeOptions, TimbreBank};
use sim_lib_sound_dissonance::DissonanceModelDescriptor;
use sim_lib_sound_render::RendererOptions;
use sim_lib_sound_tuning::{PitchClassN, TuningDescriptor};

use crate::SoundShapeError;
use crate::decode_timbre;
use crate::parse::{atom_text, field_atom, field_form_text, field_list, parse_f64, parse_node};

/// Decodes a [`PitchClassN`] from its sound-shape text form.
pub fn decode_pitch_class_n(value: &str) -> Result<PitchClassN, SoundShapeError> {
    let node = parse_node(value)?;
    PitchClassN::new(
        field_atom(&node, "divisions")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        field_atom(&node, "index")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
    )
    .map_err(|_| SoundShapeError::InvalidSoundShape)
}

/// Decodes a [`TuningDescriptor`] from its sound-shape text form.
pub fn decode_tuning_descriptor(value: &str) -> Result<TuningDescriptor, SoundShapeError> {
    let node = parse_node(value)?;
    let descriptor = match field_atom(&node, "kind")?.as_str() {
        "EqualTemperament" => TuningDescriptor::EqualTemperament {
            divisions: field_atom(&node, "divisions")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
            reference_midi: field_atom(&node, "reference_midi")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
            reference_hz: parse_f64(&field_atom(&node, "reference_hz")?)?,
        },
        "JustIntonation" => {
            let ratios = field_list(&node, "ratios")?
                .iter()
                .map(atom_text)
                .map(|value| parse_f64(&value))
                .collect::<Result<Vec<_>, _>>()?;
            let ratios: [f64; 12] = ratios
                .try_into()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?;
            TuningDescriptor::JustIntonation {
                root: field_atom(&node, "root")?
                    .parse()
                    .map_err(|_| SoundShapeError::InvalidSoundShape)?,
                ratios,
                reference_midi: field_atom(&node, "reference_midi")?
                    .parse()
                    .map_err(|_| SoundShapeError::InvalidSoundShape)?,
                reference_hz: parse_f64(&field_atom(&node, "reference_hz")?)?,
            }
        }
        "PythagoreanTuning" => TuningDescriptor::PythagoreanTuning {
            reference_midi: field_atom(&node, "reference_midi")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
            reference_hz: parse_f64(&field_atom(&node, "reference_hz")?)?,
        },
        "MeantoneQuarterComma" => TuningDescriptor::MeantoneQuarterComma {
            reference_midi: field_atom(&node, "reference_midi")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
            reference_hz: parse_f64(&field_atom(&node, "reference_hz")?)?,
        },
        "WerckmeisterIII" => TuningDescriptor::WerckmeisterIII {
            reference_midi: field_atom(&node, "reference_midi")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
            reference_hz: parse_f64(&field_atom(&node, "reference_hz")?)?,
        },
        "YoungTemperament" => TuningDescriptor::YoungTemperament {
            reference_midi: field_atom(&node, "reference_midi")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
            reference_hz: parse_f64(&field_atom(&node, "reference_hz")?)?,
        },
        "ScalaScl" => TuningDescriptor::ScalaScl {
            cents: field_list(&node, "cents")?
                .iter()
                .map(atom_text)
                .map(|value| parse_f64(&value))
                .collect::<Result<Vec<_>, _>>()?,
            reference_midi: field_atom(&node, "reference_midi")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
            reference_hz: parse_f64(&field_atom(&node, "reference_hz")?)?,
        },
        _ => return Err(SoundShapeError::InvalidSoundShape),
    };
    descriptor
        .to_tuning()
        .map_err(|_| SoundShapeError::InvalidSoundShape)?;
    Ok(descriptor)
}

/// Decodes a [`DissonanceModelDescriptor`] from its sound-shape text form.
pub fn decode_dissonance_model_descriptor(
    value: &str,
) -> Result<DissonanceModelDescriptor, SoundShapeError> {
    let node = parse_node(value)?;
    match field_atom(&node, "kind")?.as_str() {
        "PlompLevelt" => Ok(DissonanceModelDescriptor::PlompLevelt),
        "Sethares" => Ok(DissonanceModelDescriptor::Sethares),
        "HelmholtzBeating" => Ok(DissonanceModelDescriptor::HelmholtzBeating),
        "HarmonicEntropy" => Ok(DissonanceModelDescriptor::HarmonicEntropy {
            spread: parse_f64(&field_atom(&node, "spread")?)?,
        }),
        _ => Err(SoundShapeError::InvalidSoundShape),
    }
}

/// Decodes [`BridgeOptions`] from its sound-shape text form.
pub fn decode_bridge_options(value: &str) -> Result<BridgeOptions, SoundShapeError> {
    let node = parse_node(value)?;
    BridgeOptions::new(
        field_atom(&node, "polyphony_limit")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        parse_f64(&field_atom(&node, "bend_range_cents")?)?,
    )
    .map_err(|_| SoundShapeError::InvalidSoundShape)
}

/// Decodes [`RendererOptions`] from its sound-shape text form.
pub fn decode_renderer_options(value: &str) -> Result<RendererOptions, SoundShapeError> {
    let node = parse_node(value)?;
    RendererOptions::new(
        field_atom(&node, "sample_rate")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        field_atom(&node, "channels")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
    )
    .map_err(|_| SoundShapeError::InvalidSoundShape)
}

/// Decodes a [`TimbreBank`] from its sound-shape text form.
pub fn decode_timbre_bank(value: &str) -> Result<TimbreBank, SoundShapeError> {
    let node = parse_node(value)?;
    let mut bank = TimbreBank::new(decode_timbre(&field_form_text(&node, "fallback")?)?);
    for entry in field_list(&node, "entries")? {
        let entry = entry.as_form()?;
        let bank_msb = field_atom(entry, "bank_msb")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?;
        let bank_lsb = field_atom(entry, "bank_lsb")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?;
        let program = field_atom(entry, "program")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?;
        let timbre = decode_timbre(&field_form_text(entry, "timbre")?)?;
        bank.insert(bank_msb, bank_lsb, program, timbre);
    }
    Ok(bank)
}
