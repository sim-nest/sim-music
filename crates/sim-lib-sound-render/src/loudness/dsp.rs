use std::f64::consts::PI;

use super::{FrequencyWeighting, LoudnessError, LoudnessSpec, TruePeakReport};

pub(super) fn weight_channels(input: &[f32], spec: &LoudnessSpec) -> Vec<f64> {
    let channels = spec.layout.channels.len();
    let mut output = input
        .iter()
        .map(|sample| f64::from(*sample))
        .collect::<Vec<_>>();
    if spec.frequency_weighting == FrequencyWeighting::Flat {
        return output;
    }
    let (shelf, high_pass) = k_weighting_coefficients(f64::from(spec.sample_rate_hz));
    for channel in 0..channels {
        let mut first = BiquadState::default();
        let mut second = BiquadState::default();
        for frame in 0..input.len() / channels {
            let at = frame * channels + channel;
            output[at] = second.process(first.process(output[at], shelf), high_pass);
        }
    }
    output
}

pub(super) fn measure_true_peak(
    input: &[f32],
    spec: &LoudnessSpec,
) -> Result<TruePeakReport, LoudnessError> {
    let channels = spec.layout.channels.len();
    let frames = input.len() / channels;
    let factor = spec.true_peak.oversample_factor;
    let work = (frames as u64)
        .checked_mul(channels as u64)
        .and_then(|value| value.checked_mul(factor as u64))
        .and_then(|value| value.checked_mul(spec.true_peak.taps as u64))
        .ok_or(LoudnessError::SizeOverflow)?;
    if work > spec.true_peak.max_work {
        return Err(LoudnessError::WorkLimit {
            required: work,
            maximum: spec.true_peak.max_work,
        });
    }
    let sample_peak = input.iter().copied().map(f32::abs).fold(0.0f32, f32::max) as f64;
    let mut true_peak = sample_peak;
    let left = spec.true_peak.taps / 2 - 1;
    for channel in 0..channels {
        for frame in 0..frames {
            for phase in 0..factor {
                let fraction = phase as f64 / factor as f64;
                let mut sample = 0.0;
                let mut weight = 0.0;
                for tap in 0..spec.true_peak.taps {
                    let source = frame as isize + tap as isize - left as isize;
                    if source < 0 || source >= frames as isize {
                        continue;
                    }
                    let distance = tap as f64 - left as f64 - fraction;
                    let window_at = tap as f64 / (spec.true_peak.taps - 1) as f64;
                    let blackman = 0.42 - 0.5 * (2.0 * PI * window_at).cos()
                        + 0.08 * (4.0 * PI * window_at).cos();
                    let coefficient = sinc(distance) * blackman;
                    sample += f64::from(input[source as usize * channels + channel]) * coefficient;
                    weight += coefficient;
                }
                if weight.abs() > f64::EPSILON {
                    true_peak = true_peak.max((sample / weight).abs());
                }
            }
        }
    }
    Ok(TruePeakReport {
        sample_peak,
        true_peak,
        true_peak_dbtp: amplitude_db(true_peak),
        oversample_factor: factor,
        work_units: work,
    })
}

#[derive(Clone, Copy)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

#[derive(Default)]
struct BiquadState {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl BiquadState {
    fn process(&mut self, input: f64, coefficients: Biquad) -> f64 {
        let output =
            coefficients.b0 * input + coefficients.b1 * self.x1 + coefficients.b2 * self.x2
                - coefficients.a1 * self.y1
                - coefficients.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

fn k_weighting_coefficients(sample_rate: f64) -> (Biquad, Biquad) {
    let shelf_frequency = 1_681.974_450_955_533;
    let shelf_gain_db = 3.999_843_853_973_347;
    let shelf_q = 0.707_175_236_955_419_6;
    let k = (PI * shelf_frequency / sample_rate).tan();
    let vh = 10.0f64.powf(shelf_gain_db / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / shelf_q + k * k;
    let shelf = Biquad {
        b0: (vh + vb * k / shelf_q + k * k) / a0,
        b1: 2.0 * (k * k - vh) / a0,
        b2: (vh - vb * k / shelf_q + k * k) / a0,
        a1: 2.0 * (k * k - 1.0) / a0,
        a2: (1.0 - k / shelf_q + k * k) / a0,
    };

    let high_pass_frequency = 38.135_470_876_024_44;
    let high_pass_q = 0.500_327_037_323_877_3;
    let k = (PI * high_pass_frequency / sample_rate).tan();
    let a0 = 1.0 + k / high_pass_q + k * k;
    let high_pass = Biquad {
        b0: 1.0 / a0,
        b1: -2.0 / a0,
        b2: 1.0 / a0,
        a1: 2.0 * (k * k - 1.0) / a0,
        a2: (1.0 - k / high_pass_q + k * k) / a0,
    };
    (shelf, high_pass)
}

fn amplitude_db(amplitude: f64) -> Option<f64> {
    (amplitude > 0.0).then(|| 20.0 * amplitude.log10())
}

fn sinc(value: f64) -> f64 {
    if value.abs() <= f64::EPSILON {
        1.0
    } else {
        (PI * value).sin() / (PI * value)
    }
}
