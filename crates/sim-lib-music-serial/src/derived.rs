//! Derived-cell deployment and invariant-form search with explicit certificates.

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    DerivationKind, RowError, RowForm, RowOperation, RowSegment, SegmentInvariant, ToneRow,
    analyze_derivation_partition, analyze_invariance,
};

use crate::techniques::derived_cells::build_derived_cell_plan;
use crate::{
    RowInstanceId, SerialDeployError, SerialEventId, SerialPlan, StructuralLicense, VoiceId,
};

/// One deployed derived-cell occurrence with preserved generator and transform evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedCellOccurrence {
    /// Stable event identity for the emitted occurrence.
    pub event_id: SerialEventId,
    /// Stable voice receiving the occurrence.
    pub voice: VoiceId,
    /// Zero-based occurrence index in source-row order.
    pub occurrence_index: usize,
    /// The source ordinals realized by this occurrence.
    pub source_ordinals: Vec<u8>,
    /// The generator-cell ordinals repeated by the derivation.
    pub generator_ordinals: Vec<u8>,
    /// The generator-cell pitch classes.
    pub generator_classes: Vec<PitchClass>,
    /// The exact row operation deriving this occurrence from the generator.
    pub operation: RowOperation,
}

/// One inspectable derived-cell deployment with immutable plan output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedCellDeployment {
    /// Immutable structural plan produced by the deployment.
    pub plan: SerialPlan,
    /// The detected derivation family that licensed the deployment.
    pub kind: DerivationKind,
    /// The generator-cell size in ordinals.
    pub generator_size: usize,
    /// Occurrence-by-occurrence derivation evidence.
    pub occurrences: Vec<DerivedCellOccurrence>,
}

/// Deploys a derivation-supported row as ordered derived-cell occurrences.
pub fn deploy_derived_cells(
    row_id: RowInstanceId,
    row_form: RowForm,
    generator_size: usize,
    voices: Vec<VoiceId>,
    event_prefix: impl AsRef<str>,
    rationale: impl AsRef<str>,
    license: StructuralLicense,
) -> Result<DerivedCellDeployment, SerialDeployError> {
    let derivation = analyze_derivation_partition(row_form.row(), generator_size)
        .map_err(|error| SerialDeployError::Plan(error.to_string()))?
        .ok_or_else(|| {
            SerialDeployError::Plan(format!(
                "row {} is not derivational at generator size {generator_size}",
                row_id.as_str()
            ))
        })?;
    let deployed = build_derived_cell_plan(
        row_id,
        row_form,
        derivation.clone(),
        voices,
        event_prefix.as_ref(),
        rationale.as_ref(),
        license,
    )?;
    Ok(DerivedCellDeployment {
        plan: deployed.plan,
        kind: derivation.kind,
        generator_size: derivation.generator_size,
        occurrences: deployed
            .occurrences
            .into_iter()
            .map(|occurrence| DerivedCellOccurrence {
                event_id: occurrence.event_id,
                voice: occurrence.voice,
                occurrence_index: occurrence.occurrence_index,
                source_ordinals: occurrence.source_ordinals,
                generator_ordinals: occurrence.generator_ordinals,
                generator_classes: occurrence.generator_classes,
                operation: occurrence.operation,
            })
            .collect(),
    })
}

/// Additional symmetry constraint layered onto invariant matching.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SymmetryRequirement {
    /// No extra symmetry requirement.
    None,
    /// Candidate pitches must read the same forwards and backwards.
    PitchPalindrome,
    /// Candidate must be the exact retrograde of the source segment.
    RetrogradeSource,
}

/// One inspectable invariant requirement.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InvariantRequirement {
    /// Preserve the source ordinals exactly.
    pub preserve_source_ordinals: bool,
    /// Preserve ordered pitch identity exactly.
    pub preserve_pitch_identity: bool,
    /// Require a transposition witness.
    pub require_transposition: bool,
    /// Require an inversion witness.
    pub require_inversion: bool,
    /// Preserve directed adjacent intervals exactly.
    pub preserve_interval_order: bool,
    /// Preserve the unordered set class.
    pub preserve_set_class: bool,
    /// Extra symmetry requirement.
    pub symmetry: SymmetryRequirement,
}

impl InvariantRequirement {
    /// Returns one permissive requirement that only asks for an explicit witness.
    pub const fn any() -> Self {
        Self {
            preserve_source_ordinals: false,
            preserve_pitch_identity: false,
            require_transposition: false,
            require_inversion: false,
            preserve_interval_order: false,
            preserve_set_class: false,
            symmetry: SymmetryRequirement::None,
        }
    }
}

/// One symmetry witness attached to an invariant candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymmetryCertificate {
    /// Whether the candidate segment is a pitch palindrome.
    pub pitch_palindrome: bool,
    /// Whether the candidate is the exact retrograde of the source segment.
    pub retrograde_source: bool,
}

