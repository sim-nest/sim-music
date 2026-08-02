use sim_kernel::Expr;
use sim_lib_pitch_ratio::PitchRatio;

use crate::harmony_expr_support::*;
use crate::harmony_rule_expr::{rule_set_from_expr, rule_set_to_expr};
use crate::{
    ChordPalette, ChordTemplate, ChordTemplateSource, Fingering, HarmonyError, HarmonyProgram,
    HarmonyRenderProfile, PaletteAlgebra, TemplateChain, VoicingChange, VoicingChangePalette,
    VoicingPolicy,
};

impl HarmonyProgram {
    /// Encodes this complete program as codec-neutral SIM expression data.
    pub fn to_expr(&self) -> Expr {
        tagged(
            "program",
            vec![
                qualified_entry("id", string(&self.id)),
                qualified_entry("palette", self.palette.to_expr()),
                qualified_entry("rules", rule_set_to_expr(&self.rules)),
                qualified_entry("voicing-changes", self.voicing_changes.to_expr()),
                qualified_entry("render", render_to_expr(&self.render)),
            ],
        )
    }

    /// Decodes and validates a complete program from SIM expression data.
    pub fn from_expr(expr: &Expr) -> Result<Self, HarmonyError> {
        require_tag(expr, "program")?;
        let program = Self {
            id: text(required(expr, "id")?, "program.id")?.to_owned(),
            palette: ChordPalette::from_expr(required(expr, "palette")?)?,
            rules: rule_set_from_expr(required(expr, "rules")?)?,
            voicing_changes: VoicingChangePalette::from_expr(required(expr, "voicing-changes")?)?,
            render: render_from_expr(required(expr, "render")?)?,
        };
        program.validate()?;
        Ok(program)
    }
}

impl ChordPalette {
    /// Encodes a materialized palette and its algebra provenance.
    pub fn to_expr(&self) -> Expr {
        tagged(
            "palette",
            vec![
                qualified_entry("id", string(&self.id)),
                qualified_entry(
                    "entries",
                    vector(self.entries.iter().map(chord_to_expr).collect()),
                ),
                qualified_entry(
                    "templates",
                    vector(self.templates.iter().map(template_to_expr).collect()),
                ),
                qualified_entry("algebra", algebra_to_expr(&self.algebra)),
            ],
        )
    }

    /// Decodes and validates a materialized palette.
    pub fn from_expr(expr: &Expr) -> Result<Self, HarmonyError> {
        require_tag(expr, "palette")?;
        let palette = Self {
            id: text(required(expr, "id")?, "palette.id")?.to_owned(),
            entries: sequence(required(expr, "entries")?, "palette.entries")?
                .iter()
                .map(chord_from_expr)
                .collect::<Result<Vec<_>, _>>()?,
            templates: sequence(required(expr, "templates")?, "palette.templates")?
                .iter()
                .map(template_from_expr)
                .collect::<Result<Vec<_>, _>>()?,
            algebra: algebra_from_expr(required(expr, "algebra")?)?,
        };
        palette.validate()?;
        Ok(palette)
    }
}

impl VoicingChangePalette {
    /// Encodes every materialized voicing change as expression data.
    pub fn to_expr(&self) -> Expr {
        tagged(
            "voicing-change-palette",
            vec![
                qualified_entry("id", string(&self.id)),
                qualified_entry(
                    "entries",
                    vector(self.entries.iter().map(voicing_change_to_expr).collect()),
                ),
            ],
        )
    }

    /// Decodes and validates a voicing-change palette.
    pub fn from_expr(expr: &Expr) -> Result<Self, HarmonyError> {
        require_tag(expr, "voicing-change-palette")?;
        let palette = Self {
            id: text(required(expr, "id")?, "voicing-change-palette.id")?.to_owned(),
            entries: sequence(required(expr, "entries")?, "voicing-change-palette.entries")?
                .iter()
                .map(voicing_change_from_expr)
                .collect::<Result<Vec<_>, _>>()?,
        };
        palette.validate()?;
        Ok(palette)
    }
}

