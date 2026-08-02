use super::{GatingPolicy, LoudnessError, LoudnessSpec, NormalizationSpec};

pub(super) fn validate_spec(spec: &LoudnessSpec) -> Result<(), LoudnessError> {
    if spec.sample_rate_hz < 8_000 || spec.sample_rate_hz > 768_000 {
        return Err(invalid("sample rate", "must be in 8000..=768000 Hz"));
    }
    if spec.layout.channels.is_empty() || spec.layout.channels.len() > 32 {
        return Err(invalid(
            "channel layout",
            "must contain between one and 32 channels",
        ));
    }
    if spec.max_frames == 0 {
        return Err(invalid("frame bound", "must be positive"));
    }
    if let GatingPolicy::AbsoluteRelative {
        absolute_lufs,
        relative_lu,
    } = spec.gating
        && (!absolute_lufs.is_finite() || !relative_lu.is_finite() || relative_lu >= 0.0)
    {
        return Err(invalid(
            "gate",
            "absolute threshold must be finite and relative LU must be negative",
        ));
    }
    let true_peak = spec.true_peak;
    if true_peak.oversample_factor == 0
        || true_peak.oversample_factor > 16
        || true_peak.taps < 8
        || true_peak.taps > 128
        || !true_peak.taps.is_multiple_of(2)
        || true_peak.max_work == 0
    {
        return Err(invalid(
            "true peak",
            "factor, even tap count, or work bound is invalid",
        ));
    }
    Ok(())
}

pub(super) fn validate_normalization(spec: NormalizationSpec) -> Result<(), LoudnessError> {
    if !spec.target_lufs.is_finite()
        || !spec.max_true_peak_dbtp.is_finite()
        || !spec.max_abs_gain_db.is_finite()
        || spec.max_abs_gain_db < 0.0
    {
        return Err(invalid(
            "normalization",
            "levels and nonnegative gain bound must be finite",
        ));
    }
    Ok(())
}

fn invalid(field: &'static str, reason: &'static str) -> LoudnessError {
    LoudnessError::InvalidPolicy { field, reason }
}
