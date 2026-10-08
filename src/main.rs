mod studio;

use std::{
    process::ExitCode,
    time::{Duration, Instant},
};

use bevy::{
    app::AppExit,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::WindowResolution,
    winit::WinitSettings,
};
use clap::Parser;
use slofox::{
    audio::{self, Capture, Envelope, Features, Reader, SpeechPose},
    config::{MouthMode, Options},
    formants::Formants,
    routing::{self, TabRouter},
};
use studio::{AnimatedPart, Host, PartKind, StudioCamera};

#[derive(Resource)]
struct Session {
    options: Options,
    readers: Vec<Reader>,
    envelopes: [Envelope; 2],
    poses: [SpeechPose; 2],
    levels: [f32; 2],
    statuses: [String; 2],
    started: Instant,
    overlay: bool,
    camera: usize,
    auto_camera: bool,
    screenshot_taken: bool,
}

#[derive(Component)]
struct DiagnosticsOverlay;

fn main() -> ExitCode {
    let options = Options::parse();
    let mut captures = Vec::new();
    let mut readers = Vec::new();
    let mut router = None;
    if options.list_devices || !options.demo {
        if !cfg!(target_os = "linux") {
            eprintln!("Live audio currently uses Linux PipeWire. Use --demo on other platforms.");
            return ExitCode::FAILURE;
        }
        let devices = match audio::devices() {
            Ok(devices) => devices,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        };
        if options.list_devices {
            for device in devices {
                println!(
                    "{} | {} | {} | {}",
                    if device.is_sink { "output" } else { "input " },
                    device.serial,
                    device.name,
                    device.description
                );
            }
            match routing::graph().and_then(|graph| routing::streams(&graph)) {
                Ok(streams) => {
                    for stream in streams {
                        println!("stream | {} | {}", stream.application, stream.title);
                    }
                }
                Err(error) => eprintln!("{error}"),
            }
            return ExitCode::SUCCESS;
        }
        for (target, sink, label, delay) in [
            (&options.browser, true, "Browser", options.browser_delay_ms),
            (
                &options.microphone,
                false,
                "Microphone",
                options.microphone_delay_ms,
            ),
        ] {
            if let Err(error) = audio::validate_target(&devices, target, sink) {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
            let capture = match Capture::start_with_diagnostics(
                target,
                sink,
                label,
                options.audio_diagnostics,
            ) {
                Ok(capture) => capture,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };
            readers.push(Reader::new(capture.signal.clone(), delay));
            captures.push(capture);
        }
        if let Some(title) = &options.route_browser_tab {
            router = match TabRouter::start(
                options.browser_application.clone(),
                title.clone(),
                options.browser.clone(),
            ) {
                Ok(router) => Some(router),
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };
        }
    }
    let session = Session {
        overlay: !options.clean,
        auto_camera: options.auto_camera,
        options,
        readers,
        envelopes: Default::default(),
        poses: Default::default(),
        levels: [0.0; 2],
        statuses: Default::default(),
        started: Instant::now(),
        camera: 0,
        screenshot_taken: false,
    };
    let exit = App::new()
        .insert_resource(ClearColor(Color::srgb_u8(21, 31, 49)))
        .insert_resource(session)
        .insert_resource(WinitSettings::continuous())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Slofox — Talk Show Studio".into(),
                resolution: WindowResolution::new(1280, 720).with_scale_factor_override(1.0),
                present_mode: bevy::window::PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(First, pace_frames)
        .add_systems(Startup, (studio::setup, setup_overlay))
        .add_systems(
            Update,
            (
                controls,
                update_audio,
                animate_hosts,
                animate_parts,
                move_camera,
                overlay,
                finish,
            )
                .chain(),
        )
        .run();
    drop(router);
    drop(captures);
    match exit {
        AppExit::Success => ExitCode::SUCCESS,
        AppExit::Error(code) => ExitCode::from(code.get()),
    }
}

fn setup_overlay(mut commands: Commands, mut session: ResMut<Session>) {
    session.started = Instant::now();
    commands.spawn((
        DiagnosticsOverlay,
        Text::new(""),
        TextFont::from_font_size(15.0),
        TextColor(Color::srgb_u8(221, 231, 234)),
        BackgroundColor(Color::srgba(0.03, 0.06, 0.09, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            bottom: px(16),
            padding: UiRect::all(px(12)),
            ..default()
        },
    ));
}

fn pace_frames(session: Res<Session>, mut previous: Local<Option<Instant>>) {
    let interval = Duration::from_secs_f64(1.0 / session.options.fps as f64);
    if let Some(previous) = *previous
        && let Some(remaining) = interval.checked_sub(previous.elapsed())
    {
        std::thread::sleep(remaining);
    }
    *previous = Some(Instant::now());
}

fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::KeyH) {
        session.overlay = !session.overlay;
    }
    if keys.just_pressed(KeyCode::KeyA) {
        session.auto_camera = !session.auto_camera;
    }
    for (index, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .into_iter()
        .enumerate()
    {
        if keys.just_pressed(key) {
            session.camera = index;
            session.auto_camera = false;
        }
    }
}

fn update_audio(time: Res<Time>, mut session: ResMut<Session>) {
    let now = Instant::now();
    let elapsed = now.duration_since(session.started).as_secs_f32();
    for index in 0..2 {
        let mut features = if session.options.demo {
            session.statuses[index] = "demo (no audio capture)".into();
            let (first, second) = match (elapsed * 1.5) as usize % 3 {
                0 => (800.0, 1200.0),
                1 => (300.0, 2400.0),
                _ => (350.0, 850.0),
            };
            Features {
                rms: audio::demo_level(elapsed, index),
                formants: Some(Formants { first, second }),
            }
        } else {
            session.statuses[index] = session.readers[index].signal.status(now);
            session.readers[index].features(now)
        };
        let gain = if index == 0 {
            session.options.browser_gain
        } else {
            session.options.microphone_gain
        };
        let scale = if index == 0 {
            session.options.browser_formant_scale
        } else {
            session.options.microphone_formant_scale
        };
        if session.options.mouth_mode == MouthMode::Volume {
            features.formants = None;
        } else if features.rms > session.options.threshold {
            let description = features.formants.map_or_else(
                || "volume fallback".to_owned(),
                |formants| {
                    format!(
                        "{} F1 {:.0} F2 {:.0} Hz",
                        formants.label(scale),
                        formants.first,
                        formants.second
                    )
                },
            );
            session.statuses[index].push_str(&format!(" / {description}"));
        }
        let threshold = session.options.threshold;
        session.levels[index] = features.rms;
        session.poses[index] = session.envelopes[index].update_features(
            features,
            gain,
            threshold,
            time.delta_secs(),
            scale,
        );
    }
}

fn animate_hosts(
    time: Res<Time>,
    session: Res<Session>,
    mut hosts: Query<(&Host, &mut Transform)>,
) {
    let elapsed = time.elapsed_secs();
    for (host, mut transform) in &mut hosts {
        let phase = elapsed + host.index as f32 * 2.1;
        let speech = session.poses[host.index].jaw_open;
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            0.015 * (phase * 0.7).sin() - speech * 0.018,
            0.025 * (phase * 0.37).sin(),
            0.012 * (phase * 0.53).sin(),
        );
        transform.translation.y = 0.76 + 0.006 * (phase * 1.7).sin();
    }
}

