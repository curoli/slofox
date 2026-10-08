#![cfg(target_os = "linux")]

use std::{
    fs,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use slofox::audio::{self, Capture, Reader, SAMPLE_RATE};

struct Process(Child);

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct TemporaryAudio(std::path::PathBuf);

impl Drop for TemporaryAudio {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn isolated_sink(name: &str) -> Process {
    Process(
        Command::new("pw-cli")
            .args([
                "--monitor", "create-node", "adapter",
                &format!("{{ factory.name = support.null-audio-sink node.name = {name} media.class = Audio/Sink audio.position = [ FL FR ] }}"),
            ])
            .stdout(Stdio::null())
            .spawn()
            .expect("pw-cli must be installed"),
    )
}

#[test]
#[ignore = "requires a running PipeWire session and pipewire-bin; creates isolated test sinks"]
fn captures_browser_monitor_and_microphone_source_without_cross_talk() {
    let first_name = format!("slofox_test_first_{}", std::process::id());
    let second_name = format!("slofox_test_second_{}", std::process::id());
    let source_name = format!("slofox_test_microphone_{}", std::process::id());
    let browser_name = format!("slofox_test_browser_{}", std::process::id());
    let _first_sink = isolated_sink(&first_name);
    let _second_sink = isolated_sink(&second_name);
    let _browser = Process(Command::new("pw-loopback")
        .args([
            "--capture-props", &format!("{{ node.name = {browser_name} media.class = Audio/Sink audio.position = [ FL FR ] }}"),
            "--playback", &first_name,
        ]).stdout(Stdio::null()).spawn().unwrap());
    let _source = Process(
        Command::new("pw-loopback")
            .args([
                "--capture",
                &second_name,
                "--capture-props",
                "{ stream.capture.sink = true }",
                "--playback-props",
                &format!("{{ node.name = {source_name} media.class = Audio/Source }}"),
            ])
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let started = Instant::now();
    loop {
        let devices = audio::devices().expect("PipeWire must be running");
        if devices.iter().any(|device| device.name == browser_name)
            && devices.iter().any(|device| device.name == source_name)
        {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "test sinks did not appear"
        );
        thread::sleep(Duration::from_millis(50));
    }

    let first_capture = Capture::start(&browser_name, true, "TestFirst").unwrap();
    let second_capture = Capture::start(&source_name, false, "TestSecond").unwrap();
    let mut first_reader = Reader::new(first_capture.signal.clone(), 0);
    let mut second_reader = Reader::new(second_capture.signal.clone(), 0);
    let file = TemporaryAudio(
        std::env::temp_dir().join(format!("slofox_test_{}.f32", std::process::id())),
    );
    let bytes: Vec<u8> = (0..SAMPLE_RATE * 3)
        .flat_map(|sample| {
            (0.2 * (sample as f32 * 440.0 * std::f32::consts::TAU / SAMPLE_RATE as f32).sin())
                .to_ne_bytes()
        })
        .collect();
    fs::write(&file.0, bytes).unwrap();
    let mut playback = Process(
        Command::new("pw-play")
            .args([
                "--raw",
                "--rate",
                "48000",
                "--channels",
                "1",
                "--format",
                "f32",
                "--target",
                &browser_name,
            ])
            .arg("-")
            .stdin(Stdio::from(fs::File::open(&file.0).unwrap()))
            .spawn()
            .unwrap(),
    );

    let started = Instant::now();
    let mut heard_tone = false;
    let mut peak_rms = 0.0_f32;
    let mut second_has_packets = false;
    while started.elapsed() < Duration::from_secs(3) {
        let now = Instant::now();
        let level = first_reader.sample(now);
        peak_rms = peak_rms.max(level);
        heard_tone |= level > 0.05;
        assert!(
            second_reader.sample(now) < 0.001,
            "tone leaked into the second input"
        );
        second_has_packets |= second_reader.signal.status(now) == "live";
        thread::sleep(Duration::from_millis(20));
    }
    assert!(playback.0.wait().unwrap().success());
    assert!(
        heard_tone,
        "first input never received the test tone: RMS {peak_rms}, status {}",
        first_reader.signal.status(Instant::now())
    );
    assert!(second_has_packets, "second input never connected");
    thread::sleep(Duration::from_millis(300));
    let mut second_playback = Process(
        Command::new("pw-play")
            .args([
                "--raw",
                "--rate",
                "48000",
                "--channels",
                "1",
                "--format",
                "f32",
                "--target",
                &second_name,
                "-",
            ])
            .stdin(Stdio::from(fs::File::open(&file.0).unwrap()))
            .spawn()
            .unwrap(),
    );
    let started = Instant::now();
    let mut second_heard_tone = false;
    while started.elapsed() < Duration::from_secs(3) {
        let now = Instant::now();
        second_heard_tone |= second_reader.sample(now) > 0.05;
        assert!(
            first_reader.sample(now) < 0.001,
            "second tone leaked into the first input"
        );
        thread::sleep(Duration::from_millis(20));
    }
    assert!(second_playback.0.wait().unwrap().success());
    assert!(
        second_heard_tone,
        "source input never received the test tone"
    );
}
