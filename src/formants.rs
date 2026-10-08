use std::f64::consts::PI;

const RATE: f64 = 16_000.0;
const WINDOW: usize = 512;
const ORDER: usize = 18;
const BINS: usize = 71;
const TAPS: usize = 31;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Formants {
    pub first: f32,
    pub second: f32,
}

impl Formants {
    pub fn shape(self, scale: f32) -> (f32, f32, f32) {
        let first = self.first / scale;
        let second = self.second / scale;
        let openness = ((first - 250.0) / 600.0).clamp(0.0, 1.0);
        let round = ((1500.0 - second) / 700.0).clamp(0.0, 1.0) * (1.0 - openness * 0.7);
        let wide = ((second - 1600.0) / 900.0).clamp(0.0, 1.0) * (1.0 - openness * 0.4);
        (openness, round, wide)
    }

    pub fn label(self, scale: f32) -> &'static str {
        let (open, round, wide) = self.shape(scale);
        if open > 0.6 {
            "A-like"
        } else if round > 0.3 {
            "O/U-like"
        } else if wide > 0.3 {
            "E/I-like"
        } else {
            "neutral"
        }
    }
}

pub struct Analyzer {
    filter: [f64; TAPS],
    input: [f64; TAPS],
    input_cursor: usize,
    decimation: usize,
    history: [f64; WINDOW],
    cursor: usize,
    count: usize,
    window: [f64; WINDOW],
    cosine: [[f64; ORDER + 1]; BINS],
    sine: [[f64; ORDER + 1]; BINS],
    last_formants: Option<Formants>,
    missing_samples: usize,
}

impl Default for Analyzer {
    fn default() -> Self {
        let mut filter = std::array::from_fn(|index| {
            let offset = index as f64 - (TAPS / 2) as f64;
            let cutoff = 6500.0 / 48_000.0;
            let sinc = if offset == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * PI * cutoff * offset).sin() / (PI * offset)
            };
            sinc * (0.54 - 0.46 * (2.0 * PI * index as f64 / (TAPS - 1) as f64).cos())
        });
        let sum: f64 = filter.iter().sum();
        for coefficient in &mut filter {
            *coefficient /= sum;
        }
        Self {
            filter,
            input: [0.0; TAPS],
            input_cursor: 0,
            decimation: 0,
            history: [0.0; WINDOW],
            cursor: 0,
            count: 0,
            window: std::array::from_fn(|index| {
                0.54 - 0.46 * (2.0 * PI * index as f64 / (WINDOW - 1) as f64).cos()
            }),
            cosine: std::array::from_fn(|bin| {
                std::array::from_fn(|order| {
                    (2.0 * PI * bin as f64 * 50.0 / RATE * order as f64).cos()
                })
            }),
            sine: std::array::from_fn(|bin| {
                std::array::from_fn(|order| {
                    (2.0 * PI * bin as f64 * 50.0 / RATE * order as f64).sin()
                })
            }),
            last_formants: None,
            missing_samples: 0,
        }
    }
}

impl Analyzer {
    pub fn analyze(&mut self, samples: &[f32]) -> Option<Formants> {
        let estimate = self.estimate(samples);
        if let Some(formants) = estimate {
            self.last_formants = Some(formants);
            self.missing_samples = 0;
        } else {
            self.missing_samples = self.missing_samples.saturating_add(samples.len());
            if self.missing_samples >= 2400 {
                self.last_formants = None;
            }
        }
        self.last_formants
    }

