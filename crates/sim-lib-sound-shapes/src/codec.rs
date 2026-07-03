use std::time::Duration;

use sim_lib_sound_audio_lift::{
    AudioLiftFrame, AudioLiftOptions, AudioNoteCandidate, PitchCandidate,
};
use sim_lib_sound_bridge::{BridgeOptions, TimbreBank};
use sim_lib_sound_core::{Amplitude, Envelope, EnvelopeShape, Frequency, Partial, Phase, Tone};
use sim_lib_sound_dissonance::DissonanceModelDescriptor;
use sim_lib_sound_render::RendererOptions;
use sim_lib_sound_spectrum::{Spectrum, SpectrumSource};
use sim_lib_sound_timbre::{AttackKind, Filter, Timbre, TimbreMeta, TimbreRecipe};
use sim_lib_sound_tuning::{PitchClassN, TuningDescriptor};
use thiserror::Error;

pub use crate::parse::{
    decode_amplitude, decode_attack_kind, decode_envelope, decode_envelope_shape, decode_filter,
    decode_frequency, decode_partial, decode_phase, decode_spectrum, decode_spectrum_source,
    decode_timbre, decode_timbre_meta, decode_timbre_recipe, decode_tone,
};
pub use crate::parse_lift::{
    decode_audio_lift_frame, decode_audio_lift_options, decode_audio_note_candidate,
    decode_pitch_candidate,
};
pub use crate::parse_surface::{
    decode_bridge_options, decode_dissonance_model_descriptor, decode_pitch_class_n,
    decode_renderer_options, decode_timbre_bank, decode_tuning_descriptor,
};

/// Error raised when decoding a sound-shape text form.
#[derive(Debug, Error)]
pub enum SoundShapeError {
    /// The input ended before a complete form was parsed.
    #[error("unexpected end of input")]
    UnexpectedEof,
    /// A token did not match the expected grammar.
    #[error("invalid token in sound shape")]
    InvalidToken,
    /// The parsed form did not describe a valid sound shape.
    #[error("invalid sound shape")]
    InvalidSoundShape,
}

/// Encodes a [`Frequency`] into its sound-shape text form.
///
/// # Examples
///
/// ```
/// use sim_lib_sound_core::Frequency;
/// use sim_lib_sound_shapes::{decode_frequency, encode_frequency};
///
/// let text = encode_frequency(Frequency(440.0));
/// assert_eq!(decode_frequency(&text).unwrap(), Frequency(440.0));
/// ```
pub fn encode_frequency(value: Frequency) -> String {
    format!("#(Frequency hz={})", value.0)
}

/// Encodes an [`Amplitude`] into its sound-shape text form.
pub fn encode_amplitude(value: Amplitude) -> String {
    format!("#(Amplitude linear={})", value.0)
}

/// Encodes a [`Phase`] into its sound-shape text form.
pub fn encode_phase(value: Phase) -> String {
    format!("#(Phase radians={})", value.0)
}

/// Encodes a [`Partial`] into its sound-shape text form.
pub fn encode_partial(value: &Partial) -> String {
    format!(
        "#(Partial frequency={} amplitude={} phase={})",
        encode_frequency(value.frequency),
        encode_amplitude(value.amplitude),
        encode_phase(value.phase),
    )
}

/// Encodes an [`EnvelopeShape`] into its sound-shape text form.
pub fn encode_envelope_shape(value: &EnvelopeShape) -> String {
    match value {
        EnvelopeShape::Linear => "#(EnvelopeShape kind=Linear)".to_owned(),
        EnvelopeShape::Exponential(curve) => {
            format!("#(EnvelopeShape kind=Exponential curve={curve})")
        }
        EnvelopeShape::Custom(name) => {
            format!("#(EnvelopeShape kind=Custom name={})", encode_string(name))
        }
    }
}

/// Encodes an [`Envelope`] into its sound-shape text form.
pub fn encode_envelope(value: &Envelope) -> String {
    format!(
        "#(Envelope attack={} decay={} sustain={} release={} shape={})",
        encode_duration(value.attack),
        encode_duration(value.decay),
        value.sustain,
        encode_duration(value.release),
        encode_envelope_shape(&value.shape),
    )
}

