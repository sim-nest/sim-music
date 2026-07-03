use std::time::Duration;

use sim_lib_sound_core::{Amplitude, Frequency, Tone};

/// Records how a [`Spectrum`] was produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpectrumSource {
    /// Sampled from a [`Tone`] at a fixed moment.
    FromTone {
        /// Offset into the tone, in milliseconds.
        at_millis: u64,
    },
    /// Computed from a PCM sample window via a discrete Fourier transform.
    FromPcm {
        /// Number of samples in the analysis window.
        window_size: usize,
        /// Sample rate of the source PCM, in hertz.
        sample_rate: u32,
    },
    /// Constructed directly rather than derived from audio.
    Synthetic,
}

/// A frequency-domain magnitude spectrum: a set of frequency/amplitude bins
/// plus a record of how it was produced.
///
/// # Examples
///
/// ```
/// use sim_lib_sound_core::{Amplitude, Frequency};
/// use sim_lib_sound_spectrum::{Spectrum, SpectrumSource};
///
/// let spectrum = Spectrum {
///     bins: vec![
///         (Frequency(100.0), Amplitude(1.0)),
///         (Frequency(200.0), Amplitude(1.0)),
///     ],
///     source: SpectrumSource::Synthetic,
/// };
/// assert!((spectrum.centroid().0 - 150.0).abs() < 1e-9);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Spectrum {
    /// Frequency/amplitude pairs that make up the spectrum.
    pub bins: Vec<(Frequency, Amplitude)>,
    /// Provenance of this spectrum.
    pub source: SpectrumSource,
}

impl Spectrum {
    /// Builds a spectrum from a [`Tone`]'s partials sampled at offset `at`,
    /// scaling amplitudes by the envelope level at that moment.
    pub fn from_tone(tone: &Tone, at: Duration) -> Self {
        let gain = tone.envelope.sample_level(at, tone.duration);
        let bins = tone
            .partials
            .iter()
            .map(|partial| {
                (
                    partial.frequency,
                    Amplitude(partial.amplitude.0 * gain.clamp(0.0, 1.0)),
                )
            })
            .collect();
        Self {
            bins,
            source: SpectrumSource::FromTone {
                at_millis: at.as_millis() as u64,
            },
        }
    }

    /// Builds a magnitude spectrum from PCM `samples` by a direct discrete
    /// Fourier transform over the first `window_size` samples.
    pub fn from_pcm(samples: &[f32], sample_rate: u32, window_size: usize) -> Self {
        let window = window_size.min(samples.len()).max(1);
        let mut bins = Vec::with_capacity(window / 2 + 1);
        for bin in 0..=window / 2 {
            let mut real = 0.0;
            let mut imag = 0.0;
            for (index, sample) in samples.iter().take(window).enumerate() {
                let angle = std::f64::consts::TAU * bin as f64 * index as f64 / window as f64;
                real += f64::from(*sample) * angle.cos();
                imag -= f64::from(*sample) * angle.sin();
            }
            let magnitude = (real * real + imag * imag).sqrt() / window as f64;
            let frequency = Frequency(bin as f64 * f64::from(sample_rate) / window as f64);
            bins.push((frequency, Amplitude(magnitude)));
        }
        Self {
            bins,
            source: SpectrumSource::FromPcm {
                window_size: window,
                sample_rate,
            },
        }
    }

    /// Returns the `n` highest-amplitude bins, strongest first.
    pub fn peaks(&self, n: usize) -> Vec<(Frequency, Amplitude)> {
        let mut bins = self.bins.clone();
        bins.sort_by(|left, right| right.1.0.total_cmp(&left.1.0));
        bins.truncate(n);
        bins
    }

    /// Returns the amplitude-weighted mean frequency (spectral centroid).
    pub fn centroid(&self) -> Frequency {
        let total_weight: f64 = self.bins.iter().map(|(_, amp)| amp.0).sum();
        if total_weight <= f64::EPSILON {
            return Frequency(0.0);
        }
        let weighted: f64 = self.bins.iter().map(|(freq, amp)| freq.0 * amp.0).sum();
        Frequency(weighted / total_weight)
    }

    /// Returns the spectral flatness in `0.0..=1.0` (the ratio of geometric to
    /// arithmetic mean amplitude; higher is more noise-like).
    pub fn flatness(&self) -> f64 {
        if self.bins.is_empty() {
            return 0.0;
        }
        let amps: Vec<f64> = self.bins.iter().map(|(_, amp)| amp.0.max(1e-12)).collect();
        let geometric = amps.iter().map(|amp| amp.ln()).sum::<f64>() / amps.len() as f64;
        let arithmetic = amps.iter().sum::<f64>() / amps.len() as f64;
        (geometric.exp() / arithmetic).clamp(0.0, 1.0)
    }

    /// Returns the frequency below which `percentile` of the total amplitude
    /// lies (the spectral rolloff point).
    pub fn rolloff(&self, percentile: f64) -> Frequency {
        if self.bins.is_empty() {
            return Frequency(0.0);
        }
        let target =
            self.bins.iter().map(|(_, amp)| amp.0).sum::<f64>() * percentile.clamp(0.0, 1.0);
        let mut sum = 0.0;
        for (frequency, amplitude) in &self.bins {
            sum += amplitude.0;
            if sum >= target {
                return *frequency;
            }
        }
        self.bins
            .last()
            .map(|(frequency, _)| *frequency)
            .unwrap_or(Frequency(0.0))
    }

    /// Returns the spectral flux between two spectra: the Euclidean norm of
    /// their bin-wise amplitude differences.
    pub fn flux(prev: &Spectrum, curr: &Spectrum) -> f64 {
        let len = prev.bins.len().max(curr.bins.len());
        let mut sum = 0.0;
        for index in 0..len {
            let left = prev.bins.get(index).map(|(_, amp)| amp.0).unwrap_or(0.0);
            let right = curr.bins.get(index).map(|(_, amp)| amp.0).unwrap_or(0.0);
            let delta = right - left;
            sum += delta * delta;
        }
        sum.sqrt()
    }
}