    fn estimate(&mut self, samples: &[f32]) -> Option<Formants> {
        for sample in samples {
            self.input[self.input_cursor] = if sample.is_finite() {
                *sample as f64
            } else {
                0.0
            };
            self.input_cursor = (self.input_cursor + 1) % TAPS;
            self.decimation += 1;
            if self.decimation == 3 {
                self.decimation = 0;
                let filtered = self
                    .filter
                    .iter()
                    .enumerate()
                    .map(|(index, coefficient)| {
                        coefficient * self.input[(self.input_cursor + TAPS - 1 - index) % TAPS]
                    })
                    .sum();
                self.history[self.cursor] = filtered;
                self.cursor = (self.cursor + 1) % WINDOW;
                self.count = (self.count + 1).min(WINDOW);
            }
        }
        if self.count < WINDOW {
            return None;
        }
        let mut frame: [f64; WINDOW] =
            std::array::from_fn(|index| self.history[(self.cursor + index) % WINDOW]);
        let mean = frame.iter().sum::<f64>() / WINDOW as f64;
        for sample in &mut frame {
            *sample -= mean;
        }
        let energy = frame.iter().map(|sample| sample * sample).sum::<f64>();
        if energy < 1e-8 {
            return None;
        }
        let periodicity = (32..=200)
            .map(|lag| {
                let mut correlation = 0.0;
                let mut left_energy = 0.0;
                let mut right_energy = 0.0;
                for index in lag..WINDOW {
                    correlation += frame[index] * frame[index - lag];
                    left_energy += frame[index] * frame[index];
                    right_energy += frame[index - lag] * frame[index - lag];
                }
                correlation / (left_energy * right_energy).sqrt().max(1e-12)
            })
            .fold(0.0_f64, f64::max);
        if periodicity < 0.45 {
            return None;
        }
        for index in (1..WINDOW).rev() {
            frame[index] = (frame[index] - 0.97 * frame[index - 1]) * self.window[index];
        }
        frame[0] *= self.window[0];
        let correlation: [f64; ORDER + 1] = std::array::from_fn(|lag| {
            (lag..WINDOW)
                .map(|index| frame[index] * frame[index - lag])
                .sum()
        });
        let mut coefficients = [0.0; ORDER + 1];
        coefficients[0] = 1.0;
        let mut error = correlation[0] * 1.00001;
        for order in 1..=ORDER {
            let reflection = -(correlation[order]
                + (1..order)
                    .map(|index| coefficients[index] * correlation[order - index])
                    .sum::<f64>())
                / error;
            if !reflection.is_finite() || reflection.abs() >= 0.9999 {
                return None;
            }
            let previous = coefficients;
            for index in 1..order {
                coefficients[index] += reflection * previous[order - index];
            }
            coefficients[order] = reflection;
            error *= 1.0 - reflection * reflection;
        }
        let spectrum: [f64; BINS] = std::array::from_fn(|bin| {
            let real: f64 = coefficients
                .iter()
                .zip(self.cosine[bin])
                .map(|(coefficient, basis)| coefficient * basis)
                .sum();
            let imaginary: f64 = coefficients
                .iter()
                .zip(self.sine[bin])
                .map(|(coefficient, basis)| coefficient * basis)
                .sum();
            1.0 / (real * real + imaginary * imaginary).max(1e-12)
        });
        let maximum = spectrum[3..=60].iter().copied().fold(0.0_f64, f64::max);
        let mut peaks = (3..=60).filter(|&bin| {
            spectrum[bin] > spectrum[bin - 1]
                && spectrum[bin] > spectrum[bin + 1]
                && spectrum[bin] > spectrum[bin - 3].min(spectrum[bin + 3]) * 1.1
                && spectrum[bin] > maximum * 0.002
        });
        let first = peaks.find(|&bin| bin <= 22)?;
        let second = peaks.find(|&bin| bin >= first + 4)?;
        if first > 10 && second - first > 40 {
            return None;
        }
        Some(Formants {
            first: first as f32 * 50.0,
            second: second as f32 * 50.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vowel(first: f64, second: f64, pitch: f64) -> Vec<f32> {
        let mut samples: Vec<f64> = (0..24_000)
            .map(|index| {
                if (index as f64 * pitch / 48_000.0).fract() < pitch / 48_000.0 {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        for (frequency, bandwidth) in [(first, 90.0), (second, 120.0), (3000.0, 180.0)] {
            let radius = (-PI * bandwidth / 48_000.0).exp();
            let feedback = 2.0 * radius * (2.0 * PI * frequency / 48_000.0).cos();
            let mut previous = 0.0;
            let mut older = 0.0;
            for sample in &mut samples {
                let filtered = *sample + feedback * previous - radius * radius * older;
                older = previous;
                previous = filtered;
                *sample = filtered;
            }
        }
        let peak = samples
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0, f64::max);
        samples
            .into_iter()
            .map(|sample| (sample / peak * 0.2) as f32)
            .collect()
    }

    #[test]
    fn recognizes_synthetic_vowel_resonances_across_pitch_and_level() {
        for (first, second) in [(800.0, 1200.0), (300.0, 2400.0), (350.0, 850.0)] {
            for pitch in [100.0, 180.0, 250.0] {
                let samples = vowel(first, second, pitch);
                for amplitude in [1.0, 0.1] {
                    let mut analyzer = Analyzer::default();
                    let mut estimates = Vec::new();
                    for block in samples.chunks(480) {
                        let block: Vec<f32> =
                            block.iter().map(|sample| sample * amplitude).collect();
                        if let Some(formants) = analyzer.analyze(&block) {
                            estimates.push(formants);
                        }
                    }
                    assert!(
                        estimates.len() > 30,
                        "{first}/{second} pitch {pitch}: {} estimates",
                        estimates.len()
                    );
                    for estimate in estimates.iter().skip(5) {
                        assert!(
                            (estimate.first as f64 - first).abs() <= 200.0,
                            "{first}/{second} pitch {pitch}: {estimate:?}"
                        );
                        assert!(
                            (estimate.second as f64 - second).abs() <= 200.0,
                            "{first}/{second} pitch {pitch}: {estimate:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn silence_dc_noise_and_pure_tones_do_not_produce_vowels() {
        let mut seed = 1234567_u32;
        for kind in 0..5 {
            let mut analyzer = Analyzer::default();
            for block_index in 0..40 {
                let samples: [f32; 480] = std::array::from_fn(|index| match kind {
                    0 => 0.0,
                    1 => 0.3,
                    2 => f32::NAN,
                    3 => {
                        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                        (seed as f64 / u32::MAX as f64 - 0.5) as f32 * 0.2
                    }
                    _ => {
                        (2.0 * PI * 440.0 * (block_index * 480 + index) as f64 / 48_000.0).sin()
                            as f32
                            * 0.2
                    }
                });
                assert_eq!(
                    analyzer.analyze(&samples),
                    None,
                    "kind {kind}, block {block_index}"
                );
            }
        }
    }

    #[test]
    fn shapes_and_normalization_are_bounded_and_distinct() {
        let open = Formants {
            first: 800.0,
            second: 1200.0,
        };
        let wide = Formants {
            first: 300.0,
            second: 2400.0,
        };
        let round = Formants {
            first: 350.0,
            second: 850.0,
        };
        assert_eq!(open.label(1.0), "A-like");
        assert_eq!(wide.label(1.0), "E/I-like");
        assert_eq!(round.label(1.0), "O/U-like");
        let normalized = Formants {
            first: 360.0,
            second: 2880.0,
        };
        let expected = wide.shape(1.0);
        let actual = normalized.shape(1.2);
        assert!((expected.2 - actual.2).abs() < 0.0001);
        for first in [0.0, 300.0, 800.0, 2000.0] {
            for second in [0.0, 850.0, 2400.0, 5000.0] {
                let (open, round, wide) = Formants { first, second }.shape(1.0);
                assert!(
                    [open, round, wide]
                        .iter()
                        .all(|value| (0.0..=1.0).contains(value))
                );
            }
        }
    }

    #[test]
    fn held_resonances_expire_and_analyzers_are_independent() {
        let samples = vowel(800.0, 1200.0, 100.0);
        let mut first = Analyzer::default();
        let mut second = Analyzer::default();
        for block in samples.chunks(480) {
            first.analyze(block);
        }
        assert!(first.last_formants.is_some());
        assert_eq!(second.analyze(&[0.0; 480]), None);
        for _ in 0..15 {
            first.analyze(&[0.0; 480]);
        }
        assert_eq!(first.last_formants, None);
    }
}
