use sim_citizen_derive::Citizen;
use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_music_core::{Chord, Melody, Music, Note, Par, Score, Seq};

use crate::{
    MusicShapeError, decode_chord, decode_melody, decode_music, decode_note, decode_score,
    encode_chord, encode_melody, encode_note, encode_par, encode_score, encode_seq,
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
        #[doc = concat!("Citizen descriptor wrapping the canonical `", $symbol, "` text form.")]
        ///
        /// Stores a single canonical `#(...)` form string and exposes it as a
        /// registered citizen class for read-construct interchange.
        #[derive(Clone, Debug, PartialEq, Citizen)]
        #[citizen(symbol = $symbol, version = 1)]
        pub struct $name {
            #[citizen(with = $field_path)]
            form: String,
        }

        impl $name {
            /// Builds a descriptor from text, canonicalizing it through the codec.
            ///
            /// Returns an error when the text is not a valid form for this class.
            pub fn from_text(value: &str) -> Result<Self> {
                Ok(Self {
                    form: $canonical(value)?,
                })
            }

            /// Returns the stored canonical `#(...)` form text.
            pub fn as_text(&self) -> &str {
                &self.form
            }

            /// Canonicalizes `value` and wraps it in a citizen read-construct expression.
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
                Self::from_text(&default_form).expect("default citizen music form should be valid")
            }
        }

        #[doc = concat!("Returns the class symbol `", $symbol, "` for this descriptor.")]
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
                        "music citizen form must be a string, found {}",
                        super::expr_kind(other)
                    ))),
                }
            }
        }
    };
}

text_citizen!(
    MusicNoteDescriptor,
    "music/Note",
    music_note_class_symbol,
    note_form,
    "note_form",
    format!(
        "{}(Note dur=1/4 pitch=C4 vel=100 channel=0 articulation=Normal)",
        "#"
    ),
    canonical_note
);
text_citizen!(
    MusicSeqDescriptor,
    "music/Seq",
    music_seq_class_symbol,
    seq_form,
    "seq_form",
    format!("{}(Seq children=[])", "#"),
    canonical_seq
);
text_citizen!(
    MusicParDescriptor,
    "music/Par",
    music_par_class_symbol,
    par_form,
    "par_form",
    format!("{}(Par children=[])", "#"),
    canonical_par
);
text_citizen!(
    MusicChordDescriptor,
    "music/Chord",
    music_chord_class_symbol,
    chord_form,
    "chord_form",
    format!(
        "{}(Chord dur=1/4 symbol=\"C\" pitches=[C4,E4,G4] vel=100 channel=0)",
        "#"
    ),
    canonical_chord
);
text_citizen!(
    MusicMelodyDescriptor,
    "music/Melody",
    music_melody_class_symbol,
    melody_form,
    "melody_form",
    format!("{}(Melody items=[])", "#"),
    canonical_melody
);
text_citizen!(
    MusicScoreDescriptor,
    "music/Score",
    music_score_class_symbol,
    score_form,
    "score_form",
    format!(
        "{}(Score tempo=120 time_sig=4/4 key=none body={}(Note dur=1/4 pitch=C4 vel=100 channel=0 articulation=Normal))",
        "#", "#"
    ),
    canonical_score
);

impl MusicNoteDescriptor {
    /// Decodes the stored form into a `Note` value.
    pub fn note(&self) -> Result<Note> {
        decode_note(&self.form).map_err(codec_error)
    }
}

impl MusicSeqDescriptor {
    /// Decodes the stored form into a `Seq` value.
    pub fn seq(&self) -> Result<Seq> {
        match decode_music(&self.form).map_err(codec_error)? {
            Music::Seq(seq) => Ok(seq),
            _ => Err(wrong_variant("Seq")),
        }
    }
}

impl MusicParDescriptor {
    /// Decodes the stored form into a `Par` value.
    pub fn par(&self) -> Result<Par> {
        match decode_music(&self.form).map_err(codec_error)? {
            Music::Par(par) => Ok(par),
            _ => Err(wrong_variant("Par")),
        }
    }
}

impl MusicChordDescriptor {
    /// Decodes the stored form into a `Chord` value.
    pub fn chord(&self) -> Result<Chord> {
        decode_chord(&self.form).map_err(codec_error)
    }
}

impl MusicMelodyDescriptor {
    /// Decodes the stored form into a `Melody` value.
    pub fn melody(&self) -> Result<Melody> {
        decode_melody(&self.form).map_err(codec_error)
    }
}

impl MusicScoreDescriptor {
    /// Decodes the stored form into a `Score` value.
    pub fn score(&self) -> Result<Score> {
        decode_score(&self.form).map_err(codec_error)
    }
}

fn canonical_note(value: &str) -> Result<String> {
    decode_note(value)
        .map(|note| encode_note(&note))
        .map_err(codec_error)
}

fn canonical_seq(value: &str) -> Result<String> {
    match decode_music(value).map_err(codec_error)? {
        Music::Seq(seq) => encode_seq(&seq).map_err(codec_error),
        _ => Err(wrong_variant("Seq")),
    }
}

fn canonical_par(value: &str) -> Result<String> {
    match decode_music(value).map_err(codec_error)? {
        Music::Par(par) => encode_par(&par).map_err(codec_error),
        _ => Err(wrong_variant("Par")),
    }
}

fn canonical_chord(value: &str) -> Result<String> {
    decode_chord(value)
        .map(|chord| encode_chord(&chord))
        .map_err(codec_error)
}

fn canonical_melody(value: &str) -> Result<String> {
    decode_melody(value)
        .map(|melody| encode_melody(&melody))
        .map_err(codec_error)
}

fn canonical_score(value: &str) -> Result<String> {
    let score = decode_score(value).map_err(codec_error)?;
    encode_score(&score).map_err(codec_error)
}

fn codec_error(error: MusicShapeError) -> Error {
    Error::Eval(format!("invalid music citizen form: {error}"))
}

fn wrong_variant(expected: &str) -> Error {
    Error::Eval(format!("music citizen expected {expected} form"))
}

use sim_value::kind::expr_kind;
