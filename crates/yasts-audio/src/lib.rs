//! Audio analysis primitives for sound-reactive LED control.
//!
//! This crate intentionally knows nothing about microphones, controllers, or
//! LED protocols. It accepts normalized audio samples and returns compact,
//! normalized features that another layer can turn into lighting behavior.

use std::collections::VecDeque;
use std::f32::consts::PI;

const DEFAULT_HISTORY_LEN: usize = 43;
const MIN_BEAT_THRESHOLD: f32 = 0.01;

/// Configuration for [`Analyzer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalyzerConfig {
    /// Input sample rate in Hertz.
    pub sample_rate_hz: u32,
    /// Expected frame size passed to [`Analyzer::process_frame`].
    pub frame_size: usize,
    /// Smoothing amount in the range `0.0..=1.0`.
    ///
    /// `0.0` follows the current frame immediately. `1.0` holds the previous
    /// value forever.
    pub smoothing: f32,
    /// Beat sensitivity in the range `0.0..=1.0`.
    ///
    /// Higher values make beat detection more selective.
    pub beat_sensitivity: f32,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            sample_rate_hz: 44_100,
            frame_size: 1024,
            smoothing: 0.65,
            beat_sensitivity: 0.5,
        }
    }
}

impl AnalyzerConfig {
    /// Returns a sanitized config with invalid values replaced or clamped.
    pub fn sanitized(self) -> Self {
        Self {
            sample_rate_hz: self.sample_rate_hz.max(1),
            frame_size: self.frame_size.max(1),
            smoothing: clamp01(self.smoothing),
            beat_sensitivity: clamp01(self.beat_sensitivity),
        }
    }
}

/// Normalized audio features suitable for driving lighting animation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AudioFeatures {
    /// Smoothed RMS loudness, normalized to `0.0..=1.0`.
    pub loudness: f32,
    /// Smoothed energy in the `20..250 Hz` range, normalized to `0.0..=1.0`.
    pub bass: f32,
    /// Smoothed energy in the `250..4000 Hz` range, normalized to `0.0..=1.0`.
    pub mids: f32,
    /// Smoothed energy in the `4000..12000 Hz` range, normalized to `0.0..=1.0`.
    pub treble: f32,
    /// True when the current frame looks like a beat-like energy increase.
    pub beat: bool,
    /// Beat intensity for the current frame, normalized to `0.0..=1.0`.
    pub beat_strength: f32,
}

/// Stateful analyzer for turning audio frames into lighting-friendly features.
#[derive(Debug, Clone)]
pub struct Analyzer {
    config: AnalyzerConfig,
    previous: AudioFeatures,
    energy_history: VecDeque<f32>,
}

impl Analyzer {
    /// Creates a new analyzer.
    ///
    /// Invalid config values are sanitized so processing remains deterministic
    /// for unusual inputs.
    pub fn new(config: AnalyzerConfig) -> Self {
        Self {
            config: config.sanitized(),
            previous: AudioFeatures::default(),
            energy_history: VecDeque::with_capacity(DEFAULT_HISTORY_LEN),
        }
    }

    /// Processes a frame of normalized `f32` samples.
    ///
    /// Input samples are clamped to `-1.0..=1.0`; `NaN` and infinite values are
    /// treated as silence.
    pub fn process_frame(&mut self, samples: &[f32]) -> AudioFeatures {
        if samples.is_empty() {
            return self.smooth(AudioFeatures::default());
        }

        let loudness = rms_loudness(samples);
        let (bass, mids, treble) = band_energies(samples, self.config.sample_rate_hz);
        let beat_strength = self.detect_beat(loudness, bass);

        self.smooth(AudioFeatures {
            loudness,
            bass,
            mids,
            treble,
            beat: beat_strength > 0.0,
            beat_strength,
        })
    }

    /// Returns the sanitized config currently used by the analyzer.
    pub fn config(&self) -> AnalyzerConfig {
        self.config
    }

    fn smooth(&mut self, current: AudioFeatures) -> AudioFeatures {
        let hold = self.config.smoothing;
        let follow = 1.0 - hold;
        let beat_strength = current.beat_strength;

        let smoothed = AudioFeatures {
            loudness: smooth_value(self.previous.loudness, current.loudness, hold, follow),
            bass: smooth_value(self.previous.bass, current.bass, hold, follow),
            mids: smooth_value(self.previous.mids, current.mids, hold, follow),
            treble: smooth_value(self.previous.treble, current.treble, hold, follow),
            beat: current.beat,
            beat_strength,
        };

        self.previous = smoothed;
        smoothed
    }