fn chord_to_expr(chord: &ChordTemplate) -> Expr {
    tagged(
        "chord",
        vec![
            qualified_entry("id", string(&chord.id)),
            qualified_entry("source", chord_source_to_expr(&chord.source)),
            qualified_entry("voicing", voicing_to_expr(chord.voicing)),
            qualified_entry(
                "ratios",
                vector(
                    chord
                        .ratios
                        .iter()
                        .map(|ratio| {
                            tagged(
                                "ratio",
                                vec![
                                    qualified_entry("numerator", scalar(ratio.numerator())),
                                    qualified_entry("denominator", scalar(ratio.denominator())),
                                ],
                            )
                        })
                        .collect(),
                ),
            ),
        ],
    )
}

fn chord_from_expr(expr: &Expr) -> Result<ChordTemplate, HarmonyError> {
    require_tag(expr, "chord")?;
    let ratios = sequence(required(expr, "ratios")?, "chord.ratios")?
        .iter()
        .map(|expr| {
            require_tag(expr, "ratio")?;
            PitchRatio::new(
                parse(required(expr, "numerator")?, "ratio.numerator")?,
                parse(required(expr, "denominator")?, "ratio.denominator")?,
            )
            .map_err(|error| HarmonyError::InvalidField {
                field: "ratio",
                reason: error.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let chord = ChordTemplate {
        id: text(required(expr, "id")?, "chord.id")?.to_owned(),
        source: chord_source_from_expr(required(expr, "source")?)?,
        voicing: voicing_from_expr(required(expr, "voicing")?)?,
        ratios,
    };
    chord.validate()?;
    Ok(chord)
}

fn chord_source_to_expr(source: &ChordTemplateSource) -> Expr {
    match source {
        ChordTemplateSource::Symbol { symbol, octave } => tagged(
            "source-symbol",
            vec![
                qualified_entry("symbol", string(symbol)),
                qualified_entry("octave", scalar(*octave)),
            ],
        ),
        ChordTemplateSource::Pitches { pitches } => tagged(
            "source-pitches",
            vec![qualified_entry(
                "semitones",
                vector(
                    pitches
                        .iter()
                        .map(|pitch| scalar(pitch.semitone()))
                        .collect(),
                ),
            )],
        ),
        ChordTemplateSource::PitchClasses {
            classes,
            root_octave,
        } => tagged(
            "source-pitch-classes",
            vec![
                qualified_entry(
                    "classes",
                    vector(classes.iter().map(|class| scalar(class.value())).collect()),
                ),
                qualified_entry("root-octave", scalar(*root_octave)),
            ],
        ),
        ChordTemplateSource::ScaleDegrees {
            scale,
            degrees,
            root_octave,
        } => tagged(
            "source-scale-degrees",
            vec![
                qualified_entry("scale", scale_to_expr(*scale)),
                qualified_entry(
                    "degrees",
                    vector(degrees.iter().map(|degree| scalar(*degree)).collect()),
                ),
                qualified_entry("root-octave", scalar(*root_octave)),
            ],
        ),
        ChordTemplateSource::PitchSet {
            mask,
            root,
            root_octave,
        } => tagged(
            "source-pitch-set",
            vec![
                qualified_entry("mask", scalar(mask.bits())),
                qualified_entry("root", scalar(root.value())),
                qualified_entry("root-octave", scalar(*root_octave)),
            ],
        ),
    }
}

fn chord_source_from_expr(expr: &Expr) -> Result<ChordTemplateSource, HarmonyError> {
    match tag(expr)? {
        "source-symbol" => Ok(ChordTemplateSource::Symbol {
            symbol: text(required(expr, "symbol")?, "source.symbol")?.to_owned(),
            octave: parse(required(expr, "octave")?, "source.octave")?,
        }),
        "source-pitches" => Ok(ChordTemplateSource::Pitches {
            pitches: scalars::<i32>(required(expr, "semitones")?, "source.semitones")?
                .into_iter()
                .map(sim_lib_pitch_core::Pitch::from_semitone)
                .collect(),
        }),
        "source-pitch-classes" => Ok(ChordTemplateSource::PitchClasses {
            classes: pitch_classes(required(expr, "classes")?)?,
            root_octave: parse(required(expr, "root-octave")?, "source.root-octave")?,
        }),
        "source-scale-degrees" => Ok(ChordTemplateSource::ScaleDegrees {
            scale: scale_from_expr(required(expr, "scale")?)?,
            degrees: scalars(required(expr, "degrees")?, "source.degrees")?,
            root_octave: parse(required(expr, "root-octave")?, "source.root-octave")?,
        }),
        "source-pitch-set" => Ok(ChordTemplateSource::PitchSet {
            mask: mask(required(expr, "mask")?, "source.mask")?,
            root: pitch_class(required(expr, "root")?, "source.root")?,
            root_octave: parse(required(expr, "root-octave")?, "source.root-octave")?,
        }),
        other => Err(invalid(format!("unknown chord source {other}"))),
    }
}

fn voicing_to_expr(voicing: VoicingPolicy) -> Expr {
    match voicing {
        VoicingPolicy::Preserve => tagged("voicing-preserve", Vec::new()),
        VoicingPolicy::Closed => tagged("voicing-closed", Vec::new()),
        VoicingPolicy::Open { spread } => tagged(
            "voicing-open",
            vec![qualified_entry("spread", scalar(spread))],
        ),
        VoicingPolicy::Drop {
            voice_index_from_top,
            octaves,
        } => tagged(
            "voicing-drop",
            vec![
                qualified_entry("voice-index-from-top", scalar(voice_index_from_top)),
                qualified_entry("octaves", scalar(octaves)),
            ],
        ),
    }
}

fn voicing_from_expr(expr: &Expr) -> Result<VoicingPolicy, HarmonyError> {
    match tag(expr)? {
        "voicing-preserve" => Ok(VoicingPolicy::Preserve),
        "voicing-closed" => Ok(VoicingPolicy::Closed),
        "voicing-open" => Ok(VoicingPolicy::Open {
            spread: parse(required(expr, "spread")?, "voicing.spread")?,
        }),
        "voicing-drop" => Ok(VoicingPolicy::Drop {
            voice_index_from_top: parse(
                required(expr, "voice-index-from-top")?,
                "voicing.voice-index-from-top",
            )?,
            octaves: parse(required(expr, "octaves")?, "voicing.octaves")?,
        }),
        other => Err(invalid(format!("unknown voicing {other}"))),
    }
}

fn template_to_expr(template: &TemplateChain) -> Expr {
    tagged(
        "template",
        vec![
            qualified_entry("id", string(&template.id)),
            qualified_entry(
                "chords",
                vector(template.chords.iter().map(chord_to_expr).collect()),
            ),
        ],
    )
}

fn template_from_expr(expr: &Expr) -> Result<TemplateChain, HarmonyError> {
    require_tag(expr, "template")?;
    TemplateChain::new(
        text(required(expr, "id")?, "template.id")?,
        sequence(required(expr, "chords")?, "template.chords")?
            .iter()
            .map(chord_from_expr)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn algebra_to_expr(algebra: &PaletteAlgebra) -> Expr {
    match algebra {
        PaletteAlgebra::Explicit => tagged("algebra-explicit", Vec::new()),
        PaletteAlgebra::Alternative { sources } => tagged(
            "algebra-alternative",
            vec![qualified_entry(
                "sources",
                vector(sources.iter().map(|source| string(source)).collect()),
            )],
        ),
        PaletteAlgebra::Chain { sources } => tagged(
            "algebra-chain",
            vec![qualified_entry(
                "sources",
                vector(sources.iter().map(|source| string(source)).collect()),
            )],
        ),
        PaletteAlgebra::Transpose { source, offsets } => tagged(
            "algebra-transpose",
            vec![
                qualified_entry("source", string(source)),
                qualified_entry(
                    "offsets",
                    vector(offsets.iter().map(|offset| scalar(*offset)).collect()),
                ),
            ],
        ),
    }
}

fn algebra_from_expr(expr: &Expr) -> Result<PaletteAlgebra, HarmonyError> {
    match tag(expr)? {
        "algebra-explicit" => Ok(PaletteAlgebra::Explicit),
        "algebra-alternative" => Ok(PaletteAlgebra::Alternative {
            sources: strings(required(expr, "sources")?, "algebra.sources")?,
        }),
        "algebra-chain" => Ok(PaletteAlgebra::Chain {
            sources: strings(required(expr, "sources")?, "algebra.sources")?,
        }),
        "algebra-transpose" => Ok(PaletteAlgebra::Transpose {
            source: text(required(expr, "source")?, "algebra.source")?.to_owned(),
            offsets: scalars(required(expr, "offsets")?, "algebra.offsets")?,
        }),
        other => Err(invalid(format!("unknown palette algebra {other}"))),
    }
}

fn voicing_change_to_expr(change: &VoicingChange) -> Expr {
    tagged(
        "voicing-change",
        vec![
            qualified_entry("id", string(&change.id)),
            qualified_entry("source", string(&change.source)),
            qualified_entry("target", string(&change.target)),
            qualified_entry(
                "leading",
                vector(
                    change
                        .leading
                        .indices
                        .iter()
                        .map(|index| scalar(*index))
                        .collect(),
                ),
            ),
            qualified_entry("cost", scalar(change.cost)),
            qualified_entry("octave", scalar(change.octave)),
        ],
    )
}

fn voicing_change_from_expr(expr: &Expr) -> Result<VoicingChange, HarmonyError> {
    require_tag(expr, "voicing-change")?;
    Ok(VoicingChange {
        id: text(required(expr, "id")?, "voicing-change.id")?.to_owned(),
        source: text(required(expr, "source")?, "voicing-change.source")?.to_owned(),
        target: text(required(expr, "target")?, "voicing-change.target")?.to_owned(),
        leading: Fingering {
            indices: scalars(required(expr, "leading")?, "voicing-change.leading")?,
        },
        cost: parse(required(expr, "cost")?, "voicing-change.cost")?,
        octave: parse(required(expr, "octave")?, "voicing-change.octave")?,
    })
}

fn render_to_expr(render: &HarmonyRenderProfile) -> Expr {
    tagged(
        "render-profile",
        vec![
            qualified_entry("id", string(&render.id)),
            qualified_entry("chord-transpose", scalar(render.chord_transpose)),
            qualified_entry("melody-transpose", scalar(render.melody_transpose)),
            qualified_entry("duration-multiplier", scalar(render.duration_multiplier)),
            qualified_entry("chord-program", scalar(render.chord_program)),
            qualified_entry("melody-program", scalar(render.melody_program)),
            qualified_entry("tempo-bpm", scalar(render.tempo_bpm)),
            qualified_entry(
                "time-signature",
                vector(vec![
                    scalar(render.time_signature.0),
                    scalar(render.time_signature.1),
                ]),
            ),
        ],
    )
}

fn render_from_expr(expr: &Expr) -> Result<HarmonyRenderProfile, HarmonyError> {
    require_tag(expr, "render-profile")?;
    let signature: Vec<u8> = scalars(required(expr, "time-signature")?, "render.time-signature")?;
    if signature.len() != 2 {
        return Err(invalid("render time-signature must contain two values"));
    }
    Ok(HarmonyRenderProfile {
        id: text(required(expr, "id")?, "render.id")?.to_owned(),
        chord_transpose: parse(required(expr, "chord-transpose")?, "render.chord-transpose")?,
        melody_transpose: parse(
            required(expr, "melody-transpose")?,
            "render.melody-transpose",
        )?,
        duration_multiplier: parse(
            required(expr, "duration-multiplier")?,
            "render.duration-multiplier",
        )?,
        chord_program: parse(required(expr, "chord-program")?, "render.chord-program")?,
        melody_program: parse(required(expr, "melody-program")?, "render.melody-program")?,
        tempo_bpm: parse(required(expr, "tempo-bpm")?, "render.tempo-bpm")?,
        time_signature: (signature[0], signature[1]),
    })
}