fn animate_parts(
    time: Res<Time>,
    session: Res<Session>,
    mut parts: Query<(&AnimatedPart, &mut Transform), Without<Host>>,
) {
    let elapsed = time.elapsed_secs();
    for (part, mut transform) in &mut parts {
        let pose = session.poses[part.host];
        let phase = elapsed + part.host as f32 * 1.83;
        *transform = part.rest;
        match part.kind {
            PartKind::Head => {
                let toward_partner = if part.host == 0 { 1.0 } else { -1.0 };
                transform.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    0.025 * (phase * 1.2).sin() + pose.jaw_open * 0.025 * (phase * 3.0).sin(),
                    toward_partner
                        * (0.07 + 0.08 * (phase * 0.28).sin())
                        * (1.0 - pose.jaw_open * 0.5),
                    0.025 * (phase * 0.64).sin(),
                );
            }
            PartKind::Arm(side) => {
                let gesture = pose.jaw_open * (0.5 + 0.5 * (phase * 1.8 + side).sin());
                transform.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    -0.04 - gesture * 0.22,
                    side * gesture * 0.10,
                    side * (0.02 + gesture * 0.10),
                );
            }
            PartKind::Eye => {
                let blink_phase = phase.rem_euclid(4.7);
                let blink = if blink_phase < 0.16 {
                    (blink_phase / 0.16 * std::f32::consts::PI).sin()
                } else {
                    0.0
                };
                transform.scale.y = 1.0 - blink * 0.94;
            }
            PartKind::Mouth => {
                transform.scale.y += pose.jaw_open * 0.025;
                transform.scale.x *= 1.0 - pose.lip_round * 0.45 + pose.lip_wide * 0.25;
                transform.translation.y -= pose.jaw_open * 0.014;
                transform.translation.z += pose.lip_round * 0.008;
            }
            PartKind::UpperLip => {
                transform.scale.x *= 1.0 - pose.lip_round * 0.45 + pose.lip_wide * 0.25;
                transform.scale.y *= 1.0 + pose.lip_round * 0.4;
                transform.translation.z += pose.lip_round * 0.008;
            }
            PartKind::LowerLip => {
                transform.translation.y -= pose.jaw_open * 0.041;
                transform.scale.x *= (1.0 - pose.jaw_open * 0.1)
                    * (1.0 - pose.lip_round * 0.45 + pose.lip_wide * 0.25);
                transform.scale.y *= 1.0 + pose.lip_round * 0.4;
                transform.translation.z += pose.lip_round * 0.008;
            }
            PartKind::Teeth => {
                transform.scale.y *= pose.jaw_open.clamp(0.01, 0.6);
                transform.scale.x *= 1.0 - pose.lip_round * 0.45 + pose.lip_wide * 0.25;
            }
        }
    }
}

