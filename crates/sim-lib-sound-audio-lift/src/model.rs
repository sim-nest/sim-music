use sim_kernel::Diagnostic;
use sim_lib_pitch_core::Pitch;
use sim_lib_sound_core::{Amplitude, Frequency};
use sim_lib_sound_spectrum::Spectrum;
use sim_lib_sound_tuning::Tuning;
use thiserror::Error;

use crate::pipeline::analyze;

/// Error raised when audio-lift options are invalid.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AudioLiftError {
    /// The sample rate was zero.
    #[error("sample rate must be positive")]
    InvalidSampleRate,
    /// The analysis window was smaller than 8 samples.
    #[error("window size must be at least 8 samples")]
    InvalidWindowSize,
    /// The hop size was zero.
    #[error("hop size must be positive")]
    InvalidHopSize,
    /// The maximum peak count was zero.
    #[error("max peaks must be positive")]
    InvalidPeakCount,
    /// The minimum note-window count was zero.
    #[error("minimum note windows must be positive")]
    InvalidMinNoteWindows,
}

/// A value paired with the diagnostics produced while computing it.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioLiftReport<T> {
    /// The computed value.
    pub value: T,
    /// Diagnostics emitted during analysis.
    pub diagnostics: Vec<Diagnostic>,
}

impl<T> AudioLiftReport<T> {
    /// Returns a report with the value transformed by `f`, keeping the
    /// diagnostics unchanged.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AudioLiftReport<U> {
        AudioLiftReport {
            value: f(self.value),
            diagnostics: self.diagnostics,
        }
    }
}

/// Parameters controlling the audio-lift analysis pipeline.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioLiftOptions {
    /// Size of the analysis window, in samples.
    pub window_size: usize,
    /// Number of samples advanced between successive windows.
    pub hop_size: usize,
    /// Maximum number of spectral peaks retained per window.
    pub max_peaks: usize,
    /// Minimum peak amplitude relative to the strongest peak.
    pub min_peak_ratio: f64,
    /// Tolerance, in cents, for matching harmonics to a fundamental.
    pub harmonic_tolerance_cents: f64,
    /// Minimum confidence for a pitch candidate to be accepted as a note.
    pub min_note_confidence: f64,
    /// Minimum number of consecutive windows a note must span.
    pub min_note_windows: usize,
}

impl AudioLiftOptions {
    /// Validates the options, returning the first violated constraint.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_sound_audio_lift::AudioLiftOptions;
    ///
    /// assert!(AudioLiftOptions::default().validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<(), AudioLiftError> {
        if self.window_size < 8 {
            return Err(AudioLiftError::InvalidWindowSize);
        }
        if self.hop_size == 0 {
            return Err(AudioLiftError::InvalidHopSize);
        }
        if self.max_peaks == 0 {
            return Err(AudioLiftError::InvalidPeakCount);
        }
        if self.min_note_windows == 0 {
            return Err(AudioLiftError::InvalidMinNoteWindows);
        }
        Ok(())
    }
}

impl Default for AudioLiftOptions {
    fn default() -> Self {
        Self {
            window_size: 2048,
            hop_size: 512,
            max_peaks: 8,
            min_peak_ratio: 0.12,
            harmonic_tolerance_cents: 28.0,
            min_note_confidence: 0.30,
            min_note_windows: 1,
        }
    }
}

/// A candidate pitch detected within a single analysis window.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchCandidate {
    /// Detected pitch.
    pub pitch: Pitch,
    /// Estimated fundamental frequency.
    pub frequency: Frequency,
    /// Amplitude associated with the candidate.
    pub amplitude: Amplitude,
    /// Detection confidence in `0.0..=1.0`.
    pub confidence: f64,
    /// Deviation of the detected frequency from the tuned pitch, in cents.
    pub cents_error: f64,
    /// Number of supporting harmonics found.
    pub harmonic_count: usize,
}

/// The analysis result for a single window of audio.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioLiftFrame {
    /// Index of the window within the analysis.
    pub index: usize,
    /// Sample offset of the window's onset.
    pub onset_sample: usize,
    /// Length of the window, in samples.
    pub duration_samples: usize,
    /// Spectrum computed for the window.
    pub spectrum: Spectrum,
    /// Pitch candidates detected in the window.
    pub pitch_candidates: Vec<PitchCandidate>,
    /// Human-readable diagnostics for the window.
    pub diagnostics: Vec<String>,
}

