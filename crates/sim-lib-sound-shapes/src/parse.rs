use std::time::Duration;

use sim_codec::{DomainForm, DomainFormError, DomainValue, format_domain_form, parse_domain_form};
use sim_lib_sound_core::{Amplitude, Envelope, EnvelopeShape, Frequency, Partial, Phase, Tone};
use sim_lib_sound_spectrum::{Spectrum, SpectrumSource};
use sim_lib_sound_timbre::{AttackKind, Filter, Timbre, TimbreMeta, TimbreRecipe};

use crate::SoundShapeError;

/// Local alias for the shared domain-form value, retained for the field helpers
/// and the sibling parse modules that pattern-match list items.
pub(crate) type Node = DomainValue;

impl From<DomainFormError> for SoundShapeError {
    fn from(error: DomainFormError) -> Self {
        match error {
            DomainFormError::UnexpectedEof => SoundShapeError::UnexpectedEof,
            DomainFormError::ExpectedForm
            | DomainFormError::InvalidToken
            | DomainFormError::TrailingInput
            | DomainFormError::DuplicateField(_) => SoundShapeError::InvalidToken,
            DomainFormError::MissingField(_) | DomainFormError::WrongFieldKind(_) => {
                SoundShapeError::InvalidSoundShape
            }
        }
    }
}

/// Decodes a [`Frequency`] from its sound-shape text form.
pub fn decode_frequency(value: &str) -> Result<Frequency, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(Frequency(parse_f64(&field_atom(&node, "hz")?)?))
}

/// Decodes an [`Amplitude`] from its sound-shape text form.
pub fn decode_amplitude(value: &str) -> Result<Amplitude, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(Amplitude(parse_f64(&field_atom(&node, "linear")?)?))
}

/// Decodes a [`Phase`] from its sound-shape text form.
pub fn decode_phase(value: &str) -> Result<Phase, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(Phase(parse_f64(&field_atom(&node, "radians")?)?))
}

/// Decodes a [`Partial`] from its sound-shape text form.
pub fn decode_partial(value: &str) -> Result<Partial, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(Partial {
        frequency: decode_frequency(&field_form_text(&node, "frequency")?)?,
        amplitude: decode_amplitude(&field_form_text(&node, "amplitude")?)?,
        phase: decode_phase(&field_form_text(&node, "phase")?)?,
    })
}

/// Decodes an [`EnvelopeShape`] from its sound-shape text form.
pub fn decode_envelope_shape(value: &str) -> Result<EnvelopeShape, SoundShapeError> {
    let node = parse_node(value)?;
    match field_atom(&node, "kind")?.as_str() {
        "Linear" => Ok(EnvelopeShape::Linear),
        "Exponential" => Ok(EnvelopeShape::Exponential(parse_f64(&field_atom(
            &node, "curve",
        )?)?)),
        "Custom" => Ok(EnvelopeShape::Custom(field_string(&node, "name")?)),
        _ => Err(SoundShapeError::InvalidSoundShape),
    }
}

/// Decodes an [`Envelope`] from its sound-shape text form.
pub fn decode_envelope(value: &str) -> Result<Envelope, SoundShapeError> {
    let node = parse_node(value)?;
    Envelope::new(
        decode_duration(&field_atom(&node, "attack")?)?,
        decode_duration(&field_atom(&node, "decay")?)?,
        parse_f64(&field_atom(&node, "sustain")?)?,
        decode_duration(&field_atom(&node, "release")?)?,
        decode_envelope_shape(&field_form_text(&node, "shape")?)?,
    )
    .map_err(|_| SoundShapeError::InvalidSoundShape)
}

/// Decodes a [`Tone`] from its sound-shape text form.
pub fn decode_tone(value: &str) -> Result<Tone, SoundShapeError> {
    let node = parse_node(value)?;
    let partials = field_list(&node, "partials")?
        .iter()
        .map(node_text)
        .map(|text| decode_partial(&text))
        .collect::<Result<Vec<_>, _>>()?;
    Tone::from_partials(
        partials,
        decode_envelope(&field_form_text(&node, "envelope")?)?,
        decode_duration(&field_atom(&node, "duration")?)?,
    )
    .map_err(|_| SoundShapeError::InvalidSoundShape)
}