fn move_camera(
    time: Res<Time>,
    mut session: ResMut<Session>,
    mut cameras: Query<&mut Transform, With<StudioCamera>>,
) {
    if session.auto_camera {
        session.camera = (time.elapsed_secs() / 14.0) as usize % 3;
    }
    let target = Vec3::new(0.0, 1.44, 0.0);
    let position = match session.camera {
        1 => Vec3::new(-2.3, 2.05, 3.7),
        2 => Vec3::new(2.3, 2.05, 3.7),
        _ => Vec3::new(0.0, 2.12, 4.25),
    };
    for mut transform in &mut cameras {
        let amount = 1.0 - (-time.delta_secs() * 1.3).exp();
        transform.translation = transform.translation.lerp(position, amount);
        transform.look_at(target, Vec3::Y);
    }
}

fn overlay(
    session: Res<Session>,
    diagnostics: Res<DiagnosticsStore>,
    mut overlay: Query<(&mut Text, &mut Node), With<DiagnosticsOverlay>>,
) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or(0.0);
    for (mut text, mut node) in &mut overlay {
        node.display = if session.overlay {
            Display::Flex
        } else {
            Display::None
        };
        text.0 = format!(
            "SLOFOX  /  TALK SHOW STUDIO                         {fps:.0} fps\n\
             1  ChatGPT    RMS {:.3}    mouth {:3.0}%    {}\n\
             2  Microphone RMS {:.3}    mouth {:3.0}%    {}\n\
             H  hide panel    1/2/3  camera    A  auto camera    Esc  quit",
            session.levels[0],
            session.poses[0].jaw_open * 100.0,
            session.statuses[0],
            session.levels[1],
            session.poses[1].jaw_open * 100.0,
            session.statuses[1],
        );
    }
}

fn finish(mut commands: Commands, mut session: ResMut<Session>, mut exit: MessageWriter<AppExit>) {
    let elapsed = session.started.elapsed().as_secs_f32();
    if elapsed > 5.0 && !session.screenshot_taken {
        if let Some(path) = &session.options.screenshot {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.clone()));
        }
        session.screenshot_taken = true;
    }
    if session
        .options
        .seconds
        .is_some_and(|seconds| elapsed >= seconds)
    {
        exit.write(AppExit::Success);
    }
}
