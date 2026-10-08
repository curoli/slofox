use std::f32::consts::{FRAC_PI_2, PI};

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::view::NoIndirectDrawing,
};

#[derive(Component)]
pub struct Host {
    pub index: usize,
}

#[derive(Component)]
pub struct AnimatedPart {
    pub host: usize,
    pub kind: PartKind,
    pub rest: Transform,
}

pub enum PartKind {
    Head,
    Arm(f32),
    Eye,
    Mouth,
    UpperLip,
    LowerLip,
    Teeth,
}

#[derive(Component)]
pub struct StudioCamera;

const DESK_THICKNESS: f32 = 0.10;
pub const DESK_SURFACE_Y: f32 = 1.17;
const ARM_CLEARANCE: f32 = 0.005;

#[derive(Component)]
pub struct ArmBounds {
    parts: [Transform; 8],
}

impl ArmBounds {
    fn lowest_y(&self, host: &Transform, arm: &Transform) -> f32 {
        let parent = host.to_matrix() * arm.to_matrix();
        self.parts
            .iter()
            .map(|part| {
                let matrix = parent * part.to_matrix();
                let extent = Vec3::new(matrix.x_axis.y, matrix.y_axis.y, matrix.z_axis.y).length();
                matrix.w_axis.y - extent
            })
            .fold(f32::INFINITY, f32::min)
    }

    pub fn keep_above_desk(&self, host: &Transform, arm: &mut Transform) {
        let correction = (DESK_SURFACE_Y + ARM_CLEARANCE - self.lowest_y(host, arm)).max(0.0);
        arm.translation += host
            .to_matrix()
            .inverse()
            .transform_vector3(Vec3::Y * correction);
    }
}

pub fn host_motion(elapsed: f32, index: usize, speech: f32) -> (Quat, f32) {
    let phase = elapsed + index as f32 * 2.1;
    (
        Quat::from_euler(
            EulerRot::XYZ,
            0.015 * (phase * 0.7).sin() - speech * 0.018,
            0.025 * (phase * 0.37).sin(),
            0.012 * (phase * 0.53).sin(),
        ),
        0.76 + 0.006 * (phase * 1.7).sin(),
    )
}

pub fn arm_rotation(phase: f32, side: f32, speech: f32) -> Quat {
    let gesture = speech * (0.5 + 0.5 * (phase * 1.8 + side).sin());
    Quat::from_euler(
        EulerRot::XYZ,
        -0.04 - gesture * 0.22,
        side * gesture * 0.10,
        side * (0.02 + gesture * 0.10),
    )
}

fn arm_parts(side: f32) -> [Transform; 8] {
    let elbow = Vec3::new(side * 0.24, -0.095, 0.16);
    let wrist = Vec3::new(-side * 0.01, -0.055, 0.46);
    std::array::from_fn(|index| match index {
        0 => limb_transform(Vec3::ZERO, elbow, 0.066),
        1 => Transform::from_translation(elbow).with_scale(Vec3::splat(0.048)),
        2 => limb_transform(elbow, wrist, 0.047),
        3 => Transform::from_translation(wrist + Vec3::new(0.0, 0.0, 0.06))
            .with_scale(Vec3::new(0.052, 0.031, 0.088)),
        _ => Transform::from_translation(
            wrist + Vec3::new(((index - 4) as f32 - 1.5) * 0.021, -0.002, 0.126),
        )
        .with_scale(Vec3::new(0.011, 0.018, 0.033)),
    })
}

struct Palette {
    skin: Handle<StandardMaterial>,
    hair: Handle<StandardMaterial>,
    streak: Handle<StandardMaterial>,
    shirt: Handle<StandardMaterial>,
    accent: Handle<StandardMaterial>,
    lips: Handle<StandardMaterial>,
    white: Handle<StandardMaterial>,
    iris: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
}

struct Geometry {
    sphere: Handle<Mesh>,
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
}

fn matte(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.85,
        ..default()
    })
}

fn object(
    commands: &mut Commands,
    parent: Option<Entity>,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    transform: Transform,
) -> Entity {
    let entity = commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            transform,
        ))
        .id();
    if let Some(parent) = parent {
        commands.entity(parent).add_child(entity);
    }
    entity
}

