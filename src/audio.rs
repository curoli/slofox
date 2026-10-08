use std::{
    collections::VecDeque,
    io::{self, Read},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::formants::{Analyzer, Formants};

pub const SAMPLE_RATE: usize = 48_000;
const BLOCK_SAMPLES: usize = 480;
const MAX_PACKETS: usize = 256;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Features {
    pub rms: f32,
    pub formants: Option<Formants>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub serial: String,
    pub name: String,
    pub description: String,
    pub is_sink: bool,
}

pub fn parse_devices(json: &str) -> Result<Vec<Device>, String> {
    let objects: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|error| format!("Invalid pw-dump JSON: {error}"))?;
    let mut devices = Vec::new();
    for object in objects {
        if object["type"] != "PipeWire:Interface:Node" {
            continue;
        }
        let properties = &object["info"]["props"];
        let class = properties["media.class"].as_str().unwrap_or("");
        if !matches!(class, "Audio/Source" | "Audio/Sink") {
            continue;
        }
        let Some(name) = properties["node.name"].as_str() else {
            continue;
        };
        let serial = match &properties["object.serial"] {
            serde_json::Value::Number(number) => number.to_string(),
            serde_json::Value::String(value) => value.clone(),
            _ => continue,
        };
        devices.push(Device {
            serial,
            name: name.to_owned(),
            description: properties["node.description"]
                .as_str()
                .unwrap_or(name)
                .to_owned(),
            is_sink: class == "Audio/Sink",
        });
    }
    devices.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(devices)
}