/// A note candidate assembled from consecutive analysis windows.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioNoteCandidate {
    /// Voice/track the note was assigned to.
    pub track: usize,
    /// Sample offset of the note's onset.
    pub onset_sample: usize,
    /// Length of the note, in samples.
    pub duration_samples: usize,
    /// Sample rate of the source audio.
    pub sample_rate: u32,
    /// Detected pitch.
    pub pitch: Pitch,
    /// Mean fundamental frequency across the note.
    pub mean_frequency: Frequency,
    /// Mean amplitude across the note.
    pub mean_amplitude: Amplitude,
    /// Detection confidence in `0.0..=1.0`.
    pub confidence: f64,
    /// Human-readable diagnostics for the note.
    pub diagnostics: Vec<String>,
}

/// The full result of lifting audio: per-window frames and assembled notes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioLiftResult {
    /// Per-window analysis frames.
    pub frames: Vec<AudioLiftFrame>,
    /// Note candidates assembled from the frames.
    pub notes: Vec<AudioNoteCandidate>,
}

/// A strategy that lifts raw audio samples into pitched note candidates.
pub trait AudioLifter {
    /// Returns the stable symbol identifying this lifter.
    fn symbol(&self) -> &'static str;

    /// Lifts `samples` into a result paired with diagnostics, interpreting
    /// pitches under `tuning`.
    fn lift_report(
        &self,
        samples: &[f32],
        sample_rate: u32,
        tuning: &dyn Tuning,
    ) -> Result<AudioLiftReport<AudioLiftResult>, AudioLiftError>;

    /// Lifts `samples`, discarding diagnostics and returning only the result.
    fn lift(
        &self,
        samples: &[f32],
        sample_rate: u32,
        tuning: &dyn Tuning,
    ) -> Result<AudioLiftResult, AudioLiftError> {
        Ok(self.lift_report(samples, sample_rate, tuning)?.value)
    }
}

/// An [`AudioLifter`] that detects pitches from spectral peaks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FftPeakLifter {
    /// Analysis options.
    pub opts: AudioLiftOptions,
}

/// An [`AudioLifter`] that detects pitches by harmonic-comb matching.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HarmonicCombLifter {
    /// Analysis options.
    pub opts: AudioLiftOptions,
}

impl AudioLifter for FftPeakLifter {
    fn symbol(&self) -> &'static str {
        "sound:FftPeakLifter"
    }

    fn lift_report(
        &self,
        samples: &[f32],
        sample_rate: u32,
        tuning: &dyn Tuning,
    ) -> Result<AudioLiftReport<AudioLiftResult>, AudioLiftError> {
        analyze(samples, sample_rate, tuning, &self.opts, false)
    }
}

impl AudioLifter for HarmonicCombLifter {
    fn symbol(&self) -> &'static str {
        "sound:HarmonicCombLifter"
    }

    fn lift_report(
        &self,
        samples: &[f32],
        sample_rate: u32,
        tuning: &dyn Tuning,
    ) -> Result<AudioLiftReport<AudioLiftResult>, AudioLiftError> {
        analyze(samples, sample_rate, tuning, &self.opts, true)
    }
}

/// Lifts `samples` with an [`FftPeakLifter`] using the given options.
pub fn lift_audio_fft_report(
    samples: &[f32],
    sample_rate: u32,
    tuning: &dyn Tuning,
    opts: AudioLiftOptions,
) -> Result<AudioLiftReport<AudioLiftResult>, AudioLiftError> {
    FftPeakLifter { opts }.lift_report(samples, sample_rate, tuning)
}

/// Lifts `samples` with a [`HarmonicCombLifter`] using the given options.
pub fn lift_audio_harmonic_report(
    samples: &[f32],
    sample_rate: u32,
    tuning: &dyn Tuning,
    opts: AudioLiftOptions,
) -> Result<AudioLiftReport<AudioLiftResult>, AudioLiftError> {
    HarmonicCombLifter { opts }.lift_report(samples, sample_rate, tuning)
}
