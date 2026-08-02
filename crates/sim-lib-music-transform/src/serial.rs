//! Serial realization adapters over the general transform and notation surfaces.

use std::collections::BTreeMap;

use sim_lib_music_core::{
    AmbiguousConversionPolicy, Music, ObjectId, Score, ScoreForm, ScoreFormKind, Staff, StaffNote,
    StaffVoice, Time, convert_score,
};
use sim_lib_music_serial::{
    RealizedSerialOrigin, SerialEventId, SerialRealization, SerialRenderOptions, VoiceId,
    render_serial_staff,
};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::RowOperation;

use crate::{RetrogradeMode, TransformError};

/// Whether a serial provenance facet survived a transform or was retired explicitly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialProvenanceStatus {
    /// The facet remains valid after the transform.
    Preserved,
    /// The transform retired the facet for the stated reason.
    Invalidated {
        /// Stable explanation for the invalidation.
        reason: &'static str,
    },
}

/// One retained serial note-evidence binding keyed by transformed staff identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialNoteEvidence {
    /// Stable score note identity.
    pub note_id: ObjectId,
    /// Stable score event identity.
    pub event_id: SerialEventId,
    /// Original serial origin carried by that note.
    pub origin: RealizedSerialOrigin,
}

/// Explicit serial provenance retained or retired by one transform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialTransformProvenance {
    /// Retained note-level origin evidence keyed by stable note ids.
    pub notes: Vec<SerialNoteEvidence>,
    /// Whether ordinal chronology still means the same thing after the transform.
    pub ordinal_order: SerialProvenanceStatus,
    /// Whether row-form evidence still names the transformed pitches truthfully.
    pub row_forms: SerialProvenanceStatus,
    /// Whether original voice identity still denotes the transformed voice layout.
    pub voices: SerialProvenanceStatus,
}

/// One transformed serial staff plus explicit provenance status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialStaffTransform {
    /// Transformed staff routed through the existing score model.
    pub staff: Staff,
    /// Explicit provenance retained or invalidated by the transform.
    pub provenance: SerialTransformProvenance,
}

/// Applies a total row operation to the realized pitches and time order.
pub fn apply_serial_row_operation(
    realization: &SerialRealization,
    operation: RowOperation,
) -> Result<SerialStaffTransform, TransformError> {
    let mut transformed = map_serial_staff(realization, |note| {
        let mut next = note.clone();
        let class = next.note.pitch.class;
        let class = if matches!(
            operation.family,
            sim_lib_pitch_serial::RowFamily::I | sim_lib_pitch_serial::RowFamily::RI
        ) {
            class.invert(PitchClass::C)
        } else {
            class
        };
        next.note.pitch.class = class.transpose(i32::from(operation.addend));
        next
    })?;
    if matches!(
        operation.family,
        sim_lib_pitch_serial::RowFamily::R | sim_lib_pitch_serial::RowFamily::RI
    ) {
        transformed = retrograde_staff(
            transformed,
            RetrogradeMode::Cutout,
            SerialProvenanceStatus::Invalidated {
                reason: "retrograde reverses serial chronology",
            },
            SerialProvenanceStatus::Preserved,
        )?;
    }
    Ok(transformed)
}

/// Transposes every realized pitch while retaining event and note identities.
pub fn transpose_serial(
    realization: &SerialRealization,
    semitones: i32,
) -> Result<SerialStaffTransform, TransformError> {
    map_serial_staff(realization, |note| {
        let mut next = note.clone();
        next.note.pitch = next.note.pitch.transpose(semitones);
        next
    })
}

/// Inverts every realized pitch class around `axis`, preserving ids but retiring row-form labels.
pub fn invert_serial(
    realization: &SerialRealization,
    axis: PitchClass,
) -> Result<SerialStaffTransform, TransformError> {
    let mut transformed = map_serial_staff(realization, |note| {
        let mut next = note.clone();
        next.note.pitch.class = next.note.pitch.class.invert(axis);
        next
    })?;
    transformed.provenance.row_forms = SerialProvenanceStatus::Invalidated {
        reason: "axis inversion is not the original row-form witness",
    };
    Ok(transformed)
}