pub fn devices() -> Result<Vec<Device>, String> {
    let output = Command::new("pw-dump")
        .output()
        .map_err(|error| format!("Cannot run pw-dump: {error}. Install pipewire-bin."))?;
    if !output.status.success() {
        return Err(format!(
            "Cannot connect to PipeWire: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_devices(&String::from_utf8_lossy(&output.stdout))
}

pub fn validate_target(devices: &[Device], target: &str, sink: bool) -> Result<(), String> {
    if !sink && target == "auto" {
        return Ok(());
    }
    if devices
        .iter()
        .any(|device| device.is_sink == sink && (device.name == target || device.serial == target))
    {
        Ok(())
    } else {
        Err(format!(
            "No {} named '{target}'. Use --list-devices.{}",
            if sink {
                "output sink"
            } else {
                "microphone source"
            },
            if sink {
                " Start scripts/browser-audio.sh and route your browser to Slofox Browser."
            } else {
                ""
            }
        ))
    }
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let count = samples.iter().filter(|sample| sample.is_finite()).count();
    if count == 0 {
        return 0.0;
    }
    let mean = samples
        .iter()
        .filter(|sample| sample.is_finite())
        .sum::<f32>()
        / count as f32;
    (samples
        .iter()
        .filter(|sample| sample.is_finite())
        .map(|sample| (sample - mean).powi(2))
        .sum::<f32>()
        / count as f32)
        .sqrt()
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SpeechPose {
    pub jaw_open: f32,
    pub lip_round: f32,
    pub lip_wide: f32,
}

#[derive(Debug, Default)]
pub struct Envelope {
    level: f32,
    round: f32,
    wide: f32,
}

impl Envelope {
    pub fn update(&mut self, rms: f32, gain: f32, threshold: f32, seconds: f32) -> SpeechPose {
        self.update_features(
            Features {
                rms,
                formants: None,
            },
            gain,
            threshold,
            seconds,
            1.0,
        )
    }

    pub fn update_features(
        &mut self,
        features: Features,
        gain: f32,
        threshold: f32,
        seconds: f32,
        scale: f32,
    ) -> SpeechPose {
        let rms = features.rms;
        let target = if rms.is_finite() && rms > threshold {
            ((rms - threshold) * gain).sqrt().clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (open, round, wide) = if target > 0.0 {
            features
                .formants
                .map_or((1.0, 0.0, 0.0), |formants| formants.shape(scale))
        } else {
            (1.0, 0.0, 0.0)
        };
        let jaw_target = target * (0.4 + 0.6 * open);
        let time_constant = if jaw_target > self.level { 0.035 } else { 0.12 };
        let amount = 1.0 - (-seconds.max(0.0) / time_constant).exp();
        self.level += (jaw_target - self.level) * amount;
        let shape_amount = 1.0 - (-seconds.max(0.0) / 0.08).exp();
        self.round += (round * target - self.round) * shape_amount;
        self.wide += (wide * target - self.wide) * shape_amount;
        SpeechPose {
            jaw_open: self.level,
            lip_round: self.round,
            lip_wide: self.wide,
        }
    }
}

#[derive(Clone, Copy)]
struct Packet {
    time: Instant,
    features: Features,
}

#[derive(Default)]
struct CaptureState {
    packets: VecDeque<Packet>,
    last_packet: Option<Instant>,
    error: Option<String>,
}

#[derive(Clone, Default)]
pub struct Signal {
    state: Arc<Mutex<CaptureState>>,
}

impl Signal {
    #[cfg(test)]
    fn push(&self, rms: f32, time: Instant) {
        self.push_features(
            Features {
                rms,
                formants: None,
            },
            time,
        );
    }

    fn push_features(&self, features: Features, time: Instant) {
        let mut state = self.state.lock().unwrap();
        if state.packets.len() >= MAX_PACKETS {
            state.packets.pop_front();
        }
        state.packets.push_back(Packet { time, features });
        state.last_packet = Some(time);
    }

    fn fail(&self, error: String) {
        self.state.lock().unwrap().error = Some(error);
    }

    pub fn status(&self, now: Instant) -> String {
        let state = self.state.lock().unwrap();
        if let Some(error) = &state.error {
            return error.clone();
        }
        match state.last_packet {
            Some(time) if now.saturating_duration_since(time) < Duration::from_millis(500) => {
                "live".into()
            }
            Some(_) => "no audio packets".into(),
            None => "waiting for audio".into(),
        }
    }
}

pub struct Reader {
    pub signal: Signal,
    delay: Duration,
    latest: Option<Packet>,
}

impl Reader {
    pub fn new(signal: Signal, delay_ms: u64) -> Self {
        Self {
            signal,
            delay: Duration::from_millis(delay_ms),
            latest: None,
        }
    }

    pub fn sample(&mut self, now: Instant) -> f32 {
        self.features(now).rms
    }

    pub fn features(&mut self, now: Instant) -> Features {
        let target = now.checked_sub(self.delay).unwrap_or(now);
        let mut state = self.signal.state.lock().unwrap();
        while state
            .packets
            .front()
            .is_some_and(|packet| packet.time <= target)
        {
            self.latest = state.packets.pop_front();
        }
        self.latest
            .filter(|packet| {
                target.saturating_duration_since(packet.time) < Duration::from_millis(250)
            })
            .map_or(Features::default(), |packet| packet.features)
    }
}

pub struct Capture {
    child: Child,
    worker: Option<JoinHandle<()>>,
    pub signal: Signal,
}

impl Capture {
    pub fn start(target: &str, sink: bool, label: &str) -> Result<Self, String> {
        let mut child = Command::new("pw-record")
            .args([
                "--raw",
                "--rate",
                "48000",
                "--channels",
                "1",
                "--format",
                "f32",
                "--latency",
                "20ms",
                "--target",
                target,
                "--properties",
                &format!(
                    "{{ application.name = \"Slofox {label}\" stream.capture.sink = {sink} }}"
                ),
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("Cannot start pw-record: {error}. Install pipewire-bin."))?;
        let mut output = child.stdout.take().unwrap();
        let signal = Signal::default();
        let worker_signal = signal.clone();
        let worker = match thread::Builder::new()
            .name(format!("audio-{label}"))
            .spawn(move || {
                let mut bytes = [0_u8; BLOCK_SAMPLES * 4];
                let mut samples = [0_f32; BLOCK_SAMPLES];
                let mut analyzer = Analyzer::default();
                loop {
                    if let Err(error) = output.read_exact(&mut bytes) {
                        worker_signal.fail(if error.kind() == io::ErrorKind::UnexpectedEof {
                            "audio capture stopped (see terminal)".into()
                        } else {
                            format!("audio read failed: {error}")
                        });
                        break;
                    }
                    for (sample, chunk) in samples.iter_mut().zip(bytes.as_chunks::<4>().0) {
                        *sample = f32::from_ne_bytes(*chunk);
                    }
                    let formants = analyzer.analyze(&samples);
                    worker_signal.push_features(
                        Features {
                            rms: rms(&samples),
                            formants,
                        },
                        Instant::now(),
                    );
                }
            }) {
            Ok(worker) => worker,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Cannot start audio thread: {error}"));
            }
        };
        Ok(Self {
            child,
            worker: Some(worker),
            signal,
        })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn demo_level(seconds: f32, host: usize) -> f32 {
    let phase = seconds.rem_euclid(12.0);
    let speaking = if host == 0 {
        phase < 5.2
    } else {
        (6.0..11.2).contains(&phase)
    };
    if speaking {
        0.025 + 0.11 * (seconds * 8.7 + host as f32).sin().abs() * (seconds * 3.1).sin().abs()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_removes_dc_and_ignores_nonfinite_samples() {
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(rms(&[0.5, 0.5]), 0.0);
        assert!((rms(&[-0.5, 0.5, f32::NAN]) - 0.5).abs() < 0.0001);
        assert_eq!(rms(&[f32::INFINITY]), 0.0);
    }

    #[test]
    fn envelope_is_bounded_and_releases_after_speech() {
        let mut envelope = Envelope::default();
        assert_eq!(envelope.update(0.005, 8.0, 0.008, 1.0).jaw_open, 0.0);
        let loud = envelope.update(1.0, 8.0, 0.008, 0.1).jaw_open;
        assert!((0.9..=1.0).contains(&loud));
        assert!(envelope.update(0.0, 8.0, 0.008, 0.1).jaw_open < loud);
        assert!(envelope.update(f32::NAN, 8.0, 0.008, 1.0).jaw_open < 0.001);
    }

    #[test]
    fn smoothing_is_independent_of_render_frame_rate() {
        let mut slow = Envelope::default();
        let mut fast = Envelope::default();
        let mut slow_pose = SpeechPose::default();
        let mut fast_pose = SpeechPose::default();
        for _ in 0..30 {
            slow_pose = slow.update(0.1, 8.0, 0.008, 1.0 / 30.0);
        }
        for _ in 0..120 {
            fast_pose = fast.update(0.1, 8.0, 0.008, 1.0 / 120.0);
        }
        assert!((slow_pose.jaw_open - fast_pose.jaw_open).abs() < 0.0001);
    }

    #[test]
    fn delay_preserves_packets_and_stale_audio_becomes_silent() {
        let signal = Signal::default();
        let now = Instant::now();
        signal.push(0.2, now);
        let mut reader = Reader::new(signal, 100);
        assert_eq!(reader.sample(now), 0.0);
        assert_eq!(reader.sample(now + Duration::from_millis(100)), 0.2);
        assert_eq!(reader.sample(now + Duration::from_millis(400)), 0.0);
    }

    #[test]
    fn inputs_are_independent_and_buffers_are_bounded() {
        let first = Signal::default();
        let second = Signal::default();
        let now = Instant::now();
        for _ in 0..1000 {
            first.push(0.4, now);
        }
        assert_eq!(first.state.lock().unwrap().packets.len(), MAX_PACKETS);
        assert_eq!(Reader::new(first, 0).sample(now), 0.4);
        assert_eq!(Reader::new(second, 0).sample(now), 0.0);
    }

    #[test]
    fn delay_keeps_formants_and_volume_together_and_expires_both() {
        let signal = Signal::default();
        let other = Signal::default();
        let now = Instant::now();
        let first = Features {
            rms: 0.2,
            formants: Some(Formants {
                first: 350.0,
                second: 850.0,
            }),
        };
        let second = Features {
            rms: 0.1,
            formants: Some(Formants {
                first: 300.0,
                second: 2400.0,
            }),
        };
        signal.push_features(first, now);
        signal.push_features(second, now + Duration::from_millis(10));
        let mut reader = Reader::new(signal, 100);
        assert_eq!(reader.features(now), Features::default());
        assert_eq!(reader.features(now + Duration::from_millis(100)), first);
        assert_eq!(reader.features(now + Duration::from_millis(110)), second);
        assert_eq!(
            reader.features(now + Duration::from_millis(400)),
            Features::default()
        );
        assert_eq!(Reader::new(other, 0).features(now), Features::default());
    }

    #[test]
    fn shapes_transition_smoothly_and_release_at_silence() {
        let mut envelope = Envelope::default();
        let round = Features {
            rms: 0.1,
            formants: Some(Formants {
                first: 350.0,
                second: 850.0,
            }),
        };
        let wide = Features {
            rms: 0.1,
            formants: Some(Formants {
                first: 300.0,
                second: 2400.0,
            }),
        };
        let previous = envelope.update_features(round, 8.0, 0.008, 1.0, 1.0);
        assert!(previous.lip_round > 0.6 && previous.lip_wide == 0.0);
        let next = envelope.update_features(wide, 8.0, 0.008, 1.0 / 30.0, 1.0);
        assert!(next.lip_round > 0.0 && next.lip_round < previous.lip_round);
        assert!(next.lip_wide > 0.0 && next.lip_wide < 0.4);
        let silence =
            envelope.update_features(Features { rms: 0.0, ..round }, 8.0, 0.008, 2.0, 1.0);
        assert!(silence.jaw_open < 0.001 && silence.lip_round < 0.001 && silence.lip_wide < 0.001);
        let fallback = envelope.update(0.1, 8.0, 0.008, 2.0);
        assert!(fallback.jaw_open > previous.jaw_open);
        assert!(fallback.lip_round < 0.001 && fallback.lip_wide < 0.001);
    }

    #[test]
    fn all_shape_coefficients_are_frame_rate_independent() {
        let features = Features {
            rms: 0.1,
            formants: Some(Formants {
                first: 350.0,
                second: 850.0,
            }),
        };
        let mut poses = Vec::new();
        for fps in [30, 120] {
            let mut envelope = Envelope::default();
            let mut pose = SpeechPose::default();
            for _ in 0..fps {
                pose = envelope.update_features(features, 8.0, 0.008, 1.0 / fps as f32, 1.0);
            }
            poses.push(pose);
        }
        assert!((poses[0].jaw_open - poses[1].jaw_open).abs() < 0.0001);
        assert!((poses[0].lip_round - poses[1].lip_round).abs() < 0.0001);
        assert!((poses[0].lip_wide - poses[1].lip_wide).abs() < 0.0001);
    }

    #[test]
    fn device_discovery_distinguishes_sinks_sources_and_streams() {
        let json = r#"[
            {"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Sink","node.name":"browser","object.serial":42}}},
            {"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Source","node.name":"mic","object.serial":"43"}}},
            {"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Stream/Output/Audio","node.name":"firefox","object.serial":44}}}
        ]"#;
        let devices = parse_devices(json).unwrap();
        assert_eq!(devices.len(), 2);
        assert!(validate_target(&devices, "browser", true).is_ok());
        assert!(validate_target(&devices, "43", false).is_ok());
        assert!(validate_target(&devices, "auto", false).is_ok());
        assert!(validate_target(&devices, "mic", true).is_err());
        assert!(parse_devices("not JSON").is_err());
    }
}
