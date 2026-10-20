#[derive(Default)]
pub struct HeadAttention {
    yaw: f32,
    engagement: f32,
    listening_time: f32,
    quiet_time: f32,
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn activity(jaw: f32) -> f32 {
    smoothstep((jaw - 0.035) / 0.18)
}

fn glance(time: f32, period: f32, start: f32, end: f32, transition: f32) -> f32 {
    let phase = time.rem_euclid(period);
    smoothstep((phase - start) / transition) * (1.0 - smoothstep((phase - end) / transition))
}

impl HeadAttention {
    pub fn update(
        &mut self,
        delta: f32,
        elapsed: f32,
        host: usize,
        own_jaw: f32,
        partner_jaw: f32,
    ) -> f32 {
        let delta = delta.max(0.0);
        let own = activity(own_jaw);
        let partner = activity(partner_jaw);
        if partner > 0.25 && own < 0.5 {
            self.quiet_time = 0.0;
            self.listening_time = (self.listening_time + delta).rem_euclid(7.8 + host as f32 * 0.7);
        } else {
            self.quiet_time = (self.quiet_time + delta).min(1.0);
            if self.quiet_time > 0.8 {
                self.listening_time = 0.0;
            }
        }
        let target_engagement = partner * (1.0 - own);
        let response = if target_engagement > self.engagement {
            0.25
        } else {
            0.65
        };
        self.engagement +=
            (target_engagement - self.engagement) * (1.0 - (-delta / response).exp());
        let listening = glance(self.listening_time, 7.8 + host as f32 * 0.7, 0.2, 4.8, 0.65);
        let occasional = glance(
            elapsed + host as f32 * 4.3,
            11.5 + host as f32 * 1.3,
            0.4,
            2.7,
            0.7,
        );
        let angle = 0.025
            + 0.80 * self.engagement * listening
            + 0.32 * (1.0 - self.engagement) * occasional * (1.0 - own * 0.55);
        let target_yaw = if host == 0 { angle } else { -angle };
        self.yaw += (target_yaw - self.yaw) * (1.0 - (-delta / 0.40).exp());
        self.yaw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listeners_turn_inward_more_than_speakers_or_silent_hosts() {
        for host in 0..2 {
            let mut listener = HeadAttention::default();
            let mut speaker = HeadAttention::default();
            let mut silent = HeadAttention::default();
            let mut listener_peak = 0.0_f32;
            let mut speaker_peak = 0.0_f32;
            let mut silent_peak = 0.0_f32;
            for frame in 0..900 {
                let elapsed = frame as f32 / 60.0;
                let yaw = listener.update(1.0 / 60.0, elapsed, host, 0.0, 0.8);
                assert!(if host == 0 { yaw >= 0.0 } else { yaw <= 0.0 });
                listener_peak = listener_peak.max(yaw.abs());
                speaker_peak =
                    speaker_peak.max(speaker.update(1.0 / 60.0, elapsed, host, 0.8, 0.0).abs());
                silent_peak =
                    silent_peak.max(silent.update(1.0 / 60.0, elapsed, host, 0.0, 0.0).abs());
            }
            assert!(listener_peak > 0.75);
            assert!(speaker_peak < 0.20);
            assert!((0.25..0.36).contains(&silent_peak));
        }
    }

    #[test]
    fn listening_includes_forward_breaks_and_independent_host_timing() {
        let mut heads = [HeadAttention::default(), HeadAttention::default()];
        let mut lowest = [f32::INFINITY; 2];
        let mut highest = [0.0_f32; 2];
        let mut different = false;
        for frame in 0..1200 {
            let elapsed = frame as f32 / 60.0;
            let mut angles = [0.0_f32; 2];
            for host in 0..2 {
                angles[host] = heads[host]
                    .update(1.0 / 60.0, elapsed, host, 0.0, 0.8)
                    .abs();
                if elapsed > 5.0 {
                    lowest[host] = lowest[host].min(angles[host]);
                    highest[host] = highest[host].max(angles[host]);
                }
            }
            different |= (angles[0] - angles[1]).abs() > 0.30;
        }
        assert!(different);
        for host in 0..2 {
            assert!(lowest[host] < 0.10);
            assert!(highest[host] > 0.75);
        }
    }

    #[test]
    fn short_word_gaps_preserve_attention_but_speaking_releases_it_smoothly() {
        let mut head = HeadAttention::default();
        let mut previous = 0.0;
        for frame in 0..360 {
            let elapsed = frame as f32 / 60.0;
            let partner = if frame % 60 < 12 { 0.0 } else { 0.8 };
            let own = if elapsed > 4.0 { 0.8 } else { 0.0 };
            let yaw = head.update(1.0 / 60.0, elapsed, 0, own, partner);
            assert!((yaw - previous).abs() < 0.04);
            if elapsed > 2.0 && elapsed < 4.0 {
                assert!(yaw > 0.55);
            }
            previous = yaw;
        }
        assert!(previous < 0.12);
    }

    #[test]
    fn turns_are_bounded_and_approximately_frame_rate_independent() {
        let sample = |fps: usize, host: usize| {
            let mut head = HeadAttention::default();
            let mut samples = Vec::new();
            for frame in 0..fps * 20 {
                let elapsed = frame as f32 / fps as f32;
                let own = if (9.0..14.0).contains(&elapsed) {
                    0.8
                } else {
                    0.0
                };
                let partner = if !(9.0..16.0).contains(&elapsed) {
                    0.8
                } else {
                    0.0
                };
                let yaw = head.update(1.0 / fps as f32, elapsed, host, own, partner);
                assert!(yaw.is_finite() && yaw.abs() <= 0.85);
                if (frame + 1) % fps == 0 {
                    samples.push(yaw);
                }
            }
            samples
        };
        for host in 0..2 {
            for (slow, fast) in sample(30, host).iter().zip(sample(120, host)) {
                assert!((slow - fast).abs() < 0.045, "{slow} versus {fast}");
            }
        }
    }
}
