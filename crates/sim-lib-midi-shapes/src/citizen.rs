use sim_citizen_derive::Citizen;
use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_midi_core::{ChannelMessage, MetaEvent, MidiEvent};
use sim_lib_midi_smf::{SmfFile, SmfTrack};

use crate::{
    MidiShapeError, decode_channel_message, decode_meta_event, decode_midi_event, decode_smf_file,
    decode_smf_track, encode_channel_message, encode_meta_event, encode_midi_event,
    encode_smf_file, encode_smf_track,
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
        #[doc = concat!("Citizen descriptor wrapping the canonical `", $symbol, "` string form.")]
        #[derive(Clone, Debug, PartialEq, Citizen)]
        #[citizen(symbol = $symbol, version = 1)]
        pub struct $name {
            #[citizen(with = $field_path)]
            form: String,
        }

        impl $name {
            /// Builds a descriptor by canonicalising `value`, failing if it is
            /// not a valid MIDI form.
            pub fn from_text(value: &str) -> Result<Self> {
                Ok(Self {
                    form: $canonical(value)?,
                })
            }

            /// Returns the canonical string form.
            pub fn as_text(&self) -> &str {
                &self.form
            }

            /// Canonicalises `value` and returns the citizen read-construct
            /// expression that rebuilds this descriptor at runtime.
            pub fn read_construct_expr_from_text(value: &str) -> Result<Expr> {
                Ok(read_construct_expr($symbol_fn(), $canonical(value)?))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                let default_form = $default.to_string();
                Self::from_text(&default_form).expect("default citizen MIDI form should be valid")
            }
        }

        #[doc = concat!("Returns the citizen class symbol `", $symbol, "`.")]
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
                        "MIDI citizen form must be a string, found {}",
                        super::expr_kind(other)
                    ))),
                }
            }
        }
    };
}

text_citizen!(
    MidiEventDescriptor,
    "midi/MidiEvent",
    midi_event_class_symbol,
    midi_event_form,
    "midi_event_form",
    format!(
        "{}(MidiEvent {}(TickTime 0 480) {}(Channel NoteOn 0 60 100))",
        "#", "#", "#"
    ),
    canonical_midi_event
);
text_citizen!(
    MidiChannelMessageDescriptor,
    "midi/ChannelMessage",
    midi_channel_message_class_symbol,
    channel_message_form,
    "channel_message_form",
    format!("{}(Channel NoteOn 0 60 100)", "#"),
    canonical_channel_message
);
text_citizen!(
    MidiMetaEventDescriptor,
    "midi/MetaEvent",
    midi_meta_event_class_symbol,
    meta_event_form,
    "meta_event_form",
    format!("{}(Meta Tempo 500000)", "#"),
    canonical_meta_event
);
text_citizen!(
    MidiSmfTrackDescriptor,
    "midi/SmfTrack",
    midi_smf_track_class_symbol,
    smf_track_form,
    "smf_track_form",
    format!("{}(SmfTrack)", "#"),
    canonical_smf_track
);
text_citizen!(
    MidiSmfFileDescriptor,
    "midi/SmfFile",
    midi_smf_file_class_symbol,
    smf_file_form,
    "smf_file_form",
    format!("{}(SmfFile SingleTrack 480)", "#"),
    canonical_smf_file
);

impl MidiEventDescriptor {
    /// Decodes the descriptor's form into a [`MidiEvent`].
    pub fn event(&self) -> Result<MidiEvent> {
        decode_midi_event(&self.form).map_err(codec_error)
    }
}

impl MidiChannelMessageDescriptor {
    /// Decodes the descriptor's form into a [`ChannelMessage`].
    pub fn message(&self) -> Result<ChannelMessage> {
        decode_channel_message(&self.form).map_err(codec_error)
    }
}

impl MidiMetaEventDescriptor {
    /// Decodes the descriptor's form into a [`MetaEvent`].
    pub fn event(&self) -> Result<MetaEvent> {
        decode_meta_event(&self.form).map_err(codec_error)
    }
}

impl MidiSmfTrackDescriptor {
    /// Decodes the descriptor's form into an [`SmfTrack`].
    pub fn track(&self) -> Result<SmfTrack> {
        decode_smf_track(&self.form).map_err(codec_error)
    }
}

impl MidiSmfFileDescriptor {
    /// Decodes the descriptor's form into an [`SmfFile`].
    pub fn file(&self) -> Result<SmfFile> {
        decode_smf_file(&self.form).map_err(codec_error)
    }
}

fn canonical_midi_event(value: &str) -> Result<String> {
    decode_midi_event(value)
        .map(|event| encode_midi_event(&event))
        .map_err(codec_error)
}

fn canonical_channel_message(value: &str) -> Result<String> {
    decode_channel_message(value)
        .map(encode_channel_message)
        .map_err(codec_error)
}

fn canonical_meta_event(value: &str) -> Result<String> {
    decode_meta_event(value)
        .map(|event| encode_meta_event(&event))
        .map_err(codec_error)
}

fn canonical_smf_track(value: &str) -> Result<String> {
    decode_smf_track(value)
        .map(|track| encode_smf_track(&track))
        .map_err(codec_error)
}

fn canonical_smf_file(value: &str) -> Result<String> {
    decode_smf_file(value)
        .map(|file| encode_smf_file(&file))
        .map_err(codec_error)
}

fn read_construct_expr(class: Symbol, form: String) -> Expr {
    Expr::Extension {
        tag: Symbol::qualified("citizen", "read-construct"),
        payload: Box::new(Expr::Vector(vec![
            Expr::Symbol(class),
            Expr::Symbol(Symbol::new("v1")),
            Expr::String(form),
        ])),
    }
}

fn codec_error(error: MidiShapeError) -> Error {
    Error::Eval(format!("invalid MIDI citizen form: {error}"))
}

use sim_value::kind::expr_kind;
