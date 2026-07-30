mod source;
mod target;

use crate::{
    AmbiguousConversionPolicy, ConversionError, MusicConversion, ScoreForm, ScoreFormKind,
};

use self::source::to_staff;
use self::target::{embedded_ids, from_staff};

/// Converts any catalog score form to any other through the identity-bearing staff.
///
/// The caller must always provide an ambiguity policy. [`AmbiguousConversionPolicy::Reject`]
/// is the loss-intolerant choice; the other policies make a deterministic
/// selection and record every discarded object in the returned report.
pub fn convert_score(
    source: &ScoreForm,
    target: ScoreFormKind,
    policy: AmbiguousConversionPolicy,
) -> Result<MusicConversion<ScoreForm>, ConversionError> {
    if source.kind() == target {
        return Ok(MusicConversion {
            value: source.clone(),
            preserved: embedded_ids(source),
            losses: Vec::new(),
        });
    }

    let source_kind = source.kind();
    let mut staff_report = to_staff(source)?;
    let mut target_report = from_staff(&staff_report.value, source_kind, target, policy)?;
    staff_report.losses.append(&mut target_report.losses);
    target_report.losses = staff_report.losses;
    Ok(target_report)
}
