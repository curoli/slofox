use slofox::{audio::rms, formants::Analyzer};
use std::{env, fs};

fn main() {
    let bytes = fs::read(env::args().nth(1).expect("path to 48 kHz mono f32le audio")).unwrap();
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect();
    let mut analyzer = Analyzer::default();
    for (index, block) in samples.chunks(480).enumerate() {
        let formants = analyzer.analyze(block);
        if index % 10 == 0 {
            println!(
                "{:.2}\t{:.4}\t{}\t{}",
                index as f32 / 100.0,
                rms(block),
                analyzer.spectral().map_or_else(
                    || formants.map_or("fallback", |formants| formants.label(1.0)),
                    |spectral| spectral.label()
                ),
                analyzer.diagnostics()
            );
        }
    }
}
