use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MouthMode {
    Formants,
    Volume,
}

#[derive(Parser, Debug, Clone)]
#[command(version, about = "Two audio-driven 3D talk show hosts for OBS")]
pub struct Options {
    #[arg(long, default_value = crate::signs::DEFAULT_SUBTITLE, help = "Subtitle on the studio wall sign")]
    pub subtitle: String,
    #[arg(long, value_parser = calendar_date, help = "Calendar date as YYYY-MM-DD (default: local date at startup)")]
    pub calendar_date: Option<chrono::NaiveDate>,
    #[arg(long, help = "Animate synthetic conversation without recording audio")]
    pub demo: bool,
    #[arg(
        long,
        help = "List PipeWire devices and application/media names for tab routing"
    )]
    pub list_devices: bool,
    #[arg(
        long,
        default_value = "slofox_browser",
        help = "PipeWire output sink name or object.serial for host 1"
    )]
    pub browser: String,
    #[arg(
        long,
        help = "Keep the browser tab with this exact PipeWire media.name routed to --browser"
    )]
    pub route_browser_tab: Option<String>,
    #[arg(
        long,
        default_value = "Firefox",
        help = "Exact PipeWire application.name used with --route-browser-tab"
    )]
    pub browser_application: String,
    #[arg(
        long,
        default_value = "auto",
        help = "PipeWire microphone source name or object.serial for host 2"
    )]
    pub microphone: String,
    #[arg(long, default_value = "8", value_parser = positive_float)]
    pub browser_gain: f32,
    #[arg(long, default_value = "8", value_parser = positive_float)]
    pub microphone_gain: f32,
    #[arg(
        long,
        value_enum,
        default_value = "formants",
        help = "Local vowel-like mouth shapes or the original volume-only animation"
    )]
    pub mouth_mode: MouthMode,
    #[arg(long, default_value = "1", value_parser = formant_scale, help = "Browser voice formant normalization (0.7–1.5; larger for higher resonances)")]
    pub browser_formant_scale: f32,
    #[arg(long, default_value = "1", value_parser = formant_scale, help = "Microphone voice formant normalization (0.7–1.5)")]
    pub microphone_formant_scale: f32,
    #[arg(long, default_value = "0.008", value_parser = threshold)]
    pub threshold: f32,
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u64).range(0..=2000))]
    pub browser_delay_ms: u64,
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u64).range(0..=2000))]
    pub microphone_delay_ms: u64,
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(15..=120))]
    pub fps: u64,
    #[arg(long, help = "Cycle slowly through three studio camera positions")]
    pub auto_camera: bool,
    #[arg(
        long,
        help = "Frame both faces closely for comparison with portrait references"
    )]
    pub portrait_preview: bool,
    #[arg(long, help = "Start with the diagnostic overlay hidden")]
    pub clean: bool,
    #[arg(
        long,
        help = "Print per-input resonance candidates once per second; no audio is saved"
    )]
    pub audio_diagnostics: bool,
    #[arg(long, value_parser = positive_float, help = "Exit after this many seconds (useful for a demo smoke test)")]
    pub seconds: Option<f32>,
    #[arg(long, help = "Save a PNG of the app window after five seconds")]
    pub screenshot: Option<String>,
}

fn calendar_date(value: &str) -> Result<chrono::NaiveDate, String> {
    let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| "expected a valid date as YYYY-MM-DD")?;
    if date.format("%Y-%m-%d").to_string() != value {
        return Err("expected a valid date as YYYY-MM-DD".into());
    }
    Ok(date)
}

fn positive_float(value: &str) -> Result<f32, String> {
    let number: f32 = value.parse().map_err(|_| "expected a number")?;
    if number.is_finite() && number > 0.0 {
        Ok(number)
    } else {
        Err("expected a finite positive number".into())
    }
}

fn threshold(value: &str) -> Result<f32, String> {
    let number: f32 = value.parse().map_err(|_| "expected a number")?;
    if number.is_finite() && (0.0..1.0).contains(&number) {
        Ok(number)
    } else {
        Err("threshold must be between 0 (inclusive) and 1 (exclusive)".into())
    }
}

fn formant_scale(value: &str) -> Result<f32, String> {
    let number = positive_float(value)?;
    if (0.7..=1.5).contains(&number) {
        Ok(number)
    } else {
        Err("formant scale must be between 0.7 and 1.5".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_subtitle_and_calendar_date_have_defaults_and_overrides() {
        let defaults = Options::try_parse_from(["slofox"]).unwrap();
        assert_eq!(defaults.subtitle, crate::signs::DEFAULT_SUBTITLE);
        assert_eq!(defaults.calendar_date, None);
        let custom = Options::try_parse_from([
            "slofox",
            "--subtitle",
            "Sonderausgabe: Iris hat Geburtstag",
            "--calendar-date",
            "2026-10-17",
        ])
        .unwrap();
        assert_eq!(custom.subtitle, "Sonderausgabe: Iris hat Geburtstag");
        assert_eq!(custom.calendar_date.unwrap().to_string(), "2026-10-17");
        assert!(Options::try_parse_from(["slofox", "--calendar-date", "2028-02-29"]).is_ok());
        for invalid in [
            "2026-02-29",
            "2026-13-01",
            "2026-10-32",
            "2026-1-2",
            "17.10.2026",
        ] {
            assert!(Options::try_parse_from(["slofox", "--calendar-date", invalid]).is_err());
        }
    }

    #[test]
    fn rejects_invalid_audio_and_timing_options() {
        for arguments in [
            vec!["slofox", "--threshold", "NaN"],
            vec!["slofox", "--threshold", "1"],
            vec!["slofox", "--browser-gain", "inf"],
            vec!["slofox", "--microphone-gain", "0"],
            vec!["slofox", "--browser-delay-ms", "2001"],
            vec!["slofox", "--fps", "0"],
            vec!["slofox", "--mouth-mode", "phonemes"],
            vec!["slofox", "--browser-formant-scale", "NaN"],
            vec!["slofox", "--microphone-formant-scale", "0.6"],
            vec!["slofox", "--browser-formant-scale", "1.6"],
        ] {
            assert!(Options::try_parse_from(arguments).is_err());
        }
    }

    #[test]
    fn portrait_preview_is_opt_in() {
        assert!(
            !Options::try_parse_from(["slofox", "--demo"])
                .unwrap()
                .portrait_preview
        );
        assert!(
            Options::try_parse_from(["slofox", "--demo", "--portrait-preview"])
                .unwrap()
                .portrait_preview
        );
    }

    #[test]
    fn formants_are_default_and_volume_remains_available() {
        let defaults = Options::try_parse_from(["slofox"]).unwrap();
        assert_eq!(defaults.mouth_mode, MouthMode::Formants);
        assert_eq!(defaults.browser_formant_scale, 1.0);
        assert!(!defaults.audio_diagnostics);
        assert!(
            Options::try_parse_from(["slofox", "--audio-diagnostics"])
                .unwrap()
                .audio_diagnostics
        );
        let volume = Options::try_parse_from([
            "slofox",
            "--mouth-mode",
            "volume",
            "--microphone-formant-scale",
            "1.2",
        ])
        .unwrap();
        assert_eq!(volume.mouth_mode, MouthMode::Volume);
        assert_eq!(volume.microphone_formant_scale, 1.2);
    }
}
