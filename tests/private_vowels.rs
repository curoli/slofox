use std::{env, fs};

use slofox::{
    audio::{Envelope, Features, rms},
    formants::Analyzer,
};

#[test]
#[ignore = "requires SLOFOX_VOWELS_F32 pointing to the private A/I/O/U/loud-O reference converted to 48 kHz mono f32le"]
fn recorded_vowels_drive_the_expected_mouth_at_normal_and_reduced_level() {
    let bytes =
        fs::read(env::var("SLOFOX_VOWELS_F32").expect("set the private recording path")).unwrap();
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect();
    assert!(samples.len() >= 24 * 48_000);
    for amplitude in [1.0, 0.5] {
        let mut analyzer = Analyzer::default();
        let mut envelope = Envelope::default();
        let intervals = [
            (0.5, 3.5, "A-like"),
            (5.5, 8.5, "E/I-like"),
            (10.5, 13.5, "O/U-like"),
            (16.0, 18.5, "O/U-like"),
            (20.5, 23.0, "O/U-like"),
        ];
        let mut totals = [0; 5];
        let mut correct = [0; 5];
        let mut animated = [0; 5];
        for (index, original) in samples.chunks(480).enumerate() {
            let block: Vec<f32> = original.iter().map(|sample| sample * amplitude).collect();
            analyzer.analyze(&block);
            let features = Features::analyzed(rms(&block), &analyzer);
            let pose = envelope.update_features(features, 8.0, 0.008, 0.01, 1.0);
            let seconds = index as f32 / 100.0;
            for (position, &(start, end, label)) in intervals.iter().enumerate() {
                if seconds >= start && seconds < end {
                    totals[position] += 1;
                    correct[position] += usize::from(
                        features
                            .selected_shape(1.0)
                            .is_some_and(|(shape, _)| shape.label() == label),
                    );
                    let appropriate = match label {
                        "A-like" => pose.jaw_open > 0.15 && pose.lip_round < pose.jaw_open * 0.4,
                        "E/I-like" => pose.lip_wide > 0.05 && pose.lip_wide > pose.lip_round,
                        _ => pose.lip_round > 0.05 && pose.lip_round > pose.lip_wide,
                    };
                    animated[position] += usize::from(appropriate);
                }
            }
        }
        for (position, interval) in intervals.iter().enumerate() {
            println!(
                "{interval:?}, amplitude {amplitude}: {} / {} classifications, {} animated",
                correct[position], totals[position], animated[position]
            );
            assert!(correct[position] * 100 >= totals[position] * 90);
            assert!(animated[position] * 100 >= totals[position] * 90);
        }
    }
}
