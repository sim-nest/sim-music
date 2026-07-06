use sim_citizen_derive::Citizen;
use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_sound_core::{Envelope, Partial, Tone};
use sim_lib_sound_spectrum::Spectrum;
use sim_lib_sound_timbre::Timbre;
use sim_lib_sound_tuning::TuningDescriptor;

use crate::{
    SoundShapeError, decode_envelope, decode_partial, decode_spectrum, decode_timbre, decode_tone,
    decode_tuning_descriptor, encode_envelope, encode_partial, encode_spectrum, encode_timbre,
    encode_tone, encode_tuning_descriptor,
};

macro_rules! text_citizen {
    (
        $name:ident,
        $symbol:literal,
        $symbol_fn:ident,
        $field_mod:ident,
        $field_path:literal,
        $default:expr,
        $canonical:ident
    ) => {
        #[doc = concat!("A citizen descriptor wrapping the canonical `", $symbol, "` text form.")]
        #[derive(Clone, Debug, PartialEq, Citizen)]
        #[citizen(symbol = $symbol, version = 1)]
        pub struct $name {
            #[citizen(with = $field_path)]
            form: String,
        }

        impl $name {
            /// Builds the descriptor from text, canonicalizing and validating it.
            pub fn from_text(value: &str) -> Result<Self> {
                Ok(Self {
                    form: $canonical(value)?,
                })
            }

            /// Returns the canonical text form.
            pub fn as_text(&self) -> &str {
                &self.form
            }

            /// Builds a citizen read-construct expression from `value`.
            pub fn read_construct_expr_from_text(value: &str) -> Result<Expr> {
                Ok(sim_citizen::text_read_construct_expr(
                    $symbol_fn(),
                    $canonical(value)?,
                ))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                let default_form = $default.to_string();
                Self::from_text(&default_form).expect("default citizen sound form should be valid")
            }
        }

        #[doc = concat!("Returns the class symbol for `", $symbol, "`.")]
        pub fn $symbol_fn() -> Symbol {
            sim_citizen::parse_symbol($symbol)
        }

        pub(crate) mod $field_mod {
            use sim_kernel::{Error, Expr, Result};

            pub fn encode(value: &str) -> Expr {
                Expr::String(value.to_owned())
            }

            pub fn decode(expr: &Expr) -> Result<String> {
                match expr {
                    Expr::String(value) => super::$canonical(value),
                    other => Err(Error::Eval(format!(
                        "sound citizen form must be a string, found {}",
                        super::expr_kind(other)
                    ))),
                }
            }
        }
    };
}

text_citizen!(
    SoundToneDescriptor,
    "sound/Tone",
    sound_tone_class_symbol,
    tone_form,
    "tone_form",
    default_tone_form(),
    canonical_tone
);
text_citizen!(
    SoundPartialDescriptor,
    "sound/Partial",
    sound_partial_class_symbol,
    partial_form,
    "partial_form",
    default_partial_form(),
    canonical_partial
);
text_citizen!(
    SoundEnvelopeDescriptor,
    "sound/Envelope",
    sound_envelope_class_symbol,
    envelope_form,
    "envelope_form",
    default_envelope_form(),
    canonical_envelope
);
text_citizen!(
    SoundSpectrumDescriptor,
    "sound/Spectrum",
    sound_spectrum_class_symbol,
    spectrum_form,
    "spectrum_form",
    default_spectrum_form(),
    canonical_spectrum
);
text_citizen!(
    SoundTimbreDescriptor,
    "sound/Timbre",
    sound_timbre_class_symbol,
    timbre_form,
    "timbre_form",
    default_timbre_form(),
    canonical_timbre
);
text_citizen!(
    SoundTuningDescriptor,
    "sound/TuningDescriptor",
    sound_tuning_descriptor_class_symbol,
    tuning_form,
    "tuning_form",
    default_tuning_form(),
    canonical_tuning_descriptor
);

impl SoundToneDescriptor {
    /// Decodes the descriptor into a [`Tone`].
    pub fn tone(&self) -> Result<Tone> {
        decode_tone(&self.form).map_err(codec_error)
    }
}