/// Decodes a [`SpectrumSource`] from its sound-shape text form.
pub fn decode_spectrum_source(value: &str) -> Result<SpectrumSource, SoundShapeError> {
    let node = parse_node(value)?;
    match field_atom(&node, "kind")?.as_str() {
        "FromTone" => Ok(SpectrumSource::FromTone {
            at_millis: field_atom(&node, "at_millis")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        }),
        "FromPcm" => Ok(SpectrumSource::FromPcm {
            window_size: field_atom(&node, "window_size")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
            sample_rate: field_atom(&node, "sample_rate")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        }),
        "Synthetic" => Ok(SpectrumSource::Synthetic),
        _ => Err(SoundShapeError::InvalidSoundShape),
    }
}

/// Decodes a [`Spectrum`] from its sound-shape text form.
pub fn decode_spectrum(value: &str) -> Result<Spectrum, SoundShapeError> {
    let node = parse_node(value)?;
    let bins = field_list(&node, "bins")?
        .iter()
        .map(|node| match node {
            DomainValue::Form(_) => Ok(node),
            _ => Err(SoundShapeError::InvalidSoundShape),
        })
        .map(|node| {
            let node = node?;
            Ok((
                decode_frequency(&field_form_text(node, "frequency")?)?,
                decode_amplitude(&field_form_text(node, "amplitude")?)?,
            ))
        })
        .collect::<Result<Vec<_>, SoundShapeError>>()?;
    Ok(Spectrum {
        bins,
        source: decode_spectrum_source(&field_form_text(&node, "source")?)?,
    })
}

/// Decodes an [`AttackKind`] from its sound-shape text form.
pub fn decode_attack_kind(value: &str) -> Result<AttackKind, SoundShapeError> {
    let node = parse_node(value)?;
    match field_atom(&node, "kind")?.as_str() {
        "Soft" => Ok(AttackKind::Soft),
        "Plucked" => Ok(AttackKind::Plucked),
        "Bowed" => Ok(AttackKind::Bowed),
        "Struck" => Ok(AttackKind::Struck),
        _ => Err(SoundShapeError::InvalidSoundShape),
    }
}

/// Decodes a [`TimbreMeta`] from its sound-shape text form.
pub fn decode_timbre_meta(value: &str) -> Result<TimbreMeta, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(TimbreMeta {
        brightness: parse_f64(&field_atom(&node, "brightness")?)?,
        roughness: parse_f64(&field_atom(&node, "roughness")?)?,
        attack_kind: decode_attack_kind(&field_form_text(&node, "attack_kind")?)?,
        category: field_string(&node, "category")?,
    })
}

/// Decodes a [`Filter`] from its sound-shape text form.
pub fn decode_filter(value: &str) -> Result<Filter, SoundShapeError> {
    let node = parse_node(value)?;
    match field_atom(&node, "kind")?.as_str() {
        "LowPass" => Ok(Filter::LowPass {
            cutoff: decode_frequency(&field_form_text(&node, "cutoff")?)?,
            q: parse_f64(&field_atom(&node, "q")?)?,
        }),
        "HighPass" => Ok(Filter::HighPass {
            cutoff: decode_frequency(&field_form_text(&node, "cutoff")?)?,
            q: parse_f64(&field_atom(&node, "q")?)?,
        }),
        "BandPass" => Ok(Filter::BandPass {
            center: decode_frequency(&field_form_text(&node, "center")?)?,
            q: parse_f64(&field_atom(&node, "q")?)?,
            gain: decode_amplitude(&field_form_text(&node, "gain")?)?,
        }),
        "Notch" => Ok(Filter::Notch {
            center: decode_frequency(&field_form_text(&node, "center")?)?,
            q: parse_f64(&field_atom(&node, "q")?)?,
        }),
        "Formant" => Ok(Filter::Formant {
            bands: field_list(&node, "bands")?
                .iter()
                .map(|band| {
                    Ok((
                        decode_frequency(&field_form_text(band, "frequency")?)?,
                        parse_f64(&field_atom(band, "width")?)?,
                        decode_amplitude(&field_form_text(band, "gain")?)?,
                    ))
                })
                .collect::<Result<Vec<_>, SoundShapeError>>()?,
        }),
        _ => Err(SoundShapeError::InvalidSoundShape),
    }
}

