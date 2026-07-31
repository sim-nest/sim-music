use std::collections::BTreeSet;

use sim_lib_discrete_graph::{
    AssignmentOperation, AssignmentPolicy, CostMatrix, min_cost_assignment,
};
use sim_lib_pitch_core::Pitch;

use crate::{ChordPalette, ChordTemplate, HarmonyError, validate_id};

/// Checked selection of chord members for ordered voices.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Fingering {
    /// Chord-member index for each output voice.
    pub indices: Vec<usize>,
}

impl Fingering {
    /// Validates indices against the chord's note count, preserving duplicates.
    pub fn new(indices: Vec<usize>, chord_note_count: usize) -> Result<Self, HarmonyError> {
        if indices.is_empty() {
            return Err(HarmonyError::Empty("fingering"));
        }
        if let Some(index) = indices
            .iter()
            .copied()
            .find(|index| *index >= chord_note_count)
        {
            return Err(HarmonyError::InvalidField {
                field: "fingering.indices",
                reason: format!("index {index} is outside {chord_note_count} chord notes"),
            });
        }
        Ok(Self { indices })
    }
}

/// Serializable minimum-cost change between two exact chord-template voicings.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VoicingChange {
    /// Stable entry identity.
    pub id: String,
    /// Source chord-template id.
    pub source: String,
    /// Target chord-template id.
    pub target: String,
    /// Target-member index selected for every source voice.
    pub leading: Fingering,
    /// Exact circular squared pitch-class cost.
    pub cost: i64,
    /// Octave size used by circular distance.
    pub octave: u16,
}

impl VoicingChange {
    /// Builds a certified, duplicate-preserving equal-cardinality voice assignment.
    pub fn between(
        id: impl Into<String>,
        source: &ChordTemplate,
        target: &ChordTemplate,
        octave: u16,
    ) -> Result<Self, HarmonyError> {
        if octave == 0 {
            return Err(HarmonyError::InvalidField {
                field: "voicing-change.octave",
                reason: "octave must be positive".to_owned(),
            });
        }
        let source_notes = source.realize()?.notes;
        let target_notes = target.realize()?.notes;
        if source_notes.len() != target_notes.len() {
            return Err(HarmonyError::InvalidField {
                field: "voicing-change.voices",
                reason: format!(
                    "source has {} voices and target has {}",
                    source_notes.len(),
                    target_notes.len()
                ),
            });
        }
        let costs = voice_costs(&source_notes, &target_notes, octave)?;
        let count = source_notes.len();
        let policy = AssignmentPolicy::new(vec![i64::MAX / 8; count], vec![i64::MAX / 8; count]);
        let assignment =
            min_cost_assignment(&costs, policy).map_err(|error| HarmonyError::InvalidField {
                field: "voicing-change.assignment",
                reason: error.to_string(),
            })?;
        let mut leading = vec![usize::MAX; count];
        for operation in &assignment.operations {
            if let AssignmentOperation::Match { source, target, .. } = operation {
                leading[*source] = *target;
            }
        }
        if leading.contains(&usize::MAX) {
            return Err(HarmonyError::InvalidField {
                field: "voicing-change.assignment",
                reason: "assignment did not match every source voice".to_owned(),
            });
        }
        Ok(Self {
            id: id.into(),
            source: source.id.clone(),
            target: target.id.clone(),
            leading: Fingering::new(leading, target_notes.len())?,
            cost: assignment.total_cost,
            octave,
        })
    }

    /// Applies the stored member mapping to target notes.
    pub fn apply(&self, target: &ChordTemplate) -> Result<Vec<Pitch>, HarmonyError> {
        if self.target != target.id {
            return Err(HarmonyError::InvalidField {
                field: "voicing-change.target",
                reason: format!("expected {}, received {}", self.target, target.id),
            });
        }
        let notes = target.realize()?.notes;
        self.leading
            .indices
            .iter()
            .map(|index| {
                notes
                    .get(*index)
                    .copied()
                    .ok_or_else(|| HarmonyError::InvalidField {
                        field: "voicing-change.leading",
                        reason: format!("target index {index} is outside {} notes", notes.len()),
                    })
            })
            .collect()
    }