fn ellipsoid(
    commands: &mut Commands,
    geometry: &Geometry,
    parent: Entity,
    material: &Handle<StandardMaterial>,
    position: Vec3,
    scale: Vec3,
) -> Entity {
    object(
        commands,
        Some(parent),
        &geometry.sphere,
        material,
        Transform::from_translation(position).with_scale(scale),
    )
}

fn limb_transform(start: Vec3, end: Vec3, radius: f32) -> Transform {
    let direction = end - start;
    Transform::from_translation((start + end) * 0.5)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, direction.normalize()))
        .with_scale(Vec3::new(
            radius,
            direction.length() * 0.5 + radius * 0.25,
            radius,
        ))
}

fn animated(commands: &mut Commands, entity: Entity, host: usize, kind: PartKind, rest: Transform) {
    commands
        .entity(entity)
        .insert(AnimatedPart { host, kind, rest });
}

fn hair_cap() -> Mesh {
    let rings = 12;
    let segments = 32;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * 2.0 * PI;
            let front = angle.cos().max(0.0);
            let extent = 1.85 - front.powi(4) * 0.75;
            let polar = (ring as f32 / rings as f32 * extent).max(0.001);
            let normal = Vec3::new(
                polar.sin() * angle.sin(),
                polar.cos(),
                polar.sin() * angle.cos(),
            );
            positions.push(normal.to_array());
            normals.push(normal.to_array());
            if ring < rings && segment < segments {
                let current = ring * (segments + 1) + segment;
                let next = current + segments + 1;
                indices.extend_from_slice(&[
                    current,
                    next,
                    current + 1,
                    current + 1,
                    next,
                    next + 1,
                ]);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

fn host(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    geometry: &Geometry,
    palette: &Palette,
    index: usize,
) {
    let root = commands
        .spawn((
            Host { index },
            Transform::from_xyz(if index == 0 { -0.76 } else { 0.76 }, 0.76, 0.0),
            Visibility::default(),
        ))
        .id();
    ellipsoid(
        commands,
        geometry,
        root,
        &palette.shirt,
        Vec3::new(0.0, 0.24, 0.0),
        Vec3::new(0.30, 0.38, 0.17),
    );
    ellipsoid(
        commands,
        geometry,
        root,
        if index == 0 {
            &palette.skin
        } else {
            &palette.shirt
        },
        Vec3::new(0.0, 0.56, 0.0),
        Vec3::new(0.31, 0.13, 0.14),
    );
    if index == 1 {
        ellipsoid(
            commands,
            geometry,
            root,
            &palette.shirt,
            Vec3::new(0.0, 0.49, 0.02),
            Vec3::new(0.30, 0.16, 0.16),
        );
        for row in 0..6 {
            for column in 0..7 {
                let horizontal = (column as f32 - 3.0) * 0.068;
                let vertical = 0.1 + row as f32 * 0.075;
                let front = 0.165 * (1.0 - (horizontal / 0.32).powi(2)).sqrt();
                ellipsoid(
                    commands,
                    geometry,
                    root,
                    &palette.accent,
                    Vec3::new(horizontal, vertical, front),
                    Vec3::new(0.018, 0.023, 0.005),
                );
            }
        }
        for button in 0..4 {
            ellipsoid(
                commands,
                geometry,
                root,
                &palette.white,
                Vec3::new(0.0, 0.15 + button as f32 * 0.1, 0.175),
                Vec3::splat(0.008),
            );
        }
    } else {
        for column in 0..9 {
            let horizontal = (column as f32 - 4.0) * 0.06;
            ellipsoid(
                commands,
                geometry,
                root,
                &palette.accent,
                Vec3::new(horizontal, 0.38, 0.13),
                Vec3::new(0.012, 0.13, 0.012),
            );
        }
    }
    ellipsoid(
        commands,
        geometry,
        root,
        &palette.skin,
        Vec3::new(0.0, 0.67, 0.0),
        Vec3::new(0.074, 0.13, 0.08),
    );

    let head_rest = Transform::from_xyz(0.0, 0.74, 0.0);
    let head = commands.spawn((head_rest, Visibility::default())).id();
    commands.entity(root).add_child(head);
    animated(commands, head, index, PartKind::Head, head_rest);
    ellipsoid(
        commands,
        geometry,
        head,
        &palette.skin,
        Vec3::new(0.0, 0.255, 0.0),
        Vec3::new(0.183, 0.257, 0.17),
    );
    ellipsoid(
        commands,
        geometry,
        head,
        &palette.skin,
        Vec3::new(0.0, 0.105, 0.026),
        Vec3::new(0.126, 0.105, 0.124),
    );
    for side in [-1.0, 1.0] {
        ellipsoid(
            commands,
            geometry,
            head,
            &palette.skin,
            Vec3::new(side * 0.18, 0.24, -0.01),
            Vec3::new(0.027, 0.049, 0.02),
        );
        let eye_rest = Transform::from_xyz(side * 0.071, 0.285, 0.153);
        let eye = commands.spawn((eye_rest, Visibility::default())).id();
        commands.entity(head).add_child(eye);
        animated(commands, eye, index, PartKind::Eye, eye_rest);
        ellipsoid(
            commands,
            geometry,
            eye,
            &palette.white,
            Vec3::ZERO,
            Vec3::new(0.038, 0.018, 0.014),
        );
        ellipsoid(
            commands,
            geometry,
            eye,
            &palette.iris,
            Vec3::new(-side * 0.002, 0.0, 0.012),
            Vec3::new(0.014, 0.014, 0.007),
        );
        ellipsoid(
            commands,
            geometry,
            eye,
            &palette.dark,
            Vec3::new(-side * 0.002, 0.0, 0.017),
            Vec3::new(0.007, 0.01, 0.005),
        );
        ellipsoid(
            commands,
            geometry,
            eye,
            &palette.white,
            Vec3::new(-0.005, 0.006, 0.022),
            Vec3::splat(0.003),
        );
        let brow = ellipsoid(
            commands,
            geometry,
            head,
            &palette.hair,
            Vec3::new(side * 0.072, 0.329, 0.151),
            Vec3::new(0.046, 0.007, 0.009),
        );
        commands.entity(brow).insert(
            Transform::from_xyz(side * 0.072, 0.329, 0.151)
                .with_scale(Vec3::new(0.046, 0.007, 0.009))
                .with_rotation(Quat::from_rotation_z(side * 0.10)),
        );
    }
    ellipsoid(
        commands,
        geometry,
        head,
        &palette.skin,
        Vec3::new(0.0, 0.246, 0.164),
        Vec3::new(0.024, 0.053, 0.026),
    );
    ellipsoid(
        commands,
        geometry,
        head,
        &palette.skin,
        Vec3::new(0.0, 0.212, 0.18),
        Vec3::new(0.035, 0.02, 0.027),
    );
    for side in [-1.0, 1.0] {
        ellipsoid(
            commands,
            geometry,
            head,
            &palette.lips,
            Vec3::new(side * 0.019, 0.2, 0.19),
            Vec3::new(0.007, 0.004, 0.005),
        );
    }

    let mouth_rest =
        Transform::from_xyz(0.0, 0.147, 0.159).with_scale(Vec3::new(0.059, 0.005, 0.013));
    let mouth = object(
        commands,
        Some(head),
        &geometry.sphere,
        &palette.dark,
        mouth_rest,
    );
    animated(commands, mouth, index, PartKind::Mouth, mouth_rest);
    let upper_rest =
        Transform::from_xyz(0.0, 0.155, 0.167).with_scale(Vec3::new(0.061, 0.008, 0.008));
    let upper_lip = object(
        commands,
        Some(head),
        &geometry.sphere,
        &palette.lips,
        upper_rest,
    );
    animated(commands, upper_lip, index, PartKind::UpperLip, upper_rest);
    let lip_rest = Transform::from_xyz(0.0, 0.139, 0.17).with_scale(Vec3::new(0.058, 0.007, 0.008));
    let lower_lip = object(
        commands,
        Some(head),
        &geometry.sphere,
        &palette.lips,
        lip_rest,
    );
    animated(commands, lower_lip, index, PartKind::LowerLip, lip_rest);
    let teeth_rest =
        Transform::from_xyz(0.0, 0.15, 0.174).with_scale(Vec3::new(0.043, 0.004, 0.004));
    let teeth = object(
        commands,
        Some(head),
        &geometry.sphere,
        &palette.white,
        teeth_rest,
    );
    animated(commands, teeth, index, PartKind::Teeth, teeth_rest);

    let cap = meshes.add(hair_cap());
    object(
        commands,
        Some(head),
        &cap,
        &palette.hair,
        Transform::from_xyz(0.0, 0.264, -0.005).with_scale(Vec3::new(0.196, 0.263, 0.184)),
    );
    ellipsoid(
        commands,
        geometry,
        head,
        &palette.hair,
        Vec3::new(0.0, 0.11, -0.095),
        Vec3::new(0.208, if index == 0 { 0.40 } else { 0.30 }, 0.105),
    );
    for side in [-1.0, 1.0] {
        for lock in 0..4 {
            let horizontal = side * (0.178 + lock as f32 * 0.013);
            let vertical = if index == 0 { 0.055 } else { 0.14 };
            ellipsoid(
                commands,
                geometry,
                head,
                &palette.hair,
                Vec3::new(horizontal, vertical, -0.038 - lock as f32 * 0.024),
                Vec3::new(0.04, if index == 0 { 0.34 } else { 0.25 }, 0.035),
            );
            if index == 1 && lock % 2 == 0 {
                ellipsoid(
                    commands,
                    geometry,
                    head,
                    &palette.streak,
                    Vec3::new(
                        horizontal + side * 0.008,
                        vertical,
                        -0.015 - lock as f32 * 0.024,
                    ),
                    Vec3::new(0.008, 0.21, 0.009),
                );
            }
        }
    }

    for side in [-1.0, 1.0] {
        let arm_rest = Transform::from_xyz(side * 0.29, 0.55, 0.0);
        let arm = commands.spawn((arm_rest, Visibility::default())).id();
        commands.entity(root).add_child(arm);
        animated(commands, arm, index, PartKind::Arm(side), arm_rest);
        let parts = arm_parts(side);
        for (part, transform) in parts.iter().enumerate() {
            let material = if index == 1 && part == 0 {
                &palette.shirt
            } else {
                &palette.skin
            };
            object(commands, Some(arm), &geometry.sphere, material, *transform);
        }
        commands.entity(arm).insert(ArmBounds { parts });
    }
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let geometry = Geometry {
        sphere: meshes.add(Sphere::new(1.0).mesh().uv(24, 16)),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cylinder: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(40)),
    };
    let navy = matte(&mut materials, Color::srgb_u8(21, 31, 49));
    let panel = matte(&mut materials, Color::srgb_u8(35, 51, 69));
    let floor = matte(&mut materials, Color::srgb_u8(26, 35, 43));
    let wood = matte(&mut materials, Color::srgb_u8(103, 58, 38));
    let edge = matte(&mut materials, Color::srgb_u8(41, 33, 35));
    let gold = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(221, 159, 82),
        emissive: LinearRgba::new(1.0, 0.42, 0.12, 1.0),
        ..default()
    });
    let teal = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(64, 167, 160),
        emissive: LinearRgba::new(0.08, 0.6, 0.52, 1.0),
        ..default()
    });

    object(
        &mut commands,
        None,
        &geometry.cube,
        &floor,
        Transform::from_xyz(0.0, -0.06, 0.0).with_scale(Vec3::new(10.0, 0.1, 8.0)),
    );
    object(
        &mut commands,
        None,
        &geometry.cube,
        &navy,
        Transform::from_xyz(0.0, 2.0, -1.8).with_scale(Vec3::new(10.0, 4.0, 0.1)),
    );
    for column in -12..=12 {
        object(
            &mut commands,
            None,
            &geometry.cube,
            &panel,
            Transform::from_xyz(column as f32 * 0.30, 1.9, -1.69)
                .with_scale(Vec3::new(0.045, 3.5, 0.07)),
        );
    }
    for side in [-1.0, 1.0] {
        object(
            &mut commands,
            None,
            &geometry.cube,
            if side < 0.0 { &teal } else { &gold },
            Transform::from_xyz(side * 2.3, 1.9, -1.60).with_scale(Vec3::new(0.025, 2.5, 0.05)),
        );
        object(
            &mut commands,
            None,
            &geometry.cube,
            &edge,
            Transform::from_xyz(side * 0.76, 1.0, -0.29).with_scale(Vec3::new(0.64, 0.95, 0.13)),
        );
    }
    let ring = meshes.add(Torus::new(0.37, 0.40).mesh());
    object(
        &mut commands,
        None,
        &ring,
        &gold,
        Transform::from_xyz(0.0, 2.2, -1.60).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    for column in -3_i32..=3 {
        object(
            &mut commands,
            None,
            &geometry.cube,
            &teal,
            Transform::from_xyz(column as f32 * 0.07, 2.2, -1.54).with_scale(Vec3::new(
                0.027,
                0.10 + (3 - column.abs()) as f32 * 0.06,
                0.025,
            )),
        );
    }

    object(
        &mut commands,
        None,
        &geometry.cube,
        &wood,
        Transform::from_xyz(0.0, DESK_SURFACE_Y - DESK_THICKNESS * 0.5, 0.65)
            .with_scale(Vec3::new(2.7, DESK_THICKNESS, 1.0)),
    );
    object(
        &mut commands,
        None,
        &geometry.cube,
        &edge,
        Transform::from_xyz(0.0, 0.83, 0.67).with_scale(Vec3::new(2.7, 0.50, 0.96)),
    );
    for side in [-1.0, 1.0] {
        object(
            &mut commands,
            None,
            &geometry.cylinder,
            &wood,
            Transform::from_xyz(side * 1.35, DESK_SURFACE_Y - DESK_THICKNESS * 0.5, 0.65)
                .with_scale(Vec3::new(0.5, DESK_THICKNESS, 0.5)),
        );
        object(
            &mut commands,
            None,
            &geometry.cylinder,
            &edge,
            Transform::from_xyz(side * 1.35, 0.83, 0.67).with_scale(Vec3::new(0.48, 0.50, 0.48)),
        );
    }
    object(
        &mut commands,
        None,
        &geometry.cube,
        &teal,
        Transform::from_xyz(0.0, 0.85, 1.155).with_scale(Vec3::new(1.5, 0.016, 0.012)),
    );

    object(
        &mut commands,
        None,
        &geometry.cube,
        &edge,
        Transform::from_xyz(0.0, 0.29, 0.67).with_scale(Vec3::new(1.2, 0.58, 0.60)),
    );

    let white = matte(&mut materials, Color::srgb_u8(246, 239, 220));
    let dark = matte(&mut materials, Color::srgb_u8(32, 21, 26));
    for index in 0..2 {
        let palette = if index == 0 {
            Palette {
                skin: matte(&mut materials, Color::srgb_u8(192, 133, 87)),
                hair: matte(&mut materials, Color::srgb_u8(18, 22, 23)),
                streak: matte(&mut materials, Color::srgb_u8(36, 38, 38)),
                shirt: matte(&mut materials, Color::srgb_u8(41, 55, 63)),
                accent: matte(&mut materials, Color::srgb_u8(164, 104, 48)),
                lips: matte(&mut materials, Color::srgb_u8(143, 72, 59)),
                iris: matte(&mut materials, Color::srgb_u8(50, 42, 25)),
                white: white.clone(),
                dark: dark.clone(),
            }
        } else {
            Palette {
                skin: matte(&mut materials, Color::srgb_u8(220, 167, 139)),
                hair: matte(&mut materials, Color::srgb_u8(70, 53, 41)),
                streak: matte(&mut materials, Color::srgb_u8(123, 106, 86)),
                shirt: matte(&mut materials, Color::srgb_u8(100, 49, 51)),
                accent: matte(&mut materials, Color::srgb_u8(187, 155, 116)),
                lips: matte(&mut materials, Color::srgb_u8(159, 91, 80)),
                iris: matte(&mut materials, Color::srgb_u8(77, 66, 44)),
                white: white.clone(),
                dark: dark.clone(),
            }
        };
        host(&mut commands, &mut meshes, &geometry, &palette, index);
        let horizontal = if index == 0 { -0.76 } else { 0.76 };
        object(
            &mut commands,
            None,
            &geometry.cylinder,
            if index == 0 { &teal } else { &gold },
            Transform::from_xyz(horizontal + 0.29, 1.235, 0.91)
                .with_scale(Vec3::new(0.053, 0.13, 0.053)),
        );
        object(
            &mut commands,
            None,
            &geometry.cylinder,
            &dark,
            Transform::from_xyz(horizontal - 0.27, 1.18, 0.86)
                .with_scale(Vec3::new(0.065, 0.018, 0.065)),
        );
        object(
            &mut commands,
            None,
            &geometry.cylinder,
            &dark,
            Transform::from_xyz(horizontal - 0.27, 1.30, 0.86)
                .with_scale(Vec3::new(0.009, 0.24, 0.009)),
        );
        object(
            &mut commands,
            None,
            &geometry.sphere,
            &dark,
            Transform::from_xyz(horizontal - 0.27, 1.45, 0.84)
                .with_scale(Vec3::new(0.039, 0.072, 0.039)),
        );
    }

    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb_u8(186, 202, 223),
        brightness: 350.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 3500.0,
            shadow_maps_enabled: false,
            color: Color::srgb_u8(255, 226, 195),
            ..default()
        },
        Transform::from_xyz(-3.0, 5.0, 4.0).looking_at(Vec3::new(0.0, 1.4, 0.0), Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            intensity: 45_000.0,
            color: Color::srgb_u8(91, 192, 195),
            range: 8.0,
            ..default()
        },
        Transform::from_xyz(-2.0, 2.5, -0.6),
    ));
    commands.spawn((
        PointLight {
            intensity: 35_000.0,
            color: Color::srgb_u8(239, 161, 96),
            range: 8.0,
            ..default()
        },
        Transform::from_xyz(2.0, 2.5, -0.6),
    ));
    commands.spawn((
        StudioCamera,
        Camera3d::default(),
        NoIndirectDrawing,
        Msaa::Sample4,
        Transform::from_xyz(0.0, 2.12, 4.25).looking_at(Vec3::new(0.0, 1.44, 0.0), Vec3::Y),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_arms_clear_the_desk_during_breathing_leaning_and_gestures() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                let bounds = ArmBounds {
                    parts: arm_parts(side),
                };
                for frame in 0..960 {
                    let elapsed = frame as f32 * 0.125;
                    for speech_step in 0..=10 {
                        let speech = speech_step as f32 / 10.0;
                        let (rotation, height) = host_motion(elapsed, index, speech);
                        let host =
                            Transform::from_xyz(if index == 0 { -0.76 } else { 0.76 }, height, 0.0)
                                .with_rotation(rotation);
                        let mut arm = Transform::from_xyz(side * 0.29, 0.55, 0.0).with_rotation(
                            arm_rotation(elapsed + index as f32 * 1.83, side, speech),
                        );
                        let original = arm.translation;
                        bounds.keep_above_desk(&host, &mut arm);
                        assert!(
                            bounds.lowest_y(&host, &arm)
                                >= DESK_SURFACE_Y + ARM_CLEARANCE - 0.00001,
                            "host {index}, side {side}, time {elapsed}, speech {speech}"
                        );
                        assert!(
                            (arm.translation - original).length() < 0.06,
                            "shoulder correction is too large"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn correction_accounts_for_rotated_nonuniform_parent_and_is_idempotent() {
        let host = Transform::from_xyz(0.0, 0.72, 0.0)
            .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.15, 0.2, -0.12))
            .with_scale(Vec3::new(1.1, 0.9, 1.2));
        let bounds = ArmBounds {
            parts: arm_parts(1.0),
        };
        let mut arm = Transform::from_xyz(0.29, 0.55, 0.0);
        assert!(bounds.lowest_y(&host, &arm) < DESK_SURFACE_Y);
        bounds.keep_above_desk(&host, &mut arm);
        assert!((bounds.lowest_y(&host, &arm) - DESK_SURFACE_Y - ARM_CLEARANCE).abs() < 0.00001);
        let corrected = arm.translation;
        bounds.keep_above_desk(&host, &mut arm);
        assert!((corrected - arm.translation).length() < 0.00001);
    }

    #[test]
    fn already_clear_arms_do_not_move() {
        let host = Transform::from_xyz(0.0, 1.0, 0.0);
        let bounds = ArmBounds {
            parts: arm_parts(-1.0),
        };
        let mut arm = Transform::from_xyz(-0.29, 0.55, 0.0);
        let original = arm;
        bounds.keep_above_desk(&host, &mut arm);
        assert_eq!(arm, original);
    }
}