/// Decodes a [`TimbreRecipe`] from its sound-shape text form.
pub fn decode_timbre_recipe(value: &str) -> Result<TimbreRecipe, SoundShapeError> {
    let node = parse_node(value)?;
    match field_atom(&node, "kind")?.as_str() {
        "PureSine" => Ok(TimbreRecipe::PureSine),
        "Sawtooth" => Ok(TimbreRecipe::Sawtooth {
            partials: field_atom(&node, "partials")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        }),
        "Square" => Ok(TimbreRecipe::Square {
            partials: field_atom(&node, "partials")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        }),
        "Triangle" => Ok(TimbreRecipe::Triangle {
            partials: field_atom(&node, "partials")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        }),
        "OrganPipe" => Ok(TimbreRecipe::OrganPipe {
            stops: field_list(&node, "stops")?
                .iter()
                .map(atom_text)
                .map(|value| parse_f64(&value))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        "KarplusStrong" => Ok(TimbreRecipe::KarplusStrong {
            damping: parse_f64(&field_atom(&node, "damping")?)?,
        }),
        "FmPair" => Ok(TimbreRecipe::FmPair {
            modulator_ratio: parse_f64(&field_atom(&node, "modulator_ratio")?)?,
            index: parse_f64(&field_atom(&node, "index")?)?,
        }),
        "BellInharmonic" => Ok(TimbreRecipe::BellInharmonic {
            ratios: field_list(&node, "ratios")?
                .iter()
                .map(atom_text)
                .map(|value| parse_f64(&value))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        "Layered" => Ok(TimbreRecipe::Layered {
            primary: Box::new(decode_timbre_recipe(&field_form_text(&node, "primary")?)?),
            secondary: Box::new(decode_timbre_recipe(&field_form_text(&node, "secondary")?)?),
            mix: parse_f64(&field_atom(&node, "mix")?)?,
        }),
        _ => Err(SoundShapeError::InvalidSoundShape),
    }
}

/// Decodes a [`Timbre`] from its sound-shape text form.
pub fn decode_timbre(value: &str) -> Result<Timbre, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(Timbre {
        name: field_string(&node, "name")?,
        recipe: decode_timbre_recipe(&field_form_text(&node, "recipe")?)?,
        default_envelope: decode_envelope(&field_form_text(&node, "envelope")?)?,
        metadata: decode_timbre_meta(&field_form_text(&node, "meta")?)?,
        filters: field_list(&node, "filters")?
            .iter()
            .map(node_text)
            .map(|text| decode_filter(&text))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(crate) fn parse_node(value: &str) -> Result<DomainForm, SoundShapeError> {
    Ok(parse_domain_form(value)?)
}

/// Accessor over a parsed value that is expected to be a `#(...)` form: either
/// the top-level form, or a form-typed list item.
pub(crate) trait AsForm {
    fn as_form(&self) -> Result<&DomainForm, SoundShapeError>;
}

impl AsForm for DomainForm {
    fn as_form(&self) -> Result<&DomainForm, SoundShapeError> {
        Ok(self)
    }
}

impl AsForm for DomainValue {
    fn as_form(&self) -> Result<&DomainForm, SoundShapeError> {
        match self {
            DomainValue::Form(form) => Ok(form),
            _ => Err(SoundShapeError::InvalidSoundShape),
        }
    }
}

pub(crate) fn field_atom(node: &impl AsForm, field: &str) -> Result<String, SoundShapeError> {
    Ok(node.as_form()?.atom(field)?.to_owned())
}

pub(crate) fn field_string(node: &impl AsForm, field: &str) -> Result<String, SoundShapeError> {
    Ok(node.as_form()?.string(field)?.to_owned())
}

pub(crate) fn field_form_text(node: &impl AsForm, field: &str) -> Result<String, SoundShapeError> {
    Ok(node_text(field_node(node, field)?))
}

pub(crate) fn field_list<'a>(
    node: &'a impl AsForm,
    field: &str,
) -> Result<&'a [DomainValue], SoundShapeError> {
    Ok(node.as_form()?.list(field)?)
}

fn field_node<'a>(node: &'a impl AsForm, field: &str) -> Result<&'a DomainValue, SoundShapeError> {
    node.as_form()?
        .field(field)
        .ok_or(SoundShapeError::InvalidSoundShape)
}

pub(crate) fn atom_text(node: &DomainValue) -> String {
    match node {
        DomainValue::Atom(atom) => atom.clone(),
        _ => String::new(),
    }
}

pub(crate) fn node_text(node: &DomainValue) -> String {
    match node {
        DomainValue::Form(form) => format_domain_form(form),
        DomainValue::List(items) => format!(
            "[{}]",
            items.iter().map(node_text).collect::<Vec<_>>().join(",")
        ),
        DomainValue::String(value) => {
            format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
        DomainValue::Atom(value) => value.clone(),
    }
}

pub(crate) fn parse_f64(value: &str) -> Result<f64, SoundShapeError> {
    value
        .parse()
        .map_err(|_| SoundShapeError::InvalidSoundShape)
}

fn decode_duration(value: &str) -> Result<Duration, SoundShapeError> {
    Ok(Duration::from_secs_f64(parse_f64(value)?))
}