/// Reverses realized chronology and retires chronology-dependent provenance explicitly.
pub fn retrograde_serial(
    realization: &SerialRealization,
    mode: RetrogradeMode,
) -> Result<SerialStaffTransform, TransformError> {
    retrograde_staff(
        base_transform(realization)?,
        mode,
        SerialProvenanceStatus::Invalidated {
            reason: "retrograde reverses serial chronology",
        },
        SerialProvenanceStatus::Invalidated {
            reason: "retrograde no longer states the original row order",
        },
    )
}

/// Scales exact onsets and durations while retaining pitch and serial evidence.
pub fn scale_serial_time(
    realization: &SerialRealization,
    factor: Time,
) -> Result<SerialStaffTransform, TransformError> {
    if factor <= Time::from_integer(0) {
        return Err(TransformError::InvalidFactor);
    }
    map_serial_staff(realization, |note| {
        let mut next = note.clone();
        next.onset *= factor;
        next.note.duration *= factor;
        next
    })
}

/// Quantizes only when the result is already exact on the requested grid; otherwise fails closed.
pub fn quantize_serial(
    realization: &SerialRealization,
    grid: Time,
) -> Result<SerialStaffTransform, TransformError> {
    if grid <= Time::from_integer(0) {
        return Err(TransformError::InvalidFactor);
    }
    let transformed = base_transform(realization)?;
    for note in transformed.staff.notes() {
        if !is_exact_multiple(note.onset, grid) || !is_exact_multiple(note.note.duration, grid) {
            return Err(TransformError::InvalidTransformOutput {
                transform: "serial-quantize",
                reason: "quantize would alter exact serial timing",
            });
        }
    }
    Ok(transformed)
}

/// Renames voices through an explicit mapping while retaining note/event origins.
pub fn remap_serial_voices(
    realization: &SerialRealization,
    mapping: &BTreeMap<VoiceId, VoiceId>,
) -> Result<SerialStaffTransform, TransformError> {
    let source = serial_staff(realization)?;
    let voices = source
        .voices
        .iter()
        .map(|voice| {
            let target_id = mapping
                .get(&voice.id)
                .cloned()
                .unwrap_or_else(|| voice.id.clone());
            let notes = voice
                .notes
                .iter()
                .map(|note| {
                    let mut next = note.clone();
                    next.voice_id = target_id.clone();
                    next
                })
                .collect::<Vec<_>>();
            StaffVoice {
                id: target_id.clone(),
                name: target_id.as_str().to_owned(),
                duration: notes
                    .iter()
                    .map(StaffNote::end)
                    .max()
                    .unwrap_or(voice.duration),
                notes,
            }
        })
        .collect::<Vec<_>>();
    let staff = Staff::new(voices)?;
    Ok(SerialStaffTransform {
        staff,
        provenance: SerialTransformProvenance {
            notes: note_evidence(realization),
            ordinal_order: SerialProvenanceStatus::Preserved,
            row_forms: SerialProvenanceStatus::Preserved,
            voices: SerialProvenanceStatus::Invalidated {
                reason: "voice remap changes declared voice identity",
            },
        },
    })
}

/// Renders realized serial material to a notation-compatible score surface.
pub fn render_serial_notation_score(
    realization: &SerialRealization,
    options: &SerialRenderOptions,
) -> Result<Score, TransformError> {
    notation_score(realization, options)
}

fn base_transform(realization: &SerialRealization) -> Result<SerialStaffTransform, TransformError> {
    Ok(SerialStaffTransform {
        staff: serial_staff(realization)?,
        provenance: SerialTransformProvenance {
            notes: note_evidence(realization),
            ordinal_order: SerialProvenanceStatus::Preserved,
            row_forms: SerialProvenanceStatus::Preserved,
            voices: SerialProvenanceStatus::Preserved,
        },
    })
}

