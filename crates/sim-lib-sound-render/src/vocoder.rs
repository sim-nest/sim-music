use std::f64::consts::{PI, TAU};

use sim_lib_numbers_signal::{
    Direction, SignalBuffer, SignalError, SignalView, SpectrumPacking, TransformKind,
    TransformPlan, transform, unwrap_phase,
};
use sim_lib_sound_audio_lift::{AudioTransformError, StftPlan, stft};
use thiserror::Error;

/// Relationship imposed between neighboring short-time Fourier bins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseLockPolicy {
    /// Propagate every bin phase independently.
    Independent,
    /// Lock non-peak bins to the nearest local spectral peak.
    Identity {
        /// Maximum distance in bins at which a local peak may own a bin.
        radius_bins: usize,
    },
}

/// Attack-handling policy for the phase vocoder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransientPolicy {
    /// Propagate every frame continuously, allowing the classic vocoder smear.
    Smear,
    /// Reset synthesis phases when normalized positive spectral flux reaches a threshold.
    Reset {
        /// Finite nonnegative onset threshold.
        spectral_flux_threshold: f64,
    },
}

/// Policy used to choose an unwrapped phase residual.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhaseUnwrapPolicy {
    /// Select the residual nearest the expected bin-center advance.
    ExpectedAdvance {
        /// Discontinuity passed to the canonical numbers-signal unwrap owner.
        discontinuity_radians: f64,
    },
}

/// Policy used to turn analysis-frame phase into synthesis frequency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstantaneousFrequencyPolicy {
    /// Use each FFT bin center and discard within-bin phase deviation.
    BinCenter,
    /// Add the unwrapped phase derivative to each bin-center frequency.
    PhaseDerivative,
}

/// Complete bounded policy for offline monophonic phase vocoding.
#[derive(Clone, Debug, PartialEq)]
pub struct VocoderPolicy {
    /// Source and destination sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Output duration divided by input duration.
    pub stretch_ratio: f64,
    /// Output pitch divided by input pitch, independent of duration.
    pub pitch_ratio: f64,
    /// Existing phase-preserving STFT plan to compose.
    pub stft: StftPlan,
    /// Neighbor-bin phase relationship.
    pub phase_lock: PhaseLockPolicy,
    /// Transient phase-reset behavior.
    pub transient: TransientPolicy,
    /// Phase-residual unwrapping convention.
    pub phase_unwrap: PhaseUnwrapPolicy,
    /// Instantaneous-frequency estimator.
    pub instantaneous_frequency: InstantaneousFrequencyPolicy,
    /// Hard output-sample bound checked before synthesis allocation.
    pub max_output_samples: usize,
}

impl Default for VocoderPolicy {
    fn default() -> Self {
        Self {
            sample_rate_hz: 48_000,
            stretch_ratio: 1.0,
            pitch_ratio: 1.0,
            stft: StftPlan::default(),
            phase_lock: PhaseLockPolicy::Identity { radius_bins: 3 },
            transient: TransientPolicy::Reset {
                spectral_flux_threshold: 0.35,
            },
            phase_unwrap: PhaseUnwrapPolicy::ExpectedAdvance {
                discontinuity_radians: PI,
            },
            instantaneous_frequency: InstantaneousFrequencyPolicy::PhaseDerivative,
            max_output_samples: 16_777_216,
        }
    }
}

/// Visible PCM and policy evidence returned by [`phase_vocode`].
#[derive(Clone, Debug, PartialEq)]
pub struct PhaseVocoderReport {
    /// Offline monophonic float PCM; values are not silently clipped.
    pub samples: Vec<f32>,
    /// Input sample count.
    pub input_samples: usize,
    /// Exact requested output sample count.
    pub output_samples: usize,
    /// Number of STFT frames composed by the transform.
    pub analysis_frames: usize,
    /// Floating synthesis hop before per-frame coordinate rounding.
    pub synthesis_hop: f64,
    /// Frames whose phases were reset by transient policy.
    pub transient_resets: usize,
    /// Absolute peak of the unbounded float result.
    pub peak: f32,
    /// Samples outside `[-1, 1]`; no limiter or clipper is hidden here.
    pub clipped_samples: usize,
    /// Exact retained policy.
    pub policy: VocoderPolicy,
}