impl SoundPartialDescriptor {
    /// Decodes the descriptor into a [`Partial`].
    pub fn partial(&self) -> Result<Partial> {
        decode_partial(&self.form).map_err(codec_error)
    }
}

impl SoundEnvelopeDescriptor {
    /// Decodes the descriptor into an [`Envelope`].
    pub fn envelope(&self) -> Result<Envelope> {
        decode_envelope(&self.form).map_err(codec_error)
    }
}

impl SoundSpectrumDescriptor {
    /// Decodes the descriptor into a [`Spectrum`].
    pub fn spectrum(&self) -> Result<Spectrum> {
        decode_spectrum(&self.form).map_err(codec_error)
    }
}

impl SoundTimbreDescriptor {
    /// Decodes the descriptor into a [`Timbre`].
    pub fn timbre(&self) -> Result<Timbre> {
        decode_timbre(&self.form).map_err(codec_error)
    }
}

impl SoundTuningDescriptor {
    /// Decodes the descriptor into a [`TuningDescriptor`].
    pub fn tuning(&self) -> Result<TuningDescriptor> {
        decode_tuning_descriptor(&self.form).map_err(codec_error)
    }
}

fn canonical_tone(value: &str) -> Result<String> {
    decode_tone(value)
        .map(|tone| encode_tone(&tone))
        .map_err(codec_error)
}

fn canonical_partial(value: &str) -> Result<String> {
    decode_partial(value)
        .map(|partial| encode_partial(&partial))
        .map_err(codec_error)
}

fn canonical_envelope(value: &str) -> Result<String> {
    decode_envelope(value)
        .map(|envelope| encode_envelope(&envelope))
        .map_err(codec_error)
}

fn canonical_spectrum(value: &str) -> Result<String> {
    decode_spectrum(value)
        .map(|spectrum| encode_spectrum(&spectrum))
        .map_err(codec_error)
}

fn canonical_timbre(value: &str) -> Result<String> {
    decode_timbre(value)
        .map(|timbre| encode_timbre(&timbre))
        .map_err(codec_error)
}

fn canonical_tuning_descriptor(value: &str) -> Result<String> {
    decode_tuning_descriptor(value)
        .map(|tuning| encode_tuning_descriptor(&tuning))
        .map_err(codec_error)
}

fn default_tone_form() -> String {
    sound_form(
        "Tone",
        &format!(
            " partials=[{}] envelope={} duration=1",
            default_partial_form(),
            default_envelope_form()
        ),
    )
}

fn default_partial_form() -> String {
    sound_form(
        "Partial",
        &format!(
            " frequency={} amplitude={} phase={}",
            sound_form("Frequency", " hz=440"),
            sound_form("Amplitude", " linear=1"),
            sound_form("Phase", " radians=0")
        ),
    )
}

fn default_envelope_form() -> String {
    sound_form(
        "Envelope",
        &format!(
            " attack=0.01 decay=0.03 sustain=0.8 release=0.08 shape={}",
            sound_form("EnvelopeShape", " kind=Linear")
        ),
    )
}

fn default_spectrum_form() -> String {
    sound_form(
        "Spectrum",
        &format!(
            " bins=[] source={}",
            sound_form("SpectrumSource", " kind=Synthetic")
        ),
    )
}

fn default_timbre_form() -> String {
    sound_form(
        "Timbre",
        &format!(
            " name=\"pure_sine\" recipe={} envelope={} meta={} filters=[]",
            sound_form("TimbreRecipe", " kind=PureSine"),
            default_envelope_form(),
            sound_form(
                "TimbreMeta",
                &format!(
                    " brightness=1 roughness=0 attack_kind={} category=\"pure\"",
                    sound_form("AttackKind", " kind=Soft")
                )
            )
        ),
    )
}

fn default_tuning_form() -> String {
    sound_form(
        "TuningDescriptor",
        " kind=EqualTemperament divisions=12 reference_midi=69 reference_hz=440",
    )
}

fn sound_form(head: &str, body: &str) -> String {
    format!("{}({head}{body})", "#")
}

fn codec_error(error: SoundShapeError) -> Error {
    Error::Eval(format!("invalid sound citizen form: {error}"))
}

use sim_value::kind::expr_kind;