    fn detect_beat(&mut self, loudness: f32, bass: f32) -> f32 {
        let energy = clamp01((loudness * 0.35) + (bass * 0.65));

        let average = if self.energy_history.is_empty() {
            energy
        } else {
            self.energy_history.iter().sum::<f32>() / self.energy_history.len() as f32
        };

        self.energy_history.push_back(energy);
        if self.energy_history.len() > DEFAULT_HISTORY_LEN {
            self.energy_history.pop_front();
        }

        let sensitivity_threshold = 0.08 + (self.config.beat_sensitivity * 0.35);
        let threshold = average + sensitivity_threshold;
        if average < MIN_BEAT_THRESHOLD || energy <= threshold {
            0.0
        } else {
            clamp01((energy - threshold) / (1.0 - threshold).max(f32::EPSILON))
        }
    }
}

fn smooth_value(previous: f32, current: f32, hold: f32, follow: f32) -> f32 {
    clamp01((previous * hold) + (current * follow))
}

fn rms_loudness(samples: &[f32]) -> f32 {
    let sum = samples
        .iter()
        .map(|sample| {
            let value = sanitize_sample(*sample);
            value * value
        })
        .sum::<f32>();

    clamp01((sum / samples.len() as f32).sqrt())
}

fn band_energies(samples: &[f32], sample_rate_hz: u32) -> (f32, f32, f32) {
    let bass = band_energy(samples, sample_rate_hz, 20.0, 250.0);
    let mids = band_energy(samples, sample_rate_hz, 250.0, 4_000.0);
    let treble = band_energy(samples, sample_rate_hz, 4_000.0, 12_000.0);

    (bass, mids, treble)
}

fn band_energy(samples: &[f32], sample_rate_hz: u32, low_hz: f32, high_hz: f32) -> f32 {
    if samples.is_empty() || sample_rate_hz == 0 {
        return 0.0;
    }

    let len = samples.len();
    let bin_hz = sample_rate_hz as f32 / len as f32;
    let nyquist = sample_rate_hz as f32 / 2.0;
    let low = low_hz.max(bin_hz);
    let high = high_hz.min(nyquist);

    if high <= low {
        return 0.0;
    }

    let start_bin = (low / bin_hz).ceil() as usize;
    let end_bin = (high / bin_hz).floor() as usize;

    if start_bin > end_bin {
        return 0.0;
    }

    let mut energy = 0.0;
    let mut bins = 0usize;
    for bin in start_bin..=end_bin {
        let frequency = bin as f32 * bin_hz;
        energy += frequency_magnitude(samples, frequency, sample_rate_hz);
        bins += 1;
    }

    if bins == 0 {
        0.0
    } else {
        clamp01((energy / bins as f32) * 2.0)
    }
}

fn frequency_magnitude(samples: &[f32], frequency_hz: f32, sample_rate_hz: u32) -> f32 {
    let mut real = 0.0;
    let mut imaginary = 0.0;

    for (index, sample) in samples.iter().enumerate() {
        let angle = 2.0 * PI * frequency_hz * index as f32 / sample_rate_hz as f32;
        let value = sanitize_sample(*sample);
        real += value * angle.cos();
        imaginary -= value * angle.sin();
    }

    ((real * real) + (imaginary * imaginary)).sqrt() / samples.len() as f32
}