/// Invalid phase-vocoder policy or delegated STFT/signal failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum VocoderError {
    /// A named policy field violated its finite definition.
    #[error("invalid vocoder {field}: {reason}")]
    InvalidPolicy {
        /// Rejected field.
        field: &'static str,
        /// Stable diagnostic.
        reason: &'static str,
    },
    /// Requested output exceeded its explicit sample bound.
    #[error("vocoder output needs {required} samples, exceeding {maximum}")]
    OutputLimit {
        /// Requested samples.
        required: usize,
        /// Policy ceiling.
        maximum: usize,
    },
    /// Output coordinate or allocation arithmetic overflowed.
    #[error("vocoder output coordinate arithmetic overflowed")]
    SizeOverflow,
    /// Existing audio-domain STFT rejected the request.
    #[error(transparent)]
    Stft(#[from] AudioTransformError),
    /// Existing generic transform or phase owner rejected the request.
    #[error(transparent)]
    Signal(#[from] SignalError),
}

/// Applies a bounded offline phase vocoder by composing the existing
/// phase-preserving STFT and generic numbers-signal FFT/unwrap owners.
pub fn phase_vocode(
    input: &[f32],
    policy: VocoderPolicy,
) -> Result<PhaseVocoderReport, VocoderError> {
    validate_policy(&policy)?;
    let output_samples = scaled_len(input.len(), policy.stretch_ratio)?;
    if output_samples > policy.max_output_samples {
        return Err(VocoderError::OutputLimit {
            required: output_samples,
            maximum: policy.max_output_samples,
        });
    }
    if input.is_empty() {
        return Ok(empty_report(policy));
    }

    let analysis = stft(input, policy.sample_rate_hz, &policy.stft)?;
    let bins = policy.stft.frame / 2 + 1;
    let synthesis_hop = policy.stft.hop as f64 * policy.stretch_ratio;
    let synthesis_window = policy.stft.synthesis_window.generate(policy.stft.frame)?;
    let analysis_window = policy.stft.analysis_window.generate(policy.stft.frame)?;
    let synthesis_len =
        synthesis_storage_len(analysis.frames.len(), synthesis_hop, policy.stft.frame)?;
    let mut output = vec![0.0f64; synthesis_len];
    let mut gain = vec![0.0f64; synthesis_len];
    let mut previous_phase = vec![0.0f64; bins];
    let mut synthesis_phase = vec![0.0f64; bins];
    let mut previous_magnitude = vec![0.0f64; bins];
    let mut transient_resets = 0usize;

    for (frame_index, frame) in analysis.frames.iter().enumerate() {
        let magnitudes = frame
            .bins
            .iter()
            .map(|(real, imaginary)| real.hypot(*imaginary))
            .collect::<Vec<_>>();
        let phases = frame
            .bins
            .iter()
            .map(|(real, imaginary)| imaginary.atan2(*real))
            .collect::<Vec<_>>();
        let reset =
            frame_index > 0 && transient_reset(&magnitudes, &previous_magnitude, policy.transient);
        transient_resets += usize::from(reset);

        for bin in 0..bins {
            if frame_index == 0 || reset {
                synthesis_phase[bin] = phases[bin];
            } else {
                let expected = TAU * policy.stft.hop as f64 * bin as f64 / policy.stft.frame as f64;
                let residual = unwrap_residual(
                    phases[bin] - previous_phase[bin] - expected,
                    policy.phase_unwrap,
                )?;
                let radians_per_sample = match policy.instantaneous_frequency {
                    InstantaneousFrequencyPolicy::BinCenter => {
                        TAU * bin as f64 / policy.stft.frame as f64
                    }
                    InstantaneousFrequencyPolicy::PhaseDerivative => {
                        (expected + residual) / policy.stft.hop as f64
                    }
                };
                synthesis_phase[bin] += radians_per_sample * synthesis_hop;
            }
        }
        apply_phase_lock(
            &magnitudes,
            &phases,
            &mut synthesis_phase,
            policy.phase_lock,
        );
        let shifted = shift_bins(&magnitudes, &synthesis_phase, policy.pitch_ratio, bins);
        overlap_add(
            &shifted,
            frame_index,
            synthesis_hop,
            &policy,
            &analysis_window.samples,
            &synthesis_window.samples,
            &mut output,
            &mut gain,
        )?;
        previous_phase.copy_from_slice(&phases);
        previous_magnitude.copy_from_slice(&magnitudes);
    }

    let trim_start = scaled_len(analysis.left_padding, policy.stretch_ratio)?;
    let samples = trim_output(&output, &gain, trim_start, output_samples);
    let peak = samples.iter().copied().map(f32::abs).fold(0.0, f32::max);
    let clipped_samples = samples.iter().filter(|sample| sample.abs() > 1.0).count();
    Ok(PhaseVocoderReport {
        samples,
        input_samples: input.len(),
        output_samples,
        analysis_frames: analysis.frames.len(),
        synthesis_hop,
        transient_resets,
        peak,
        clipped_samples,
        policy,
    })
}

fn validate_policy(policy: &VocoderPolicy) -> Result<(), VocoderError> {
    if policy.sample_rate_hz == 0 {
        return Err(invalid("sample rate", "must be positive"));
    }
    if !policy.stretch_ratio.is_finite() || policy.stretch_ratio <= 0.0 {
        return Err(invalid("stretch ratio", "must be finite and positive"));
    }
    if !policy.pitch_ratio.is_finite() || policy.pitch_ratio <= 0.0 {
        return Err(invalid("pitch ratio", "must be finite and positive"));
    }
    if policy.max_output_samples == 0 {
        return Err(invalid("output bound", "must be positive"));
    }
    if let PhaseLockPolicy::Identity { radius_bins } = policy.phase_lock
        && radius_bins == 0
    {
        return Err(invalid("phase lock radius", "must be positive"));
    }
    if let TransientPolicy::Reset {
        spectral_flux_threshold,
    } = policy.transient
        && (!spectral_flux_threshold.is_finite() || spectral_flux_threshold < 0.0)
    {
        return Err(invalid(
            "transient threshold",
            "must be finite and nonnegative",
        ));
    }
    let PhaseUnwrapPolicy::ExpectedAdvance {
        discontinuity_radians,
    } = policy.phase_unwrap;
    if !discontinuity_radians.is_finite() || !(PI..=TAU).contains(&discontinuity_radians) {
        return Err(invalid(
            "phase unwrap discontinuity",
            "must lie between pi and one turn",
        ));
    }
    Ok(())
}

fn scaled_len(len: usize, ratio: f64) -> Result<usize, VocoderError> {
    let scaled = len as f64 * ratio;
    if !scaled.is_finite() || scaled > usize::MAX as f64 {
        return Err(VocoderError::SizeOverflow);
    }
    Ok(scaled.round() as usize)
}

fn synthesis_storage_len(frames: usize, hop: f64, frame_len: usize) -> Result<usize, VocoderError> {
    let last = frames.saturating_sub(1) as f64 * hop;
    if !last.is_finite() || last > usize::MAX as f64 {
        return Err(VocoderError::SizeOverflow);
    }
    (last.round() as usize)
        .checked_add(frame_len)
        .ok_or(VocoderError::SizeOverflow)
}

fn transient_reset(current: &[f64], previous: &[f64], policy: TransientPolicy) -> bool {
    let TransientPolicy::Reset {
        spectral_flux_threshold,
    } = policy
    else {
        return false;
    };
    let positive_flux = current
        .iter()
        .zip(previous)
        .map(|(current, previous)| (current - previous).max(0.0))
        .sum::<f64>();
    let energy = current.iter().sum::<f64>().max(f64::EPSILON);
    positive_flux / energy >= spectral_flux_threshold
}

fn unwrap_residual(residual: f64, policy: PhaseUnwrapPolicy) -> Result<f64, SignalError> {
    let PhaseUnwrapPolicy::ExpectedAdvance {
        discontinuity_radians,
    } = policy;
    let unwrapped = unwrap_phase(&[0.0, residual], discontinuity_radians)?;
    Ok(unwrapped[1])
}

fn apply_phase_lock(
    magnitudes: &[f64],
    analysis_phase: &[f64],
    synthesis_phase: &mut [f64],
    policy: PhaseLockPolicy,
) {
    let PhaseLockPolicy::Identity { radius_bins } = policy else {
        return;
    };
    let peaks = (0..magnitudes.len())
        .filter(|index| {
            let left = index
                .checked_sub(1)
                .map(|left| magnitudes[left])
                .unwrap_or(f64::NEG_INFINITY);
            let right = magnitudes
                .get(index + 1)
                .copied()
                .unwrap_or(f64::NEG_INFINITY);
            magnitudes[*index] >= left && magnitudes[*index] >= right
        })
        .collect::<Vec<_>>();
    let independent = synthesis_phase.to_vec();
    for bin in 0..synthesis_phase.len() {
        let owner = peaks
            .iter()
            .copied()
            .filter(|peak| peak.abs_diff(bin) <= radius_bins)
            .min_by_key(|peak| peak.abs_diff(bin));
        if let Some(peak) = owner {
            synthesis_phase[bin] = independent[peak] + analysis_phase[bin] - analysis_phase[peak];
        }
    }
}

fn shift_bins(
    magnitudes: &[f64],
    phases: &[f64],
    pitch_ratio: f64,
    output_bins: usize,
) -> Vec<(f64, f64)> {
    let mut shifted = vec![(0.0, 0.0); output_bins];
    for source in 0..magnitudes.len() {
        let target = source as f64 * pitch_ratio;
        let lower = target.floor() as usize;
        let fraction = target - target.floor();
        add_polar(
            &mut shifted,
            lower,
            magnitudes[source] * (1.0 - fraction),
            phases[source] * pitch_ratio,
        );
        add_polar(
            &mut shifted,
            lower.saturating_add(1),
            magnitudes[source] * fraction,
            phases[source] * pitch_ratio,
        );
    }
    if let Some(dc) = shifted.first_mut() {
        dc.1 = 0.0;
    }
    if let Some(nyquist) = shifted.last_mut() {
        nyquist.1 = 0.0;
    }
    shifted
}

fn add_polar(target: &mut [(f64, f64)], bin: usize, magnitude: f64, phase: f64) {
    if let Some((real, imaginary)) = target.get_mut(bin) {
        *real += magnitude * phase.cos();
        *imaginary += magnitude * phase.sin();
    }
}

#[allow(clippy::too_many_arguments)]
fn overlap_add(
    bins: &[(f64, f64)],
    frame_index: usize,
    synthesis_hop: f64,
    policy: &VocoderPolicy,
    analysis_window: &[f64],
    synthesis_window: &[f64],
    output: &mut [f64],
    gain: &mut [f64],
) -> Result<(), VocoderError> {
    let mut inverse = TransformPlan::new(TransformKind::RealFft, policy.stft.frame);
    inverse.direction = Direction::Inverse;
    inverse.normalization = policy.stft.normalization;
    inverse.sign = policy.stft.phase;
    inverse.packing = SpectrumPacking::HermitianHalf;
    let SignalBuffer::Real(frame) = transform(&inverse, SignalView::Complex(bins))? else {
        unreachable!("inverse real FFT returns real samples")
    };
    let start = (frame_index as f64 * synthesis_hop).round() as usize;
    for offset in 0..policy.stft.frame {
        let at = start
            .checked_add(offset)
            .ok_or(VocoderError::SizeOverflow)?;
        output[at] += frame.as_slice()[offset] * synthesis_window[offset];
        gain[at] += analysis_window[offset] * synthesis_window[offset];
    }
    Ok(())
}

fn trim_output(output: &[f64], gain: &[f64], start: usize, len: usize) -> Vec<f32> {
    (0..len)
        .map(|offset| {
            let at = start.saturating_add(offset);
            let divisor = gain.get(at).copied().unwrap_or(0.0);
            if divisor.abs() <= 1e-12 {
                0.0
            } else {
                (output.get(at).copied().unwrap_or(0.0) / divisor) as f32
            }
        })
        .collect()
}

fn empty_report(policy: VocoderPolicy) -> PhaseVocoderReport {
    PhaseVocoderReport {
        samples: Vec::new(),
        input_samples: 0,
        output_samples: 0,
        analysis_frames: 0,
        synthesis_hop: policy.stft.hop as f64 * policy.stretch_ratio,
        transient_resets: 0,
        peak: 0.0,
        clipped_samples: 0,
        policy,
    }
}

fn invalid(field: &'static str, reason: &'static str) -> VocoderError {
    VocoderError::InvalidPolicy { field, reason }
}
