use std::collections::BTreeSet;

use crate::{ChordTemplate, HarmonyError, harmony_model::validate_id};

/// Ordered cadence template. Adjacent templates overlap only when flattened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateChain {
    /// Stable template identity.
    pub id: String,
    /// Chords concatenated in full inside this template.
    pub chords: Vec<ChordTemplate>,
}

impl TemplateChain {
    /// Builds a template containing at least two chords.
    pub fn new(id: impl Into<String>, chords: Vec<ChordTemplate>) -> Result<Self, HarmonyError> {
        let chain = Self {
            id: id.into(),
            chords,
        };
        chain.validate()?;
        Ok(chain)
    }

    /// Validates the catalog's strict template length and every chord.
    pub fn validate(&self) -> Result<(), HarmonyError> {
        validate_id(&self.id)?;
        if self.chords.len() < 2 {
            return Err(HarmonyError::InvalidField {
                field: "template.chords",
                reason: "a template chain must contain at least two chords".to_owned(),
            });
        }
        for chord in &self.chords {
            chord.validate()?;
        }
        Ok(())
    }

    /// Flattens connected templates, dropping each later template's joint chord.
    pub fn flatten_connected(chains: &[Self]) -> Result<Vec<ChordTemplate>, HarmonyError> {
        let Some(first) = chains.first() else {
            return Ok(Vec::new());
        };
        let mut flattened = first.chords.clone();
        for pair in chains.windows(2) {
            let left = pair[0]
                .chords
                .last()
                .ok_or(HarmonyError::Empty("template"))?;
            let right = pair[1]
                .chords
                .first()
                .ok_or(HarmonyError::Empty("template"))?;
            if left.pitch_set()? != right.pitch_set()? {
                return Err(HarmonyError::InvalidField {
                    field: "template.connection",
                    reason: format!("{} does not connect to {}", pair[0].id, pair[1].id),
                });
            }
            flattened.extend(pair[1].chords.iter().skip(1).cloned());
        }
        Ok(flattened)
    }
}

/// Declarative operation that produced a materialized [`ChordPalette`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteAlgebra {
    /// Entries and templates were supplied literally.
    Explicit,
    /// Alternatives were unioned in source order.
    Alternative {
        /// Source palette ids.
        sources: Vec<String>,
    },
    /// Template alternatives were combined by Cartesian concatenation.
    Chain {
        /// Source palette ids.
        sources: Vec<String>,
    },
    /// Every source entry and template was transposed by the listed offsets.
    Transpose {
        /// Source palette id.
        source: String,
        /// Semitone offsets in output order.
        offsets: Vec<i32>,
    },
}

/// Materialized chord alternatives and cadence templates with algebra provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordPalette {
    /// Stable palette identity.
    pub id: String,
    /// Atomic chord alternatives in deterministic order.
    pub entries: Vec<ChordTemplate>,
    /// Cadence-template alternatives in deterministic order.
    pub templates: Vec<TemplateChain>,
    /// Operation that produced the materialized contents.
    pub algebra: PaletteAlgebra,
}

impl ChordPalette {
    /// Builds an explicit palette.
    pub fn explicit(
        id: impl Into<String>,
        entries: Vec<ChordTemplate>,
        templates: Vec<TemplateChain>,
    ) -> Result<Self, HarmonyError> {
        let palette = Self {
            id: id.into(),
            entries,
            templates,
            algebra: PaletteAlgebra::Explicit,
        };
        palette.validate()?;
        Ok(palette)
    }

    /// Unions palette alternatives while retaining first occurrence order.
    pub fn alternative(id: impl Into<String>, sources: &[Self]) -> Result<Self, HarmonyError> {
        if sources.is_empty() {
            return Err(HarmonyError::Empty("palette alternatives"));
        }
        let mut entries = Vec::new();
        let mut templates = Vec::new();
        for source in sources {
            for entry in &source.entries {
                if !entries.contains(entry) {
                    entries.push(entry.clone());
                }
            }
            for template in &source.templates {
                if !templates.contains(template) {
                    templates.push(template.clone());
                }
            }
        }
        let palette = Self {
            id: id.into(),
            entries,
            templates,
            algebra: PaletteAlgebra::Alternative {
                sources: sources.iter().map(|source| source.id.clone()).collect(),
            },
        };
        palette.validate()?;
        Ok(palette)
    }