    /// Validates identifiers, bounds, and duplicate-preserving mapping.
    pub fn validate(&self) -> Result<(), HarmonyError> {
        validate_id(&self.id)?;
        validate_id(&self.source)?;
        validate_id(&self.target)?;
        if self.octave == 0 || self.cost < 0 {
            return Err(HarmonyError::InvalidField {
                field: "voicing-change",
                reason: "octave must be positive and cost non-negative".to_owned(),
            });
        }
        if self.leading.indices.is_empty() {
            return Err(HarmonyError::Empty("voicing-change leading"));
        }
        let unique = self
            .leading
            .indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if unique.len() != self.leading.indices.len() {
            return Err(HarmonyError::InvalidField {
                field: "voicing-change.leading",
                reason: "equal-cardinality leading must use every target voice once".to_owned(),
            });
        }
        Ok(())
    }
}

/// Deduplicated, finite set of declarative voicing changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoicingChangePalette {
    /// Stable palette identity.
    pub id: String,
    /// Changes in deterministic source/target order.
    pub entries: Vec<VoicingChange>,
}

impl VoicingChangePalette {
    /// Builds every equal-cardinality ordered pair from a chord palette.
    pub fn from_chord_palette(
        id: impl Into<String>,
        palette: &ChordPalette,
        octave: u16,
    ) -> Result<Self, HarmonyError> {
        let id = id.into();
        let mut candidates = palette.entries.clone();
        for template in &palette.templates {
            for chord in &template.chords {
                if !candidates.contains(chord) {
                    candidates.push(chord.clone());
                }
            }
        }
        let mut entries = Vec::new();
        let mut semantic_keys = BTreeSet::new();
        for source in &candidates {
            for target in &candidates {
                if source.realize()?.notes.len() != target.realize()?.notes.len() {
                    continue;
                }
                let change = VoicingChange::between(
                    format!("{id}/{}/{}", source.id, target.id),
                    source,
                    target,
                    octave,
                )?;
                let key = (
                    source.pitch_set()?.bits(),
                    target.pitch_set()?.bits(),
                    change.leading.indices.clone(),
                );
                if semantic_keys.insert(key) {
                    entries.push(change);
                }
            }
        }
        let result = Self { id, entries };
        result.validate()?;
        Ok(result)
    }

    /// Returns an explicitly empty palette for a program with no voicing changes.
    pub fn empty(id: impl Into<String>) -> Result<Self, HarmonyError> {
        let palette = Self {
            id: id.into(),
            entries: Vec::new(),
        };
        palette.validate()?;
        Ok(palette)
    }

    /// Validates entry ids and semantic uniqueness.
    pub fn validate(&self) -> Result<(), HarmonyError> {
        validate_id(&self.id)?;
        let mut ids = BTreeSet::new();
        let mut semantic = BTreeSet::new();
        for entry in &self.entries {
            entry.validate()?;
            if !ids.insert(&entry.id) {
                return Err(HarmonyError::InvalidId(entry.id.clone()));
            }
            let key = (
                entry.source.as_str(),
                entry.target.as_str(),
                entry.leading.indices.as_slice(),
            );
            if !semantic.insert(key) {
                return Err(HarmonyError::InvalidField {
                    field: "voicing-change-palette.entries",
                    reason: "duplicate semantic change".to_owned(),
                });
            }
        }
        Ok(())
    }
}

pub(crate) fn circular_squared_cost(left: i32, right: i32, octave: u16) -> i64 {
    let octave = i32::from(octave);
    let ascending = (right - left).rem_euclid(octave);
    let descending = ascending - octave;
    let distance = ascending.abs().min(descending.abs());
    i64::from(distance) * i64::from(distance)
}

fn voice_costs(
    source: &[Pitch],
    target: &[Pitch],
    octave: u16,
) -> Result<CostMatrix<i64>, HarmonyError> {
    CostMatrix::new(
        source.len(),
        target.len(),
        source
            .iter()
            .flat_map(|left| {
                target.iter().map(move |right| {
                    circular_squared_cost(
                        i32::from(left.class.value()),
                        i32::from(right.class.value()),
                        octave,
                    )
                })
            })
            .collect(),
    )
    .map_err(|error| HarmonyError::InvalidField {
        field: "voicing-change.costs",
        reason: error.to_string(),
    })
}
