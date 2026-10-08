use std::f64::consts::PI;

const RATE: f64 = 12_000.0;
const WINDOW: usize = 384;
const ORDER: usize = 12;
const BINS: usize = 71;
const TAPS: usize = 31;

#[derive(Clone, Copy)]
struct Complex {
    real: f64,
    imaginary: f64,
}

impl Complex {
    fn subtract(self, other: Self) -> Self {
        Self {
            real: self.real - other.real,
            imaginary: self.imaginary - other.imaginary,
        }
    }

    fn multiply(self, other: Self) -> Self {
        Self {
            real: self.real * other.real - self.imaginary * other.imaginary,
            imaginary: self.real * other.imaginary + self.imaginary * other.real,
        }
    }

    fn divide(self, other: Self) -> Self {
        let norm = other.real.powi(2) + other.imaginary.powi(2);
        Self {
            real: (self.real * other.real + self.imaginary * other.imaginary) / norm,
            imaginary: (self.imaginary * other.real - self.real * other.imaginary) / norm,
        }
    }

    fn norm(self) -> f64 {
        self.real.hypot(self.imaginary)
    }
}

fn resonances(coefficients: &[f64; ORDER + 1]) -> Option<Vec<(f64, f64)>> {
    let mut roots: [Complex; ORDER] = std::array::from_fn(|index| {
        let angle = 2.0 * PI * (index as f64 + 0.37) / ORDER as f64;
        Complex {
            real: 0.9 * angle.cos(),
            imaginary: 0.9 * angle.sin(),
        }
    });
    for _ in 0..100 {
        let mut largest_change = 0.0_f64;
        for index in 0..ORDER {
            let root = roots[index];
            let mut value = Complex {
                real: 1.0,
                imaginary: 0.0,
            };
            for coefficient in &coefficients[1..] {
                value = value.multiply(root);
                value.real += coefficient;
            }
            let mut denominator = Complex {
                real: 1.0,
                imaginary: 0.0,
            };
            for (other_index, other_root) in roots.iter().enumerate() {
                if index != other_index {
                    denominator = denominator.multiply(root.subtract(*other_root));
                }
            }
            let change = value.divide(denominator);
            if !change.norm().is_finite() {
                return None;
            }
            largest_change = largest_change.max(change.norm());
            roots[index] = root.subtract(change);
        }
        if largest_change < 1e-8 {
            let mut resonances: Vec<_> = roots
                .iter()
                .filter(|root| root.imaginary > 0.0)
                .map(|root| {
                    (
                        root.imaginary.atan2(root.real) * RATE / (2.0 * PI),
                        -root.norm().ln() * RATE / PI,
                    )
                })
                .filter(|&(frequency, bandwidth)| {
                    (150.0..=3200.0).contains(&frequency) && (0.0..=1500.0).contains(&bandwidth)
                })
                .collect();
            resonances.sort_by(|left, right| left.0.total_cmp(&right.0));
            return Some(resonances);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Formants {
    pub first: f32,
    pub second: f32,
}

#[derive(Clone, Copy)]
struct Candidate {
    frequency: f64,
    bandwidth: f64,
    strength: f64,
}

fn select_formants(candidates: &[Candidate]) -> Option<Formants> {
    let mut eligible = candidates
        .iter()
        .filter(|candidate| (30.0..=500.0).contains(&candidate.bandwidth));
    let first = eligible.next()?;
    let second = eligible.find(|candidate| candidate.frequency >= first.frequency + 100.0)?;
    if first.frequency > 1100.0
        || [first, second]
            .iter()
            .any(|candidate| candidate.strength <= 0.002)
        || (first.frequency > 500.0 && second.frequency - first.frequency > 2000.0)
    {
        return None;
    }
    Some(Formants {
        first: first.frequency as f32,
        second: second.frequency as f32,
    })
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
    candidates: Vec<Candidate>,
    reason: &'static str,
}

impl Default for Analyzer {
    fn default() -> Self {
        let mut filter = std::array::from_fn(|index| {
            let offset = index as f64 - (TAPS / 2) as f64;
            let cutoff = 5000.0 / 48_000.0;
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
            candidates: Vec::new(),
            reason: "waiting for audio",
        }
    }
}

impl Analyzer {
    pub fn diagnostics(&self) -> String {
        let candidates = self
            .candidates
            .iter()
            .map(|candidate| {
                format!(
                    "{:.0}Hz/BW{:.0}Hz/{:.4}",
                    candidate.frequency, candidate.bandwidth, candidate.strength
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{}; poles [{}] (frequency/bandwidth/relative power)",
            self.reason, candidates
        )
    }

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
        self.candidates.clear();
        self.reason = "warming up";
        for sample in samples {
            self.input[self.input_cursor] = if sample.is_finite() {
                *sample as f64
            } else {
                0.0
            };
            self.input_cursor = (self.input_cursor + 1) % TAPS;
            self.decimation += 1;
            if self.decimation == 4 {
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
            self.reason = "silent";
            return None;
        }
        let periodicity = (24..=150)
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
            self.reason = "unvoiced";
            return None;
        }
        for index in (1..WINDOW).rev() {
            frame[index] = (frame[index] - 0.97 * frame[index - 1]) * self.window[index];
        }
        frame[0] *= self.window[0];
        let mut coefficients = [0.0; ORDER + 1];
        self.reason = "unstable LPC";
        coefficients[0] = 1.0;
        let mut forward = frame;
        let mut backward = frame;
        for order in 1..=ORDER {
            let mut numerator = 0.0;
            let mut denominator = 0.0;
            for index in order..WINDOW {
                numerator += forward[index] * backward[index - 1];
                denominator += forward[index].powi(2) + backward[index - 1].powi(2);
            }
            let reflection = -2.0 * numerator / denominator.max(1e-20);
            if !reflection.is_finite() || reflection.abs() >= 0.9999 {
                return None;
            }
            let previous = coefficients;
            for index in 1..order {
                coefficients[index] += reflection * previous[order - index];
            }
            coefficients[order] = reflection;
            for index in (order..WINDOW).rev() {
                let old_forward = forward[index];
                let old_backward = backward[index - 1];
                forward[index] = old_forward + reflection * old_backward;
                backward[index] = old_backward + reflection * old_forward;
            }
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
        self.reason = "root solver did not converge";
        let resonances = resonances(&coefficients)?;
        self.candidates
            .extend(resonances.into_iter().map(|(frequency, bandwidth)| {
                let bin = (frequency / 50.0).round() as usize;
                Candidate {
                    frequency,
                    bandwidth,
                    strength: spectrum[bin] / maximum.max(1e-12),
                }
            }));
        let formants = select_formants(&self.candidates);
        self.reason = if formants.is_some() {
            "accepted F1/F2"
        } else {
            "uncertain F1/F2: volume fallback after hold"
        };
        formants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_second_formant_is_not_replaced_by_a_strong_third() {
        let mut candidates = [
            Candidate {
                frequency: 432.0,
                bandwidth: 100.0,
                strength: 1.0,
            },
            Candidate {
                frequency: 850.0,
                bandwidth: 150.0,
                strength: 0.001,
            },
            Candidate {
                frequency: 3038.0,
                bandwidth: 100.0,
                strength: 0.1,
            },
        ];
        assert_eq!(select_formants(&candidates), None);
        candidates[1].strength = 0.01;
        assert_eq!(
            select_formants(&candidates),
            Some(Formants {
                first: 432.0,
                second: 850.0
            })
        );
        candidates[0].strength = 0.001;
        assert_eq!(select_formants(&candidates), None);
    }

    #[test]
    fn diagnostics_include_rejected_poles_without_audio_samples() {
        let analyzer = Analyzer {
            candidates: vec![Candidate {
                frequency: 850.0,
                bandwidth: 650.0,
                strength: 0.001,
            }],
            reason: "uncertain F1/F2: volume fallback after hold",
            ..Default::default()
        };
        let description = analyzer.diagnostics();
        assert!(description.contains("volume fallback"));
        assert!(description.contains("850Hz/BW650Hz/0.0010"));
        assert!(
            Analyzer::default()
                .diagnostics()
                .contains("waiting for audio; poles []")
        );
    }

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
                        "{first}/{second} pitch {pitch}: {} estimates; {}",
                        estimates.len(),
                        analyzer.diagnostics()
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
    fn rounded_vowels_remain_rounded_after_other_vowels() {
        for pitch in [100.0, 140.0, 180.0] {
            let mut analyzer = Analyzer::default();
            for (first, second, label) in [
                (300.0, 2400.0, "E/I-like"),
                (250.0, 600.0, "O/U-like"),
                (800.0, 1200.0, "A-like"),
                (300.0, 700.0, "O/U-like"),
            ] {
                let samples = vowel(first, second, pitch);
                let mut correct = 0;
                let mut wrong = Vec::new();
                for block in samples.chunks(480).cycle().take(300) {
                    let estimate = analyzer.analyze(block);
                    if let Some(formants) = estimate {
                        if formants.label(1.0) == label {
                            correct += 1;
                        } else {
                            wrong.push(formants);
                        }
                    }
                }
                assert!(
                    correct > 270,
                    "{first}/{second} pitch {pitch}: {correct} correct, wrong {wrong:?}"
                );
            }
        }
    }

    #[test]
    fn identifies_close_poles_even_without_separate_spectral_peaks() {
        let mut coefficients = [0.0; ORDER + 1];
        coefficients[0] = 1.0;
        let mut degree = 0;
        for (frequency, bandwidth) in [(250.0, 90.0), (600.0, 120.0), (3000.0, 180.0)] {
            let radius = (-PI * bandwidth / RATE).exp();
            let linear = -2.0 * radius * (2.0 * PI * frequency / RATE).cos();
            let quadratic = radius * radius;
            let previous = coefficients;
            coefficients.fill(0.0);
            for index in 0..=degree {
                coefficients[index] += previous[index];
                coefficients[index + 1] += previous[index] * linear;
                coefficients[index + 2] += previous[index] * quadratic;
            }
            degree += 2;
        }
        let estimates = resonances(&coefficients).unwrap();
        assert_eq!(estimates.len(), 3);
        for ((frequency, bandwidth), (expected_frequency, expected_bandwidth)) in estimates
            .into_iter()
            .zip([(250.0, 90.0), (600.0, 120.0), (3000.0, 180.0)])
        {
            assert!((frequency - expected_frequency).abs() < 0.01);
            assert!((bandwidth - expected_bandwidth).abs() < 0.01);
        }
    }

    #[test]
    fn rounded_vowels_survive_source_tilt_and_low_background_noise() {
        let mut seed = 7531_u32;
        for (first, second) in [(250.0, 600.0), (300.0, 700.0), (350.0, 850.0)] {
            let mut samples = vowel(first, second, 140.0);
            let mut previous = 0.0;
            for sample in &mut samples {
                previous += 0.15 * (*sample - previous);
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = (seed as f64 / u32::MAX as f64 - 0.5) as f32 * 0.0005;
                *sample = previous + noise;
            }
            let mut analyzer = Analyzer::default();
            let mut correct = 0;
            for block in samples.chunks(480) {
                if analyzer
                    .analyze(block)
                    .is_some_and(|formants| formants.label(1.0) == "O/U-like")
                {
                    correct += 1;
                }
            }
            assert!(correct > 35, "{first}/{second}: {correct} correct");
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
