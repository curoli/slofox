use bevy::prelude::*;

#[derive(Clone, Copy)]
pub struct HandGesture {
    pub arm_rotation: Quat,
    pub wrist_rotation: Quat,
    pub curl: f32,
}

#[derive(Default)]
pub struct GestureState {
    engagement: f32,
}

impl GestureState {
    pub fn update(&mut self, delta: f32, jaw: f32) -> f32 {
        let target = ((jaw - 0.025) / 0.10).clamp(0.0, 1.0);
        let response = if target > self.engagement { 0.9 } else { 1.5 };
        self.engagement += (target - self.engagement) * (1.0 - (-delta.max(0.0) / response).exp());
        self.engagement
    }
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value.powi(3) * (10.0 + value * (-15.0 + value * 6.0))
}

fn variation(key: i32, host: usize, side: f32, salt: u32) -> f32 {
    let mut seed = (key as u32)
        .wrapping_mul(747796405)
        .wrapping_add(host as u32 * 2891336453)
        .wrapping_add(if side > 0.0 { 981233 } else { 432791 })
        .wrapping_add(salt);
    seed = (seed ^ (seed >> 16)).wrapping_mul(2246822519);
    (seed ^ (seed >> 13)) as f32 / u32::MAX as f32
}

fn key_pose(elapsed: f32, host: usize, side: f32, key: i32, engagement: f32) -> HandGesture {
    let dominant = side == if host == 0 { 1.0 } else { -1.0 };
    let mode = (key + host as i32).rem_euclid(4);
    let active = engagement.clamp(0.0, 1.0)
        * match mode {
            0 => 0.0,
            1 | 2 => {
                if dominant {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.85,
        };
    let phase = elapsed * 1.05 + host as f32 * 1.7;
    let circle = phase + if mode == 3 { 0.0 } else { side * 0.45 };
    let rest_yaw = (variation(key, host, side, 17) - 0.5) * 0.10;
    let rest_pitch = 0.08 + 0.008 * (elapsed * 0.38 + side * 1.8).sin();
    let rest_roll = -side * 1.2 * smoothstep((variation(key, host, side, 43) - 0.4) / 0.3);
    let palm_up = variation(key, host, 1.0, 107) > 0.45;
    let wrist_roll = if palm_up {
        -side * 2.8
    } else if mode == 3 {
        -side * 1.5
    } else {
        -side * 0.35
    };
    let rest_wrist = Quat::from_euler(EulerRot::XYZ, 0.0, rest_yaw, rest_roll);
    let active_wrist = Quat::from_euler(
        EulerRot::XYZ,
        if palm_up { 0.10 } else { 0.65 } + 0.08 * circle.sin(),
        side * (0.08 + 0.08 * circle.cos()),
        wrist_roll + side * 0.10 * circle.sin(),
    );
    HandGesture {
        arm_rotation: Quat::from_euler(
            EulerRot::XYZ,
            rest_pitch - active * (0.35 + 0.055 * circle.sin()),
            rest_yaw + side * active * (0.045 + 0.04 * circle.cos()),
            side * (0.02 + active * (0.055 + 0.025 * circle.cos())),
        ),
        wrist_rotation: rest_wrist.slerp(active_wrist, active),
        curl: 0.15 + 0.18 * (rest_roll.abs() / 1.2) * (1.0 - active) + active * 0.08,
    }
}

pub fn sample(elapsed: f32, host: usize, side: f32, engagement: f32) -> HandGesture {
    let clock = elapsed.max(0.0) + host as f32 * 3.7;
    let key = (clock / 10.0).floor() as i32 + 1;
    let blend = smoothstep(clock.rem_euclid(10.0) / 2.0);
    let previous = key_pose(elapsed, host, side, key - 1, engagement);
    let next = key_pose(elapsed, host, side, key, engagement);
    HandGesture {
        arm_rotation: previous.arm_rotation.slerp(next.arm_rotation, blend),
        wrist_rotation: previous.wrist_rotation.slerp(next.wrist_rotation, blend),
        curl: previous.curl + (next.curl - previous.curl) * blend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_hand_gestures_use_her_left_and_his_right_hand() {
        for host in 0..2 {
            let dominant = if host == 0 { 1.0 } else { -1.0 };
            let elapsed = if host == 0 { 15.0 } else { 1.0 };
            let active = sample(elapsed, host, dominant, 1.0);
            let resting = sample(elapsed, host, -dominant, 1.0);
            let idle = sample(elapsed, host, -dominant, 0.0);
            assert!(resting.arm_rotation.angle_between(idle.arm_rotation) < 0.001);
            assert!(
                active
                    .arm_rotation
                    .angle_between(sample(elapsed, host, dominant, 0.0).arm_rotation)
                    > 0.15
            );
        }
    }

    #[test]
    fn repertoire_includes_rest_single_both_and_varied_palms() {
        for host in 0..2 {
            let mut counts = [0; 3];
            let mut palm_up = false;
            let mut inward = false;
            for key in 0..24 {
                let elapsed = key as f32 * 10.0 + 5.0;
                let mut moving = 0;
                for side in [-1.0, 1.0] {
                    let active = sample(elapsed, host, side, 1.0);
                    let idle = sample(elapsed, host, side, 0.0);
                    if active.arm_rotation.angle_between(idle.arm_rotation) > 0.1 {
                        moving += 1;
                        let palm = active.wrist_rotation * -Vec3::Y;
                        palm_up |= palm.y > 0.3;
                        inward |= palm.x * side < -0.5;
                    }
                }
                counts[moving] += 1;
            }
            assert!(counts.into_iter().all(|count| count > 0));
            assert!(palm_up && inward);
        }
    }

    #[test]
    fn transitions_and_speech_changes_stay_calm_and_frame_rate_independent() {
        for host in 0..2 {
            for side in [-1.0, 1.0] {
                let mut state = GestureState::default();
                let mut previous = sample(0.0, host, side, 0.0);
                for frame in 1..18000 {
                    let elapsed = frame as f32 / 60.0;
                    let jaw = if ((elapsed / 7.0) as usize).is_multiple_of(2) {
                        0.8
                    } else {
                        0.0
                    };
                    let engagement = state.update(1.0 / 60.0, jaw);
                    let pose = sample(elapsed, host, side, engagement);
                    assert!(pose.arm_rotation.is_finite() && pose.wrist_rotation.is_finite());
                    assert!(previous.arm_rotation.angle_between(pose.arm_rotation) < 0.012);
                    assert!(previous.wrist_rotation.angle_between(pose.wrist_rotation) < 0.055);
                    previous = pose;
                }
            }
        }
        let mut slow = GestureState::default();
        let mut fast = GestureState::default();
        for _ in 0..30 {
            slow.update(1.0 / 30.0, 0.8);
        }
        for _ in 0..120 {
            fast.update(1.0 / 120.0, 0.8);
        }
        assert!((slow.engagement - fast.engagement).abs() < 0.00001);
        let held = slow.update(0.1, 0.0);
        assert!(held > 0.5);
        for _ in 0..300 {
            slow.update(1.0 / 60.0, 0.0);
        }
        assert!(slow.engagement < 0.05);
    }

    #[test]
    fn resting_hands_are_asymmetric_and_reposition_slowly() {
        for host in 0..2 {
            let first = sample(5.0, host, 1.0, 0.0);
            let mut repositioned = false;
            let mut asymmetric = false;
            let mut thumb_up = false;
            let mut flat = false;
            for key in 0..24 {
                let elapsed = key as f32 * 10.0 + 5.0;
                let later = sample(elapsed, host, 1.0, 0.0);
                let other = sample(elapsed, host, -1.0, 0.0);
                repositioned |= first.wrist_rotation.angle_between(later.wrist_rotation) > 0.03;
                asymmetric |= later.wrist_rotation.angle_between(other.wrist_rotation) > 0.03;
                let thumb = later.wrist_rotation * -Vec3::X;
                thumb_up |= thumb.y > 0.8;
                flat |= thumb.y.abs() < 0.1;
            }
            assert!(repositioned && asymmetric && thumb_up && flat);
        }
    }
}