fn map_serial_staff(
    realization: &SerialRealization,
    mut map: impl FnMut(&StaffNote) -> StaffNote,
) -> Result<SerialStaffTransform, TransformError> {
    let source = serial_staff(realization)?;
    let voices = source
        .voices
        .iter()
        .map(|voice| {
            let notes = voice.notes.iter().map(&mut map).collect::<Vec<_>>();
            StaffVoice {
                id: voice.id.clone(),
                name: voice.name.clone(),
                duration: notes
                    .iter()
                    .map(StaffNote::end)
                    .max()
                    .unwrap_or(voice.duration),
                notes,
            }
        })
        .collect::<Vec<_>>();
    Ok(SerialStaffTransform {
        staff: Staff::new(voices)?,
        provenance: SerialTransformProvenance {
            notes: note_evidence(realization),
            ordinal_order: SerialProvenanceStatus::Preserved,
            row_forms: SerialProvenanceStatus::Preserved,
            voices: SerialProvenanceStatus::Preserved,
        },
    })
}

fn retrograde_staff(
    source: SerialStaffTransform,
    mode: RetrogradeMode,
    ordinal_order: SerialProvenanceStatus,
    row_forms: SerialProvenanceStatus,
) -> Result<SerialStaffTransform, TransformError> {
    let total = source.staff.duration();
    let voices = source
        .staff
        .voices
        .iter()
        .map(|voice| {
            let mut notes = voice.notes.clone();
            match mode {
                RetrogradeMode::Cutout => {
                    for note in &mut notes {
                        note.onset = total - note.onset - note.note.duration;
                    }
                }
                RetrogradeMode::PinnedNoteOn => {
                    let mut onsets = notes.iter().map(|note| note.onset).collect::<Vec<_>>();
                    onsets.sort();
                    let payloads = notes
                        .iter()
                        .rev()
                        .map(|note| note.note.clone())
                        .collect::<Vec<_>>();
                    for (index, note) in notes.iter_mut().enumerate() {
                        note.onset = onsets[index];
                        note.note = payloads[index].clone();
                    }
                }
            }
            StaffVoice {
                id: voice.id.clone(),
                name: voice.name.clone(),
                duration: total,
                notes,
            }
        })
        .collect::<Vec<_>>();
    Ok(SerialStaffTransform {
        staff: Staff::new(voices)?,
        provenance: SerialTransformProvenance {
            notes: source.provenance.notes,
            ordinal_order,
            row_forms,
            voices: source.provenance.voices,
        },
    })
}

fn notation_score(
    realization: &SerialRealization,
    options: &SerialRenderOptions,
) -> Result<Score, TransformError> {
    let staff = serial_staff(realization)?;
    let report = convert_score(
        &ScoreForm::Staff(staff),
        ScoreFormKind::Counterpoint,
        AmbiguousConversionPolicy::Reject,
    )?;
    let ScoreForm::Counterpoint(counterpoint) = report.value else {
        unreachable!("counterpoint conversion returns counterpoint");
    };
    Score::new(
        options.tempo_bpm,
        options.time_signature,
        options.key.clone(),
        Music::Counterpoint(counterpoint),
    )
    .map_err(TransformError::from)
}

fn note_evidence(realization: &SerialRealization) -> Vec<SerialNoteEvidence> {
    realization
        .notes()
        .iter()
        .map(|note| SerialNoteEvidence {
            note_id: ObjectId::new(format!(
                "serial-note/{}/{}/{}",
                note.event_id, note.note_index, note.origin.source_ordinal.ordinal
            ))
            .expect("rendered serial note id"),
            event_id: note.event_id.clone(),
            origin: note.origin.clone(),
        })
        .collect()
}

fn is_exact_multiple(value: Time, step: Time) -> bool {
    let ratio = value / step;
    *ratio.denom() == 1
}

fn serial_staff(realization: &SerialRealization) -> Result<Staff, TransformError> {
    render_serial_staff(realization).map_err(|_| TransformError::InvalidTransformOutput {
        transform: "serial-staff",
        reason: "serial realization could not render to staff",
    })
}
