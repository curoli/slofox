use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(version, about = "Two audio-driven 3D talk show hosts for OBS")]
pub struct Options {
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
    #[arg(long, help = "Start with the diagnostic overlay hidden")]
    pub clean: bool,
    #[arg(long, value_parser = positive_float, help = "Exit after this many seconds (useful for a demo smoke test)")]
    pub seconds: Option<f32>,
    #[arg(long, help = "Save a PNG of the app window after five seconds")]
    pub screenshot: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_audio_and_timing_options() {
        for arguments in [
            vec!["slofox", "--threshold", "NaN"],
            vec!["slofox", "--threshold", "1"],
            vec!["slofox", "--browser-gain", "inf"],
            vec!["slofox", "--microphone-gain", "0"],
            vec!["slofox", "--browser-delay-ms", "2001"],
            vec!["slofox", "--fps", "0"],
        ] {
            assert!(Options::try_parse_from(arguments).is_err());
        }
    }
}