/// Encodes a [`Tone`] into its sound-shape text form.
pub fn encode_tone(value: &Tone) -> String {
    format!(
        "#(Tone partials=[{}] envelope={} duration={})",
        value
            .partials
            .iter()
            .map(encode_partial)
            .collect::<Vec<_>>()
            .join(","),
        encode_envelope(&value.envelope),
        encode_duration(value.duration),
    )
}

/// Encodes a [`SpectrumSource`] into its sound-shape text form.
pub fn encode_spectrum_source(value: &SpectrumSource) -> String {
    match value {
        SpectrumSource::FromTone { at_millis } => {
            format!("#(SpectrumSource kind=FromTone at_millis={at_millis})")
        }
        SpectrumSource::FromPcm {
            window_size,
            sample_rate,
        } => format!(
            "#(SpectrumSource kind=FromPcm window_size={} sample_rate={})",
            window_size, sample_rate
        ),
        SpectrumSource::Synthetic => "#(SpectrumSource kind=Synthetic)".to_owned(),
    }
}

/// Encodes a [`Spectrum`] into its sound-shape text form.
pub fn encode_spectrum(value: &Spectrum) -> String {
    let bins = value
        .bins
        .iter()
        .map(|(frequency, amplitude)| {
            format!(
                "#(Bin frequency={} amplitude={})",
                encode_frequency(*frequency),
                encode_amplitude(*amplitude),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "#(Spectrum bins=[{}] source={})",
        bins,
        encode_spectrum_source(&value.source),
    )
}

/// Encodes an [`AttackKind`] into its sound-shape text form.
pub fn encode_attack_kind(value: AttackKind) -> String {
    let kind = match value {
        AttackKind::Soft => "Soft",
        AttackKind::Plucked => "Plucked",
        AttackKind::Bowed => "Bowed",
        AttackKind::Struck => "Struck",
    };
    format!("#(AttackKind kind={kind})")
}

/// Encodes a [`TimbreMeta`] into its sound-shape text form.
pub fn encode_timbre_meta(value: &TimbreMeta) -> String {
    format!(
        "#(TimbreMeta brightness={} roughness={} attack_kind={} category={})",
        value.brightness,
        value.roughness,
        encode_attack_kind(value.attack_kind),
        encode_string(&value.category),
    )
}

/// Encodes a [`Filter`] into its sound-shape text form.
pub fn encode_filter(value: &Filter) -> String {
    match value {
        Filter::LowPass { cutoff, q } => format!(
            "#(Filter kind=LowPass cutoff={} q={})",
            encode_frequency(*cutoff),
            q
        ),
        Filter::HighPass { cutoff, q } => format!(
            "#(Filter kind=HighPass cutoff={} q={})",
            encode_frequency(*cutoff),
            q
        ),
        Filter::BandPass { center, q, gain } => format!(
            "#(Filter kind=BandPass center={} q={} gain={})",
            encode_frequency(*center),
            q,
            encode_amplitude(*gain),
        ),
        Filter::Notch { center, q } => format!(
            "#(Filter kind=Notch center={} q={})",
            encode_frequency(*center),
            q
        ),
        Filter::Formant { bands } => format!(
            "#(Filter kind=Formant bands=[{}])",
            bands
                .iter()
                .map(|(frequency, width, gain)| format!(
                    "#(Band frequency={} width={} gain={})",
                    encode_frequency(*frequency),
                    width,
                    encode_amplitude(*gain),
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// Encodes a [`TimbreRecipe`] into its sound-shape text form.
pub fn encode_timbre_recipe(value: &TimbreRecipe) -> String {
    match value {
        TimbreRecipe::PureSine => "#(TimbreRecipe kind=PureSine)".to_owned(),
        TimbreRecipe::Sawtooth { partials } => {
            format!("#(TimbreRecipe kind=Sawtooth partials={partials})")
        }
        TimbreRecipe::Square { partials } => {
            format!("#(TimbreRecipe kind=Square partials={partials})")
        }
        TimbreRecipe::Triangle { partials } => {
            format!("#(TimbreRecipe kind=Triangle partials={partials})")
        }
        TimbreRecipe::OrganPipe { stops } => format!(
            "#(TimbreRecipe kind=OrganPipe stops=[{}])",
            stops
                .iter()
                .map(|stop| stop.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        TimbreRecipe::KarplusStrong { damping } => {
            format!("#(TimbreRecipe kind=KarplusStrong damping={damping})")
        }
        TimbreRecipe::FmPair {
            modulator_ratio,
            index,
        } => format!(
            "#(TimbreRecipe kind=FmPair modulator_ratio={} index={})",
            modulator_ratio, index
        ),
        TimbreRecipe::BellInharmonic { ratios } => format!(
            "#(TimbreRecipe kind=BellInharmonic ratios=[{}])",
            ratios
                .iter()
                .map(|ratio| ratio.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        TimbreRecipe::Layered {
            primary,
            secondary,
            mix,
        } => format!(
            "#(TimbreRecipe kind=Layered primary={} secondary={} mix={})",
            encode_timbre_recipe(primary),
            encode_timbre_recipe(secondary),
            mix,
        ),
    }
}

/// Encodes a [`Timbre`] into its sound-shape text form.
pub fn encode_timbre(value: &Timbre) -> String {
    format!(
        "#(Timbre name={} recipe={} envelope={} meta={} filters=[{}])",
        encode_string(&value.name),
        encode_timbre_recipe(&value.recipe),
        encode_envelope(&value.default_envelope),
        encode_timbre_meta(&value.metadata),
        value
            .filters
            .iter()
            .map(encode_filter)
            .collect::<Vec<_>>()
            .join(","),
    )
}

/// Encodes a [`PitchClassN`] into its sound-shape text form.
pub fn encode_pitch_class_n(value: PitchClassN) -> String {
    format!(
        "#(PitchClassN divisions={} index={})",
        value.divisions, value.index
    )
}

/// Encodes a [`TuningDescriptor`] into its sound-shape text form.
pub fn encode_tuning_descriptor(value: &TuningDescriptor) -> String {
    match value {
        TuningDescriptor::EqualTemperament {
            divisions,
            reference_midi,
            reference_hz,
        } => format!(
            "#(TuningDescriptor kind=EqualTemperament divisions={} reference_midi={} reference_hz={})",
            divisions, reference_midi, reference_hz
        ),
        TuningDescriptor::JustIntonation {
            root,
            ratios,
            reference_midi,
            reference_hz,
        } => format!(
            "#(TuningDescriptor kind=JustIntonation root={} ratios=[{}] reference_midi={} reference_hz={})",
            root,
            ratios
                .iter()
                .map(|ratio| ratio.to_string())
                .collect::<Vec<_>>()
                .join(","),
            reference_midi,
            reference_hz
        ),
        TuningDescriptor::PythagoreanTuning {
            reference_midi,
            reference_hz,
        } => format!(
            "#(TuningDescriptor kind=PythagoreanTuning reference_midi={} reference_hz={})",
            reference_midi, reference_hz
        ),
        TuningDescriptor::MeantoneQuarterComma {
            reference_midi,
            reference_hz,
        } => format!(
            "#(TuningDescriptor kind=MeantoneQuarterComma reference_midi={} reference_hz={})",
            reference_midi, reference_hz
        ),
        TuningDescriptor::WerckmeisterIII {
            reference_midi,
            reference_hz,
        } => format!(
            "#(TuningDescriptor kind=WerckmeisterIII reference_midi={} reference_hz={})",
            reference_midi, reference_hz
        ),
        TuningDescriptor::YoungTemperament {
            reference_midi,
            reference_hz,
        } => format!(
            "#(TuningDescriptor kind=YoungTemperament reference_midi={} reference_hz={})",
            reference_midi, reference_hz
        ),
        TuningDescriptor::ScalaScl {
            cents,
            reference_midi,
            reference_hz,
        } => format!(
            "#(TuningDescriptor kind=ScalaScl cents=[{}] reference_midi={} reference_hz={})",
            cents
                .iter()
                .map(|cents| cents.to_string())
                .collect::<Vec<_>>()
                .join(","),
            reference_midi,
            reference_hz
        ),
    }
}

/// Encodes a [`DissonanceModelDescriptor`] into its sound-shape text form.
pub fn encode_dissonance_model_descriptor(value: &DissonanceModelDescriptor) -> String {
    match value {
        DissonanceModelDescriptor::PlompLevelt => {
            "#(DissonanceModelDescriptor kind=PlompLevelt)".to_owned()
        }
        DissonanceModelDescriptor::Sethares => {
            "#(DissonanceModelDescriptor kind=Sethares)".to_owned()
        }
        DissonanceModelDescriptor::HelmholtzBeating => {
            "#(DissonanceModelDescriptor kind=HelmholtzBeating)".to_owned()
        }
        DissonanceModelDescriptor::HarmonicEntropy { spread } => {
            format!("#(DissonanceModelDescriptor kind=HarmonicEntropy spread={spread})")
        }
    }
}

/// Encodes [`BridgeOptions`] into its sound-shape text form.
pub fn encode_bridge_options(value: &BridgeOptions) -> String {
    format!(
        "#(BridgeOptions polyphony_limit={} bend_range_cents={})",
        value.polyphony_limit, value.bend_range_cents
    )
}

/// Encodes [`RendererOptions`] into its sound-shape text form.
pub fn encode_renderer_options(value: &RendererOptions) -> String {
    format!(
        "#(RendererOptions sample_rate={} channels={})",
        value.sample_rate, value.channels
    )
}

/// Encodes a [`TimbreBank`] into its sound-shape text form.
pub fn encode_timbre_bank(value: &TimbreBank) -> String {
    let entries = value
        .entries()
        .iter()
        .map(|((msb, lsb, program), timbre)| {
            format!(
                "#(BankEntry bank_msb={} bank_lsb={} program={} timbre={})",
                msb,
                lsb,
                program,
                encode_timbre(timbre),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "#(TimbreBank fallback={} entries=[{}])",
        encode_timbre(value.fallback()),
        entries
    )
}

/// Encodes [`AudioLiftOptions`] into its sound-shape text form.
pub fn encode_audio_lift_options(value: &AudioLiftOptions) -> String {
    format!(
        "#(AudioLiftOptions window_size={} hop_size={} max_peaks={} min_peak_ratio={} harmonic_tolerance_cents={} min_note_confidence={} min_note_windows={})",
        value.window_size,
        value.hop_size,
        value.max_peaks,
        value.min_peak_ratio,
        value.harmonic_tolerance_cents,
        value.min_note_confidence,
        value.min_note_windows,
    )
}

/// Encodes a [`PitchCandidate`] into its sound-shape text form.
pub fn encode_pitch_candidate(value: &PitchCandidate) -> String {
    format!(
        "#(PitchCandidate semitone={} frequency={} amplitude={} confidence={} cents_error={} harmonic_count={})",
        value.pitch.semitone(),
        encode_frequency(value.frequency),
        encode_amplitude(value.amplitude),
        value.confidence,
        value.cents_error,
        value.harmonic_count,
    )
}

/// Encodes an [`AudioLiftFrame`] into its sound-shape text form.
pub fn encode_audio_lift_frame(value: &AudioLiftFrame) -> String {
    format!(
        "#(AudioLiftFrame index={} onset_sample={} duration_samples={} spectrum={} pitch_candidates=[{}] diagnostics=[{}])",
        value.index,
        value.onset_sample,
        value.duration_samples,
        encode_spectrum(&value.spectrum),
        value
            .pitch_candidates
            .iter()
            .map(encode_pitch_candidate)
            .collect::<Vec<_>>()
            .join(","),
        encode_string_list(&value.diagnostics),
    )
}

/// Encodes an [`AudioNoteCandidate`] into its sound-shape text form.
pub fn encode_audio_note_candidate(value: &AudioNoteCandidate) -> String {
    format!(
        "#(AudioNoteCandidate track={} onset_sample={} duration_samples={} sample_rate={} semitone={} mean_frequency={} mean_amplitude={} confidence={} diagnostics=[{}])",
        value.track,
        value.onset_sample,
        value.duration_samples,
        value.sample_rate,
        value.pitch.semitone(),
        encode_frequency(value.mean_frequency),
        encode_amplitude(value.mean_amplitude),
        value.confidence,
        encode_string_list(&value.diagnostics),
    )
}

fn encode_duration(value: Duration) -> String {
    value.as_secs_f64().to_string()
}

fn encode_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(crate) fn encode_string_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| encode_string(value))
        .collect::<Vec<_>>()
        .join(",")
}
