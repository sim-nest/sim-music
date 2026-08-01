use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_serial::RowForm;

use crate::{
    ReferentialClaim, ReferentialContextReport, ReferentialEvidence, ReferentialReport,
    ReferentialRequest,
    referential::{ratio_summary, roman_claim, scale_degrees},
};

pub(crate) fn build_referential_report(
    row_form: &RowForm,
    request: ReferentialRequest,
) -> Result<ReferentialReport, String> {
    if request.ordinals.is_empty() {
        return Err("referential ordinals cannot be empty".to_owned());
    }
    let segment = row_form
        .row()
        .indexed_segment(&request.ordinals)
        .map_err(|error| error.to_string())?;
    let subset_classes = segment.classes().to_vec();
    let subset_mask = segment.mask();

    let (claim, harmonic_claim, scale_degrees, ratio_summary) = match &request.evidence {
        ReferentialEvidence::Chord { label, chord } => {
            let target = chord.pitch_set().map_err(|error| error.to_string())?;
            let claim = if target == subset_mask {
                ReferentialClaim::EmbeddedSubset
            } else {
                ReferentialClaim::NoReferentialSubset
            };
            (claim, Some(label.clone()), Vec::new(), None)
        }
        ReferentialEvidence::Scale { label, scale } => {
            let claim = if subset_classes
                .iter()
                .all(|class| scale.mask().pitch_classes().contains(class))
            {
                ReferentialClaim::EmbeddedSubset
            } else {
                ReferentialClaim::NoReferentialSubset
            };
            (
                claim,
                Some(format!("{} {}", label, scale.mode.name())),
                scale_degrees(*scale, &subset_classes),
                None,
            )
        }
        ReferentialEvidence::KeyRegion {
            label,
            key,
            preferred_root,
        } => {
            let scale = Scale::new(key.tonic, key.mode);
            let claim = if subset_classes
                .iter()
                .all(|class| scale.mask().pitch_classes().contains(class))
            {
                ReferentialClaim::EmbeddedSubset
            } else {
                ReferentialClaim::NoReferentialSubset
            };
            (
                claim,
                roman_claim(subset_mask, *key, *preferred_root).or_else(|| Some(label.clone())),
                scale_degrees(scale, &subset_classes),
                None,
            )
        }
        ReferentialEvidence::PitchRatio {
            ratios,
            policy,
            label,
        } => {
            let summary = ratio_summary(ratios, *policy)?;
            let claim = if ratios.len() == subset_classes.len() {
                ReferentialClaim::EmbeddedSubset
            } else {
                ReferentialClaim::NoReferentialSubset
            };
            (claim, Some(label.clone()), Vec::new(), Some(summary))
        }
        ReferentialEvidence::Mask { label, mask } => {
            let claim = if *mask == subset_mask {
                ReferentialClaim::EmbeddedSubset
            } else {
                ReferentialClaim::NoReferentialSubset
            };
            (claim, Some(label.clone()), Vec::new(), None)
        }
    };

    Ok(ReferentialReport {
        claim,
        ordinals: request.ordinals,
        context: ReferentialContextReport {
            evidence_kind: request.evidence.kind(),
            evidence_label: request.evidence.label().to_owned(),
            harmonic_claim,
            subset_classes,
            subset_mask,
            scale_degrees,
            ratio_summary,
        },
        emphasis: request.emphasis,
        claims_whole_passage_tonality: false,
    })
}