    /// Distributes full template concatenation across palette alternatives.
    pub fn chain(id: impl Into<String>, sources: &[Self]) -> Result<Self, HarmonyError> {
        if sources.len() < 2 {
            return Err(HarmonyError::InvalidField {
                field: "palette.chain",
                reason: "chain algebra needs at least two palettes".to_owned(),
            });
        }
        let mut products = vec![Vec::new()];
        for source in sources {
            let alternatives = source.as_template_alternatives();
            let mut next = Vec::new();
            for prefix in &products {
                for alternative in &alternatives {
                    let mut chords = prefix.clone();
                    chords.extend(alternative.chords.clone());
                    next.push(chords);
                }
            }
            products = next;
        }
        let id = id.into();
        let templates = products
            .into_iter()
            .enumerate()
            .map(|(index, chords)| TemplateChain::new(format!("{id}/template/{index}"), chords))
            .collect::<Result<Vec<_>, _>>()?;
        let palette = Self {
            id,
            entries: Vec::new(),
            templates,
            algebra: PaletteAlgebra::Chain {
                sources: sources.iter().map(|source| source.id.clone()).collect(),
            },
        };
        palette.validate()?;
        Ok(palette)
    }

    /// Materializes each requested transposition without symmetry duplicates.
    pub fn transpose(
        id: impl Into<String>,
        source: &Self,
        offsets: &[i32],
    ) -> Result<Self, HarmonyError> {
        if offsets.is_empty() {
            return Err(HarmonyError::Empty("palette transpositions"));
        }
        let id = id.into();
        let mut entries = Vec::new();
        let mut templates = Vec::new();
        for offset in offsets {
            for entry in &source.entries {
                let candidate =
                    entry.transpose(format!("{}/{}/{}", id, entry.id, offset), *offset)?;
                if !entries.iter().any(|known: &ChordTemplate| {
                    known.pitch_set().ok() == candidate.pitch_set().ok()
                }) {
                    entries.push(candidate);
                }
            }
            for template in &source.templates {
                let chords = template
                    .chords
                    .iter()
                    .map(|chord| {
                        chord.transpose(
                            format!("{}/{}/{}/{}", id, template.id, chord.id, offset),
                            *offset,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                templates.push(TemplateChain::new(
                    format!("{}/{}/{}", id, template.id, offset),
                    chords,
                )?);
            }
        }
        let palette = Self {
            id,
            entries,
            templates,
            algebra: PaletteAlgebra::Transpose {
                source: source.id.clone(),
                offsets: offsets.to_vec(),
            },
        };
        palette.validate()?;
        Ok(palette)
    }

    /// Validates identifiers, contents, and operation provenance.
    pub fn validate(&self) -> Result<(), HarmonyError> {
        validate_id(&self.id)?;
        if self.entries.is_empty() && self.templates.is_empty() {
            return Err(HarmonyError::Empty("palette"));
        }
        let mut ids = BTreeSet::new();
        for entry in &self.entries {
            entry.validate()?;
            if !ids.insert(&entry.id) {
                return Err(HarmonyError::InvalidId(entry.id.clone()));
            }
        }
        for template in &self.templates {
            template.validate()?;
            if !ids.insert(&template.id) {
                return Err(HarmonyError::InvalidId(template.id.clone()));
            }
        }
        match &self.algebra {
            PaletteAlgebra::Explicit => {}
            PaletteAlgebra::Alternative { sources } | PaletteAlgebra::Chain { sources } => {
                validate_nonempty_strings("palette sources", sources)?;
            }
            PaletteAlgebra::Transpose { source, offsets } => {
                validate_id(source)?;
                if offsets.is_empty() {
                    return Err(HarmonyError::Empty("palette transpositions"));
                }
            }
        }
        Ok(())
    }

    fn as_template_alternatives(&self) -> Vec<TemplateChain> {
        let mut alternatives = self.templates.clone();
        alternatives.extend(self.entries.iter().map(|entry| TemplateChain {
            id: format!("{}/single/{}", self.id, entry.id),
            chords: vec![entry.clone()],
        }));
        alternatives
    }
}

fn validate_nonempty_strings(field: &'static str, values: &[String]) -> Result<(), HarmonyError> {
    if values.is_empty() {
        return Err(HarmonyError::Empty(field));
    }
    for value in values {
        validate_id(value)?;
    }
    Ok(())
}