fn sanitize_sample(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

fn clamp01(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyzer_with_no_smoothing() -> Analyzer {
        Analyzer::new(AnalyzerConfig {
            smoothing: 0.0,
            beat_sensitivity: 0.5,
            ..AnalyzerConfig::default()
        })
    }

    fn sine_wave(
        frequency_hz: f32,
        amplitude: f32,
        sample_rate_hz: u32,
        frame_size: usize,
    ) -> Vec<f32> {
        (0..frame_size)
            .map(|index| {
                let phase = 2.0 * PI * frequency_hz * index as f32 / sample_rate_hz as f32;
                amplitude * phase.sin()
            })
            .collect()
    }

    fn assert_normalized(features: AudioFeatures) {
        for value in [
            features.loudness,
            features.bass,
            features.mids,
            features.treble,
            features.beat_strength,
        ] {
            assert!((0.0..=1.0).contains(&value), "{value} was not normalized");
        }
    }

    #[test]
    fn sanitizes_invalid_config_values() {
        let analyzer = Analyzer::new(AnalyzerConfig {
            sample_rate_hz: 0,
            frame_size: 0,
            smoothing: 3.0,
            beat_sensitivity: -2.0,
        });

        assert_eq!(
            analyzer.config(),
            AnalyzerConfig {
                sample_rate_hz: 1,
                frame_size: 1,
                smoothing: 1.0,
                beat_sensitivity: 0.0,
            }
        );
    }

    #[test]
    fn silence_returns_zero_features() {
        let mut analyzer = analyzer_with_no_smoothing();

        let features = analyzer.process_frame(&vec![0.0; 1024]);

        assert_eq!(features, AudioFeatures::default());
        assert_normalized(features);
    }

    #[test]
    fn empty_frames_are_valid_and_normalized() {
        let mut analyzer = analyzer_with_no_smoothing();

        let features = analyzer.process_frame(&[]);

        assert_eq!(features, AudioFeatures::default());
        assert_normalized(features);
    }

    #[test]
    fn loudness_uses_rms_and_clips_invalid_samples() {
        let mut analyzer = analyzer_with_no_smoothing();

        let features = analyzer.process_frame(&[1.0, -1.0, 2.0, f32::NAN, f32::INFINITY]);

        assert!((features.loudness - (3.0_f32 / 5.0).sqrt()).abs() < 0.0001);
        assert_normalized(features);
    }

    #[test]
    fn smoothing_reduces_abrupt_changes() {
        let mut analyzer = Analyzer::new(AnalyzerConfig {
            smoothing: 0.5,
            ..AnalyzerConfig::default()
        });

        let first = analyzer.process_frame(&vec![1.0; 1024]);
        let second = analyzer.process_frame(&vec![0.0; 1024]);

        assert!((first.loudness - 0.5).abs() < 0.0001);
        assert!((second.loudness - 0.25).abs() < 0.0001);
        assert_normalized(first);
        assert_normalized(second);
    }

    #[test]
    fn bass_sine_wave_produces_more_bass_than_other_bands() {
        let mut analyzer = analyzer_with_no_smoothing();
        let samples = sine_wave(120.0, 0.8, 44_100, 2048);

        let features = analyzer.process_frame(&samples);

        assert!(features.bass > features.mids);
        assert!(features.bass > features.treble);
        assert_normalized(features);
    }

    #[test]
    fn mid_sine_wave_produces_more_mids_than_other_bands() {
        let mut analyzer = analyzer_with_no_smoothing();
        let samples = sine_wave(1_000.0, 0.8, 44_100, 2048);

        let features = analyzer.process_frame(&samples);

        assert!(features.mids > features.bass);
        assert!(features.mids > features.treble);
        assert_normalized(features);
    }

    #[test]
    fn treble_sine_wave_produces_more_treble_than_other_bands() {
        let mut analyzer = analyzer_with_no_smoothing();
        let samples = sine_wave(8_000.0, 0.8, 44_100, 2048);

        let features = analyzer.process_frame(&samples);

        assert!(features.treble > features.bass);
        assert!(features.treble > features.mids);
        assert_normalized(features);
    }

    #[test]
    fn sudden_energy_increase_reports_beat() {
        let mut analyzer = Analyzer::new(AnalyzerConfig {
            smoothing: 0.0,
            beat_sensitivity: 0.0,
            ..AnalyzerConfig::default()
        });

        let quiet_bass = sine_wave(120.0, 0.1, 44_100, 2048);
        for _ in 0..8 {
            analyzer.process_frame(&quiet_bass);
        }

        let loud_bass = sine_wave(120.0, 0.95, 44_100, 2048);
        let features = analyzer.process_frame(&loud_bass);

        assert!(features.beat);
        assert!(features.beat_strength > 0.0);
        assert_normalized(features);
    }

    #[test]
    fn steady_energy_does_not_keep_reporting_beats() {
        let mut analyzer = Analyzer::new(AnalyzerConfig {
            smoothing: 0.0,
            beat_sensitivity: 0.0,
            ..AnalyzerConfig::default()
        });

        let mut last = AudioFeatures::default();
        for _ in 0..64 {
            last = analyzer.process_frame(&vec![0.6; 1024]);
        }

        assert!(!last.beat);
        assert_eq!(last.beat_strength, 0.0);
        assert_normalized(last);
    }
}