/// One invariant witness proving why a form candidate matches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantCertificate {
    /// Operation producing the candidate row form.
    pub operation: RowOperation,
    /// Candidate segment ordinals inside that form.
    pub candidate_ordinals: Vec<u8>,
    /// Ordered invariance facts for the comparison.
    pub invariant: SegmentInvariant,
    /// Any extra symmetry evidence.
    pub symmetry: SymmetryCertificate,
}

impl InvariantCertificate {
    /// Returns whether this certificate satisfies the selected requirement.
    pub fn satisfies(&self, requirement: &InvariantRequirement) -> bool {
        (!requirement.preserve_source_ordinals || self.invariant.ordinal_identity)
            && (!requirement.preserve_pitch_identity || self.invariant.pitch_identity)
            && (!requirement.require_transposition || self.invariant.transposition.is_some())
            && (!requirement.require_inversion || self.invariant.inversion.is_some())
            && (!requirement.preserve_interval_order || self.invariant.interval_order_identity)
            && (!requirement.preserve_set_class || self.invariant.set_class_identity)
            && match requirement.symmetry {
                SymmetryRequirement::None => true,
                SymmetryRequirement::PitchPalindrome => self.symmetry.pitch_palindrome,
                SymmetryRequirement::RetrogradeSource => self.symmetry.retrograde_source,
            }
    }
}

/// One candidate row form whose certificate satisfies the requirement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantFormCandidate {
    /// Matching row form.
    pub form: RowForm,
    /// Inspectable evidence for the match.
    pub certificate: InvariantCertificate,
}

/// Explicit unsatisfied evidence when no candidate meets the requirement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsatisfiedInvariantRequest {
    /// Requirement that could not be satisfied.
    pub requirement: InvariantRequirement,
    /// Number of row forms inspected.
    pub forms_checked: usize,
    /// Number of same-length segments inspected.
    pub segments_checked: usize,
}

/// Invariant search result that preserves both matching candidates and explicit failure evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantFormCandidates {
    candidates: Vec<InvariantFormCandidate>,
    unsatisfied: Option<UnsatisfiedInvariantRequest>,
}

impl InvariantFormCandidates {
    /// Returns the matching candidates in stable search order.
    pub fn iter(&self) -> impl Iterator<Item = &InvariantFormCandidate> {
        self.candidates.iter()
    }

    /// Returns the matching candidates as a slice.
    pub fn as_slice(&self) -> &[InvariantFormCandidate] {
        &self.candidates
    }

    /// Returns explicit unsatisfied evidence when no candidate matched.
    pub fn unsatisfied(&self) -> Option<&UnsatisfiedInvariantRequest> {
        self.unsatisfied.as_ref()
    }
}

/// Searches every row form for same-length segments satisfying the requirement.
pub fn forms_with_invariant(
    row: &ToneRow,
    segment: &RowSegment,
    requirement: InvariantRequirement,
) -> Result<InvariantFormCandidates, RowError> {
    let segment_len = segment.classes().len();
    let mut candidates = Vec::new();
    let mut forms_checked = 0usize;
    let mut segments_checked = 0usize;

    for family in [
        sim_lib_pitch_serial::RowFamily::P,
        sim_lib_pitch_serial::RowFamily::I,
        sim_lib_pitch_serial::RowFamily::R,
        sim_lib_pitch_serial::RowFamily::RI,
    ] {
        for addend in 0..12 {
            forms_checked += 1;
            let form = row.apply(RowOperation::new(family, addend));
            for start in 0..=(form.classes().len() - segment_len) {
                segments_checked += 1;
                let candidate_segment = form.row().segment(start, segment_len)?;
                let certificate = build_certificate(segment, &candidate_segment, form.operation());
                if certificate.satisfies(&requirement) {
                    candidates.push(InvariantFormCandidate {
                        form: form.clone(),
                        certificate,
                    });
                }
            }
        }
    }

    let unsatisfied = candidates
        .is_empty()
        .then_some(UnsatisfiedInvariantRequest {
            requirement,
            forms_checked,
            segments_checked,
        });
    Ok(InvariantFormCandidates {
        candidates,
        unsatisfied,
    })
}

fn build_certificate(
    source: &RowSegment,
    candidate: &RowSegment,
    operation: RowOperation,
) -> InvariantCertificate {
    InvariantCertificate {
        operation,
        candidate_ordinals: candidate.ordinals().to_vec(),
        invariant: analyze_invariance(source, candidate),
        symmetry: SymmetryCertificate {
            pitch_palindrome: is_pitch_palindrome(candidate.classes()),
            retrograde_source: candidate.classes().iter().copied().eq(source
                .classes()
                .iter()
                .rev()
                .copied()),
        },
    }
}

fn is_pitch_palindrome(classes: &[PitchClass]) -> bool {
    classes.iter().eq(classes.iter().rev())
}
