use sim_citizen_derive::Citizen;
use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_pitch_chord::Chord;
use sim_lib_pitch_core::{Interval, Pitch};
use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_set::PitchClassMask;

use crate::{
    PitchShapeError, decode_chord, decode_interval, decode_pitch, decode_pitch_class_mask,
    decode_scale, encode_chord, encode_interval, encode_pitch, encode_pitch_class_mask,
    encode_scale,
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
        #[doc = concat!("Citizen descriptor wrapping the canonical text form of the `", $symbol, "` pitch shape.")]
        #[derive(Clone, Debug, PartialEq, Citizen)]
        #[citizen(symbol = $symbol, version = 1)]
        pub struct $name {
            #[citizen(with = $field_path)]
            form: String,
        }

        impl $name {
            /// Builds the descriptor from text, normalizing it to canonical form.
            pub fn from_text(value: &str) -> Result<Self> {
                Ok(Self {
                    form: $canonical(value)?,
                })
            }

            /// Returns the stored canonical text form.
            pub fn as_text(&self) -> &str {
                &self.form
            }

            /// Returns the citizen read-construct [`Expr`] for `value` after
            /// canonicalizing it.
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
                Self::from_text(&default_form).expect("default citizen pitch form should be valid")
            }
        }

        #[doc = concat!("Returns the class [`Symbol`] for the `", $symbol, "` pitch shape.")]
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
                        "pitch citizen form must be a string, found {}",
                        super::expr_kind(other)
                    ))),
                }
            }
        }
    };
}

text_citizen!(
    PitchDescriptor,
    "pitch/Pitch",
    pitch_class_symbol,
    pitch_form,
    "pitch_form",
    "C4",
    canonical_pitch
);
text_citizen!(
    PitchIntervalDescriptor,
    "pitch/Interval",
    pitch_interval_class_symbol,
    interval_form,
    "interval_form",
    "P5",
    canonical_interval
);
text_citizen!(
    PitchClassMaskDescriptor,
    "pitch/PitchClassMask",
    pitch_class_mask_class_symbol,
    pitch_class_mask_form,
    "pitch_class_mask_form",
    format!("{}(PitchClassMask 145)", "#"),
    canonical_pitch_class_mask
);
text_citizen!(
    PitchScaleDescriptor,
    "pitch/Scale",
    pitch_scale_class_symbol,
    scale_form,
    "scale_form",
    "C:major",
    canonical_scale
);
text_citizen!(
    PitchChordDescriptor,
    "pitch/Chord",
    pitch_chord_class_symbol,
    chord_form,
    "chord_form",
    "C4,E4,G4",
    canonical_chord
);

impl PitchDescriptor {
    /// Decodes the descriptor's text form into a [`Pitch`].
    pub fn pitch(&self) -> Result<Pitch> {
        decode_pitch(&self.form).map_err(codec_error)
    }
}

impl PitchIntervalDescriptor {
    /// Decodes the descriptor's text form into an [`Interval`].
    pub fn interval(&self) -> Result<Interval> {
        decode_interval(&self.form).map_err(codec_error)
    }
}

impl PitchClassMaskDescriptor {
    /// Decodes the descriptor's text form into a [`PitchClassMask`].
    pub fn mask(&self) -> Result<PitchClassMask> {
        decode_pitch_class_mask(&self.form).map_err(codec_error)
    }
}

impl PitchScaleDescriptor {
    /// Decodes the descriptor's text form into a [`Scale`].
    pub fn scale(&self) -> Result<Scale> {
        decode_scale(&self.form).map_err(codec_error)
    }
}

impl PitchChordDescriptor {
    /// Decodes the descriptor's text form into a [`Chord`].
    pub fn chord(&self) -> Result<Chord> {
        decode_chord(&self.form).map_err(codec_error)
    }
}

fn canonical_pitch(value: &str) -> Result<String> {
    decode_pitch(value).map(encode_pitch).map_err(codec_error)
}

fn canonical_interval(value: &str) -> Result<String> {
    decode_interval(value)
        .map(encode_interval)
        .map_err(codec_error)
}

fn canonical_pitch_class_mask(value: &str) -> Result<String> {
    decode_pitch_class_mask(value)
        .map(encode_pitch_class_mask)
        .map_err(codec_error)
}

fn canonical_scale(value: &str) -> Result<String> {
    decode_scale(value).map(encode_scale).map_err(codec_error)
}

fn canonical_chord(value: &str) -> Result<String> {
    decode_chord(value)
        .map(|chord| encode_chord(&chord))
        .map_err(codec_error)
}

fn codec_error(error: PitchShapeError) -> Error {
    Error::Eval(format!("invalid pitch citizen form: {error}"))
}

use sim_value::kind::expr_kind;
