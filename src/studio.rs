use std::f32::consts::{FRAC_PI_2, PI};

use bevy::{
    camera::visibility::DynamicSkinnedMeshBounds,
    mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    prelude::*,
    render::view::NoIndirectDrawing,
};

mod body;
mod desk;
mod portrait;
mod sculpt;

pub fn teeth_pose(index: usize, mut rest: Transform, jaw: f32, round: f32, wide: f32) -> Transform {
    rest.scale.y *= portrait::tooth_visibility(index, jaw);
    rest.scale.x *= 1.0 - round * 0.45 + wide * 0.25;
    rest.translation.y -= portrait::TOOTH_TOP * rest.scale.y;
    rest.translation.z += round * 0.008;
    rest
}

pub fn oral_visibility(jaw: f32) -> Visibility {
    if jaw > 0.02 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

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

#[derive(Component)]
pub struct UpperLipContour {
    rest_positions: Vec<[f32; 3]>,
}

impl UpperLipContour {
    fn new(mesh: &Mesh) -> Self {
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            unreachable!()
        };
        Self {
            rest_positions: positions.clone(),
        }
    }

    pub fn deform(&self, mesh: &mut Mesh, jaw: f32, vertical_scale: f32) {
        if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            for (position, rest) in positions.iter_mut().zip(&self.rest_positions) {
                let arch = (1.0 - rest[0].powi(2)).max(0.0).sqrt();
                *position = *rest;
                position[1] += jaw * (0.025 * arch - 0.014) / vertical_scale;
            }
        }
        mesh.compute_area_weighted_normals();
    }
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

pub const DESK_SURFACE_Y: f32 = desk::SURFACE_Y;
const ARM_CLEARANCE: f32 = 0.005;

#[derive(Component)]
pub struct ArmBounds {
    parts: [Transform; 5],
    vertices: Vec<Vec3>,
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
            .chain(
                self.vertices
                    .iter()
                    .map(|vertex| parent.transform_point3(*vertex).y),
            )
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
        DESK_SURFACE_Y - body::NAVEL_Y + 0.006 * (phase * 1.7).sin(),
    )
}

fn host_rest(index: usize) -> Transform {
    let depth = if index == 0 {
        desk::BACK_EDGE_Z - 0.30
    } else {
        0.0
    };
    Transform::from_xyz(
        if index == 0 { -0.76 } else { 0.76 },
        DESK_SURFACE_Y - body::NAVEL_Y,
        depth,
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

fn hand_parts(side: f32) -> [Transform; 5] {
    let wrist = body::wrist(side);
    std::array::from_fn(|index| {
        if index == 0 {
            Transform::from_translation(wrist + Vec3::new(0.0, 0.0, 0.06))
                .with_scale(Vec3::new(0.052, 0.031, 0.088))
        } else {
            Transform::from_translation(
                wrist + Vec3::new(((index - 1) as f32 - 1.5) * 0.021, -0.002, 0.126),
            )
            .with_scale(Vec3::new(0.011, 0.018, 0.033))
        }
    })
}

fn arm_bounds(index: usize, side: f32) -> ArmBounds {
    let surface = body::surface(index);
    let joint = if side < 0.0 { 1 } else { 2 };
    ArmBounds {
        parts: hand_parts(side),
        vertices: surface
            .positions
            .iter()
            .zip(&surface.weights)
            .filter(|(_, weights)| weights[joint] > 0.0)
            .map(|(point, _)| Vec3::from_array(*point) - body::shoulder(side))
            .collect(),
    }
}

struct Palette {
    skin: Handle<StandardMaterial>,
    body: Handle<StandardMaterial>,
    hair: Handle<StandardMaterial>,
    streak: Handle<StandardMaterial>,
    shirt: Handle<StandardMaterial>,
    lips: Handle<StandardMaterial>,
    crease: Handle<StandardMaterial>,
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

fn mouth_geometry(index: usize) -> [Transform; 4] {
    let portrait = portrait::Portrait::for_host(index);
    let mouth_depth = portrait::face_front(index, 0.0, portrait.mouth_y);
    let mouth_rest = Transform::from_xyz(0.0, portrait.mouth_y, mouth_depth + 0.002)
        .with_scale(Vec3::new(portrait.mouth_width, 0.0005, 0.003));
    let upper_rest = Transform::from_xyz(0.0, portrait.mouth_y + 0.004, mouth_depth + 0.009)
        .with_scale(Vec3::new(portrait.mouth_width + 0.002, 0.006, 0.007));
    let lip_rest = Transform::from_xyz(0.0, portrait.mouth_y - 0.004, mouth_depth + 0.011)
        .with_scale(Vec3::new(portrait.mouth_width - 0.001, 0.007, 0.008));
    let teeth_rest = Transform::from_xyz(0.0, portrait.mouth_y - 0.001, mouth_depth + 0.007)
        .with_scale(Vec3::new(
            portrait.mouth_width * if index == 0 { 0.75 } else { 0.73 },
            if index == 0 { 0.010 } else { 0.004 },
            0.0015,
        ));
    [mouth_rest, upper_rest, lip_rest, teeth_rest]
}

fn host(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    geometry: &Geometry,
    palette: &Palette,
    bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
    index: usize,
) {
    let portrait = portrait::Portrait::for_host(index);
    let root = commands
        .spawn((Host { index }, host_rest(index), Visibility::default()))
        .id();
    let garment = meshes.add(portrait::garment(index));
    object(
        commands,
        Some(root),
        &garment,
        &palette.white,
        Transform::default(),
    );
    if index == 1 {
        for side in [-1.0, 1.0] {
            let collar = meshes.add(portrait::collar(side));
            object(
                commands,
                Some(root),
                &collar,
                &palette.shirt,
                Transform::default(),
            );
        }
        for button in 0..4 {
            let height = 0.15 + button as f32 * 0.1;
            ellipsoid(
                commands,
                geometry,
                root,
                &palette.white,
                Vec3::new(0.0, height, body::front(index, 0.0, height) + 0.010),
                Vec3::splat(0.008),
            );
        }
    }

    let head_rest = Transform::from_xyz(0.0, 0.74, 0.0);
    let head = commands.spawn((head_rest, Visibility::default())).id();
    commands.entity(root).add_child(head);
    animated(commands, head, index, PartKind::Head, head_rest);
    let face = meshes.add(portrait::face(index));
    object(
        commands,
        Some(head),
        &face,
        &palette.skin,
        Transform::default(),
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
        let eye_rest = Transform::from_xyz(
            side * portrait.eye_spacing,
            portrait.eye_y,
            portrait::face_front(index, side * portrait.eye_spacing, portrait.eye_y) + 0.004,
        )
        .with_rotation(Quat::from_rotation_z(
            side * if index == 0 { 0.10 } else { -0.04 },
        ));
        let eye = commands.spawn((eye_rest, Visibility::default())).id();
        commands.entity(head).add_child(eye);
        animated(commands, eye, index, PartKind::Eye, eye_rest);
        let sclera = meshes.add(portrait::eye());
        object(
            commands,
            Some(eye),
            &sclera,
            &palette.white,
            Transform::from_scale(Vec3::new(portrait.eye_width, portrait.eye_height, 0.009)),
        );
        for upper in [false, true] {
            let lid = meshes.add(portrait::eyelid(index, upper));
            object(
                commands,
                Some(eye),
                &lid,
                if upper {
                    &palette.crease
                } else {
                    &palette.skin
                },
                Transform::default(),
            );
        }
        ellipsoid(
            commands,
            geometry,
            eye,
            &palette.iris,
            Vec3::new(-side * 0.002, 0.0, 0.008),
            Vec3::new(
                if index == 0 { 0.016 } else { 0.014 },
                portrait.eye_height * 0.92,
                0.005,
            ),
        );
        ellipsoid(
            commands,
            geometry,
            eye,
            &palette.dark,
            Vec3::new(-side * 0.002, 0.0, 0.012),
            Vec3::new(0.006, portrait.eye_height * 0.78, 0.003),
        );
        ellipsoid(
            commands,
            geometry,
            eye,
            &palette.white,
            Vec3::new(-0.004, 0.004, 0.016),
            Vec3::splat(0.002),
        );
        for segment in 0..5 {
            let progress = segment as f32 / 5.0;
            let next = (segment + 1) as f32 / 5.0;
            let point = |amount: f32| {
                Vec3::new(
                    side * (portrait.eye_spacing - 0.040 + amount * 0.080),
                    portrait.eye_y + 0.034 + (amount * PI).sin() * 0.009,
                    portrait::face_front(
                        index,
                        side * (portrait.eye_spacing - 0.040 + amount * 0.080),
                        portrait.eye_y + 0.034 + (amount * PI).sin() * 0.009,
                    ) + 0.002,
                )
            };
            object(
                commands,
                Some(head),
                &geometry.sphere,
                &palette.hair,
                limb_transform(
                    point(progress),
                    point(next),
                    if index == 0 { 0.0045 } else { 0.0055 },
                ),
            );
        }
    }
    for side in [-1.0, 1.0] {
        ellipsoid(
            commands,
            geometry,
            head,
            &palette.crease,
            Vec3::new(
                side * portrait.nose_width * 0.57,
                0.193,
                portrait::face_front(index, side * portrait.nose_width * 0.57, 0.193) + 0.001,
            ),
            Vec3::new(0.006, 0.0025, 0.002),
        );
    }

    let upper_mesh = portrait::lip(index, true);
    let upper_contour = UpperLipContour::new(&upper_mesh);
    let upper_shape = meshes.add(upper_mesh);
    let lower_shape = meshes.add(portrait::lip(index, false));
    let [mouth_rest, upper_rest, lip_rest, teeth_rest] = mouth_geometry(index);
    let mouth = object(
        commands,
        Some(head),
        &geometry.sphere,
        &palette.dark,
        mouth_rest,
    );
    animated(commands, mouth, index, PartKind::Mouth, mouth_rest);
    let upper_lip = object(
        commands,
        Some(head),
        &upper_shape,
        &palette.lips,
        upper_rest,
    );
    animated(commands, upper_lip, index, PartKind::UpperLip, upper_rest);
    commands.entity(upper_lip).insert(upper_contour);
    let lower_lip = object(commands, Some(head), &lower_shape, &palette.lips, lip_rest);
    animated(commands, lower_lip, index, PartKind::LowerLip, lip_rest);
    let tooth_shape = meshes.add(portrait::teeth());
    let teeth = object(
        commands,
        Some(head),
        &tooth_shape,
        &palette.white,
        teeth_rest,
    );
    animated(commands, teeth, index, PartKind::Teeth, teeth_rest);

    for segment in 0..8 {
        let start_polar = 0.12 + segment as f32 * 0.105;
        let end_polar = start_polar + 0.105;
        let radial = (1.0 - (portrait.hair_part / 0.205).powi(2)).sqrt();
        let start = Vec3::new(
            portrait.hair_part,
            0.264 + 0.282 * radial * start_polar.cos(),
            0.209 * radial * start_polar.sin(),
        );
        let end = Vec3::new(
            portrait.hair_part,
            0.264 + 0.282 * radial * end_polar.cos(),
            0.209 * radial * end_polar.sin(),
        );
        object(
            commands,
            Some(head),
            &geometry.sphere,
            &palette.streak,
            limb_transform(start, end, 0.0018),
        );
    }
    for side in [-1.0, 1.0] {
        for strand in 0..4 {
            let strand = meshes.add(portrait::crown_strand(index, side, strand));
            object(
                commands,
                Some(head),
                &strand,
                &palette.streak,
                Transform::default(),
            );
        }
    }
    let cap = meshes.add(portrait::hair_cap(index));
    object(
        commands,
        Some(head),
        &cap,
        &palette.hair,
        Transform::default(),
    );
    ellipsoid(
        commands,
        geometry,
        head,
        &palette.hair,
        Vec3::new(0.0, if index == 0 { 0.08 } else { 0.07 }, -0.10),
        Vec3::new(0.19, if index == 0 { 0.34 } else { 0.30 }, 0.10),
    );
    for side in [-1.0, 1.0] {
        for lock in 0..5 {
            let hair = meshes.add(portrait::hair_lock(index, side, lock, false));
            object(
                commands,
                Some(head),
                &hair,
                &palette.hair,
                Transform::default(),
            );
            if lock % 2 == 0 {
                let streak = meshes.add(portrait::hair_lock(index, side, lock, true));
                object(
                    commands,
                    Some(head),
                    &streak,
                    &palette.streak,
                    Transform::default(),
                );
            }
        }
    }

    let mut joints = vec![root];
    let mut inverse_bindposes = vec![Mat4::IDENTITY];
    for side in [-1.0, 1.0] {
        let arm_rest = Transform::from_translation(body::shoulder(side));
        let arm = commands.spawn((arm_rest, Visibility::default())).id();
        commands.entity(root).add_child(arm);
        animated(commands, arm, index, PartKind::Arm(side), arm_rest);
        for transform in hand_parts(side) {
            object(
                commands,
                Some(arm),
                &geometry.sphere,
                &palette.skin,
                transform,
            );
        }
        commands.entity(arm).insert(arm_bounds(index, side));
        joints.push(arm);
        inverse_bindposes.push(Mat4::from_translation(-body::shoulder(side)));
    }
    let skin = meshes.add(body::surface(index).mesh());
    let surface = object(
        commands,
        Some(root),
        &skin,
        &palette.body,
        Transform::default(),
    );
    commands.entity(surface).insert((
        SkinnedMesh {
            inverse_bindposes: bindposes.add(inverse_bindposes),
            joints,
        },
        DynamicSkinnedMeshBounds,
    ));
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
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

    let tabletop = meshes.add(desk::tabletop());
    let apron = meshes.add(desk::apron());
    object(&mut commands, None, &tabletop, &wood, Transform::default());
    object(&mut commands, None, &apron, &edge, Transform::default());
    object(
        &mut commands,
        None,
        &geometry.cube,
        &teal,
        Transform::from_xyz(0.0, 0.85, 1.125).with_scale(Vec3::new(1.5, 0.016, 0.012)),
    );

    object(
        &mut commands,
        None,
        &geometry.cube,
        &edge,
        Transform::from_xyz(0.0, 0.29, 0.67).with_scale(Vec3::new(1.2, 0.58, 0.60)),
    );

    let white = matte(&mut materials, Color::srgb_u8(246, 239, 220));
    let body_material = matte(&mut materials, Color::WHITE);
    let dark = matte(&mut materials, Color::srgb_u8(32, 21, 26));
    for index in 0..2 {
        let palette = if index == 0 {
            Palette {
                skin: matte(&mut materials, body::skin_color(0)),
                body: body_material.clone(),
                hair: matte(&mut materials, Color::srgb_u8(18, 22, 23)),
                streak: matte(&mut materials, Color::srgb_u8(30, 31, 30)),
                shirt: matte(&mut materials, Color::srgb_u8(41, 55, 63)),
                lips: matte(&mut materials, Color::srgb_u8(143, 72, 59)),
                crease: matte(&mut materials, Color::srgb_u8(113, 70, 48)),
                iris: matte(&mut materials, Color::srgb_u8(50, 42, 25)),
                white: white.clone(),
                dark: dark.clone(),
            }
        } else {
            Palette {
                skin: matte(&mut materials, body::skin_color(1)),
                body: body_material.clone(),
                hair: matte(&mut materials, Color::srgb_u8(70, 53, 41)),
                streak: matte(&mut materials, Color::srgb_u8(123, 106, 86)),
                shirt: matte(&mut materials, Color::srgb_u8(166, 141, 111)),
                lips: matte(&mut materials, Color::srgb_u8(147, 94, 81)),
                crease: matte(&mut materials, Color::srgb_u8(143, 106, 89)),
                iris: matte(&mut materials, Color::srgb_u8(77, 66, 44)),
                white: white.clone(),
                dark: dark.clone(),
            }
        };
        host(
            &mut commands,
            &mut meshes,
            &geometry,
            &palette,
            &mut bindposes,
            index,
        );
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
        Transform::from_xyz(0.0, 2.32, 4.25).looking_at(Vec3::new(0.0, 1.64, 0.0), Vec3::Y),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_body_material_preserves_full_skin_vertex_colors() {
        use bevy::mesh::VertexAttributeValues;

        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
            .add_systems(Startup, setup);
        app.update();
        let world = app.world_mut();
        let mut query = world.query_filtered::<(
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &ChildOf,
            &SkinnedMesh,
        ), With<DynamicSkinnedMeshBounds>>();
        let mut count = 0;
        for (mesh, material, parent, _) in query.iter(world) {
            let host = world.get::<Host>(parent.parent()).unwrap();
            let material = world
                .resource::<Assets<StandardMaterial>>()
                .get(&material.0)
                .unwrap();
            assert_eq!(material.base_color.to_linear(), LinearRgba::WHITE);
            let mesh = world.resource::<Assets<Mesh>>().get(&mesh.0).unwrap();
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("missing positions")
            };
            let Some(VertexAttributeValues::Float32x4(colors)) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("missing colors")
            };
            let skin = body::skin_color(host.index).to_linear();
            let mut neck_vertices = 0;
            for (position, color) in positions.iter().zip(colors) {
                if position[1] > 0.67 {
                    assert_eq!(*color, [skin.red, skin.green, skin.blue, 1.0]);
                    neck_vertices += 1;
                }
            }
            assert!(neck_vertices > 0);
            assert!(mesh.skinned_mesh_bounds().is_some());
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn dynamic_skin_bounds_enclose_vertices_that_escape_static_bounds() {
        use bevy::{
            ecs::system::SystemState, mesh::skinning::entity_aabb_from_skinned_mesh_bounds,
        };

        let mut escaped_static_bounds = false;
        for index in 0..2 {
            let surface = body::surface(index);
            let mesh = surface.mesh();
            let generated = mesh.skinned_mesh_bounds().expect("missing skin bounds");
            assert_eq!(generated.aabbs.len(), 3);
            let (static_min, static_max) = surface.positions.iter().fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(minimum, maximum), point| {
                    let point = Vec3::from_array(*point);
                    (minimum.min(point), maximum.max(point))
                },
            );
            let mut world = World::new();
            let joints: Vec<_> = (0..3)
                .map(|_| world.spawn(GlobalTransform::IDENTITY).id())
                .collect();
            let skinned_mesh = SkinnedMesh {
                joints,
                ..default()
            };
            let inverse_bindposes = SkinnedMeshInverseBindposes::from(vec![
                Mat4::IDENTITY,
                Mat4::from_translation(-body::shoulder(-1.0)),
                Mat4::from_translation(-body::shoulder(1.0)),
            ]);
            let mut query_state = SystemState::<Query<&GlobalTransform>>::new(&mut world);
            let arm_bounds = [arm_bounds(index, -1.0), arm_bounds(index, 1.0)];
            for frame in 0..120 {
                let elapsed = frame as f32 * 0.5;
                for speech_step in 0..=10 {
                    let speech = speech_step as f32 / 10.0;
                    let (rotation, height) = host_motion(elapsed, index, speech);
                    let mut host = host_rest(index).with_rotation(rotation);
                    host.translation.y = height;
                    let root = host.to_matrix();
                    let joints: [Mat4; 3] = std::array::from_fn(|joint| {
                        if joint == 0 {
                            return root;
                        }
                        let side = if joint == 1 { -1.0 } else { 1.0 };
                        let mut arm =
                            Transform::from_translation(body::shoulder(side)).with_rotation(
                                arm_rotation(elapsed + index as f32 * 1.83, side, speech),
                            );
                        arm_bounds[joint - 1].keep_above_desk(&host, &mut arm);
                        root * arm.to_matrix()
                    });
                    for (entity, matrix) in skinned_mesh.joints.iter().zip(joints) {
                        world
                            .entity_mut(*entity)
                            .insert(GlobalTransform::from(matrix));
                    }
                    let bounds = entity_aabb_from_skinned_mesh_bounds(
                        &query_state.get(&world).unwrap(),
                        &mesh,
                        &skinned_mesh,
                        &inverse_bindposes,
                        Some(&GlobalTransform::from(root)),
                    )
                    .unwrap();
                    let matrices: [Mat4; 3] = std::array::from_fn(|joint| {
                        root.inverse() * joints[joint] * inverse_bindposes[joint]
                    });
                    let minimum = Vec3::from(bounds.min) - Vec3::splat(0.00001);
                    let maximum = Vec3::from(bounds.max) + Vec3::splat(0.00001);
                    for (point, weights) in surface.positions.iter().zip(&surface.weights) {
                        let point = Vec3::from_array(*point);
                        let skinned: Vec3 = (0..3)
                            .map(|joint| matrices[joint].transform_point3(point) * weights[joint])
                            .sum();
                        assert!(
                            skinned.cmpge(minimum).all() && skinned.cmple(maximum).all(),
                            "host {index}, time {elapsed}, speech {speech}: {skinned:?}"
                        );
                        escaped_static_bounds |=
                            skinned.cmplt(static_min - Vec3::splat(0.0001)).any()
                                || skinned.cmpgt(static_max + Vec3::splat(0.0001)).any();
                    }
                }
            }
        }
        assert!(escaped_static_bounds);
    }

    #[test]
    fn chest_and_top_stay_behind_the_desk_while_hands_still_reach_it() {
        use bevy::mesh::VertexAttributeValues;

        let surface = body::surface(0);
        let garment = portrait::garment(0);
        let Some(VertexAttributeValues::Float32x3(garment_positions)) =
            garment.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing garment positions")
        };
        let torso_positions = surface
            .positions
            .iter()
            .zip(&surface.weights)
            .filter(|(_, weights)| weights[0] >= 0.99)
            .map(|(point, _)| point)
            .chain(garment_positions);
        let (minimum, maximum) = torso_positions.fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(minimum, maximum), point| {
                let point = Vec3::from_array(*point);
                (minimum.min(point), maximum.max(point))
            },
        );
        let bounds = [arm_bounds(0, -1.0), arm_bounds(0, 1.0)];
        for frame in 0..960 {
            let elapsed = frame as f32 * 0.125;
            for speech_step in 0..=10 {
                let speech = speech_step as f32 / 10.0;
                let (rotation, height) = host_motion(elapsed, 0, speech);
                let mut host = host_rest(0).with_rotation(rotation);
                host.translation.y = height;
                for corner in 0..8 {
                    let point = Vec3::from_array(std::array::from_fn(|axis| {
                        if corner & (1 << axis) == 0 {
                            minimum[axis]
                        } else {
                            maximum[axis]
                        }
                    }));
                    assert!(
                        host.transform_point(point).z <= desk::BACK_EDGE_Z - 0.02,
                        "torso intersects desk: time {elapsed}, speech {speech}, corner {point:?}"
                    );
                }
                for (joint, side) in [(0, -1.0), (1, 1.0)] {
                    let mut arm = Transform::from_translation(body::shoulder(side))
                        .with_rotation(arm_rotation(elapsed, side, speech));
                    bounds[joint].keep_above_desk(&host, &mut arm);
                    for hand in hand_parts(side) {
                        let matrix = host.to_matrix() * arm.to_matrix() * hand.to_matrix();
                        let depth_extent =
                            Vec3::new(matrix.x_axis.z, matrix.y_axis.z, matrix.z_axis.z).length();
                        assert!(
                            matrix.w_axis.z - depth_extent >= desk::BACK_EDGE_Z + ARM_CLEARANCE,
                            "hand no longer reaches desk: time {elapsed}, speech {speech}, side {side}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn complete_arms_clear_the_desk_during_breathing_leaning_and_gestures() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                let bounds = arm_bounds(index, side);
                for frame in 0..960 {
                    let elapsed = frame as f32 * 0.125;
                    for speech_step in 0..=10 {
                        let speech = speech_step as f32 / 10.0;
                        let (rotation, height) = host_motion(elapsed, index, speech);
                        let mut host = host_rest(index).with_rotation(rotation);
                        host.translation.y = height;
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
    fn blended_skin_vertices_clear_the_desk_during_gestures() {
        for index in 0..2 {
            let surface = body::surface(index);
            let bounds = [arm_bounds(index, -1.0), arm_bounds(index, 1.0)];
            for frame in 0..120 {
                let elapsed = frame as f32 * 0.5;
                for speech_step in 0..=10 {
                    let speech = speech_step as f32 / 10.0;
                    let (rotation, height) = host_motion(elapsed, index, speech);
                    let mut host = host_rest(index).with_rotation(rotation);
                    host.translation.y = height;
                    let matrices: [Mat4; 2] = std::array::from_fn(|joint| {
                        let side = if joint == 0 { -1.0 } else { 1.0 };
                        let mut arm =
                            Transform::from_translation(body::shoulder(side)).with_rotation(
                                arm_rotation(elapsed + index as f32 * 1.83, side, speech),
                            );
                        bounds[joint].keep_above_desk(&host, &mut arm);
                        host.to_matrix()
                            * arm.to_matrix()
                            * Mat4::from_translation(-body::shoulder(side))
                    });
                    for (point, weights) in surface.positions.iter().zip(&surface.weights) {
                        if weights[1] + weights[2] == 0.0 {
                            continue;
                        }
                        let point = Vec3::from_array(*point);
                        let skinned = host.transform_point(point) * weights[0]
                            + matrices[0].transform_point3(point) * weights[1]
                            + matrices[1].transform_point3(point) * weights[2];
                        assert!(skinned.is_finite());
                        assert!(
                            skinned.y >= DESK_SURFACE_Y + ARM_CLEARANCE - 0.00001,
                            "host {index}, time {elapsed}, speech {speech}: {point:?} -> {skinned:?}"
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
        let bounds = arm_bounds(0, 1.0);
        let mut arm = Transform::from_xyz(0.29, 0.55, 0.0);
        assert!(bounds.lowest_y(&host, &arm) < DESK_SURFACE_Y);
        bounds.keep_above_desk(&host, &mut arm);
        assert!((bounds.lowest_y(&host, &arm) - DESK_SURFACE_Y - ARM_CLEARANCE).abs() < 0.00001);
        let corrected = arm.translation;
        bounds.keep_above_desk(&host, &mut arm);
        assert!((corrected - arm.translation).length() < 0.00001);
    }

    #[test]
    fn teeth_stay_behind_lips_and_inside_mouth_bounds_across_all_shapes() {
        use bevy::mesh::VertexAttributeValues;

        let mesh = portrait::teeth();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing tooth positions")
        };
        for index in 0..2 {
            let [mouth, upper, lower, teeth] = mouth_geometry(index);
            for jaw_step in 0..=10 {
                for round_step in 0..=10 {
                    for wide_step in 0..=10 {
                        let jaw = jaw_step as f32 / 10.0;
                        let round = round_step as f32 / 10.0;
                        let wide = wide_step as f32 / 10.0;
                        let pose = teeth_pose(index, teeth, jaw, round, wide);
                        if oral_visibility(jaw) == Visibility::Hidden {
                            assert_eq!(pose.scale.y, 0.0);
                            continue;
                        }
                        let matrix = pose.to_matrix();
                        let pucker = round * 0.008;
                        let mouth_width = mouth.scale.x * (1.0 - round * 0.45 + wide * 0.25);
                        let mouth_bottom =
                            mouth.translation.y - jaw * 0.014 - mouth.scale.y - jaw * 0.025;
                        let mut uppermost = f32::NEG_INFINITY;
                        for position in positions {
                            let point = matrix.transform_point3(Vec3::from_array(*position));
                            uppermost = uppermost.max(point.y);
                            assert!(point.z < upper.translation.z + pucker);
                            assert!(point.z < lower.translation.z + pucker);
                            assert!(point.z > mouth.translation.z + mouth.scale.z + pucker);
                            assert!(point.x.abs() <= mouth_width);
                            assert!(point.y >= mouth_bottom - 0.000001);
                            assert!(point.y <= upper.translation.y);
                        }
                        assert!(
                            (uppermost - teeth.translation.y).abs() < 0.000001,
                            "gumline moved: host {index}, jaw {jaw}, round {round}, wide {wide}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn upper_lips_follow_the_opening_without_upturned_tips_or_accumulated_deformation() {
        use bevy::mesh::VertexAttributeValues;

        let sphere = Sphere::new(1.0).mesh().uv(32, 16);
        let Some(VertexAttributeValues::Float32x3(sphere_positions)) =
            sphere.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing sphere positions")
        };
        for index in 0..2 {
            let [mouth, upper, _, _] = mouth_geometry(index);
            let mut mesh = portrait::lip(index, true);
            let contour = UpperLipContour::new(&mesh);
            for jaw_step in 0..=10 {
                let jaw = jaw_step as f32 / 10.0;
                for round_step in 0..=10 {
                    let round = round_step as f32 / 10.0;
                    for wide_step in 0..=10 {
                        let wide = wide_step as f32 / 10.0;
                        let mut pose = upper;
                        pose.scale.x *= 1.0 - round * 0.45 + wide * 0.25;
                        pose.scale.y *= 1.0 + round * 0.4;
                        contour.deform(&mut mesh, jaw, pose.scale.y);
                        let Some(VertexAttributeValues::Float32x3(positions)) =
                            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                        else {
                            panic!("missing lip positions")
                        };
                        for (position, sphere_position) in positions.iter().zip(sphere_positions) {
                            let point = pose.transform_point(Vec3::from_array(*position));
                            let centerline_y = point.y - sphere_position[1] * pose.scale.y;
                            let mouth_width = mouth.scale.x * (1.0 - round * 0.45 + wide * 0.25);
                            let arch = (1.0 - (point.x / mouth_width).powi(2)).max(0.0).sqrt();
                            let boundary_y = mouth.translation.y - jaw * 0.014
                                + (mouth.scale.y + jaw * 0.025) * arch;
                            let margin = centerline_y - boundary_y;
                            assert!(
                                (0.0005..0.009).contains(&margin),
                                "host {index}, jaw {jaw}, round {round}, wide {wide}: {margin}"
                            );
                            if position[0].abs() > 0.99 {
                                assert!(centerline_y < mouth.translation.y - jaw * 0.014 + 0.004);
                            }
                        }
                        let Some(VertexAttributeValues::Float32x3(normals)) =
                            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                        else {
                            panic!("missing lip normals")
                        };
                        assert!(normals.iter().flatten().all(|value| value.is_finite()));
                    }
                }
            }
            contour.deform(&mut mesh, 0.0, upper.scale.y);
            assert_eq!(
                mesh.attribute(Mesh::ATTRIBUTE_POSITION),
                Some(&VertexAttributeValues::Float32x3(contour.rest_positions))
            );
        }
    }

    #[test]
    fn silent_hosts_have_closed_lips_and_no_visible_oral_geometry() {
        for index in 0..2 {
            let [mouth, upper, lower, teeth] = mouth_geometry(index);
            assert!(lower.translation.y + lower.scale.y >= upper.translation.y - upper.scale.y);
            assert_eq!(oral_visibility(0.0), Visibility::Hidden);
            assert_eq!(oral_visibility(0.01), Visibility::Hidden);
            assert_eq!(oral_visibility(0.02), Visibility::Hidden);
            assert_eq!(oral_visibility(0.03), Visibility::Inherited);
            assert_eq!(teeth_pose(index, teeth, 0.0, 0.0, 0.0).scale.y, 0.0);
            assert!(mouth.scale.y < 0.001);
            let speaking = teeth_pose(index, teeth, 1.0, 0.0, 0.0);
            assert!(speaking.scale.y <= 0.00601);
            assert!(speaking.scale.x / mouth.scale.x <= 0.75);
        }
    }

    #[test]
    fn raised_hosts_keep_the_desk_at_navel_height_and_arms_relaxed() {
        for index in 0..2 {
            let rest = host_rest(index);
            assert!((rest.translation.y + body::NAVEL_Y - DESK_SURFACE_Y).abs() < 0.00001);
            for frame in 0..240 {
                let (rotation, height) = host_motion(frame as f32 * 0.25, index, 1.0);
                let host = Transform {
                    rotation,
                    translation: Vec3::new(0.0, height, 0.0),
                    ..rest
                };
                let navel = host.transform_point(Vec3::new(
                    0.0,
                    body::NAVEL_Y,
                    body::front(index, 0.0, body::NAVEL_Y),
                ));
                assert!((navel.y - DESK_SURFACE_Y).abs() < 0.015);
            }
            for side in [-1.0, 1.0] {
                let bounds = arm_bounds(index, side);
                let mut arm = Transform::from_translation(body::shoulder(side))
                    .with_rotation(arm_rotation(0.0, side, 0.0));
                bounds.keep_above_desk(&rest, &mut arm);
                let shoulder = rest.transform_point(arm.translation);
                let wrist = rest.transform_point(arm.transform_point(body::wrist(side)));
                assert!(shoulder.y - wrist.y > 0.35);
                assert!((DESK_SURFACE_Y..DESK_SURFACE_Y + 0.12).contains(&wrist.y));
            }
        }
    }

    #[test]
    fn already_clear_arms_do_not_move() {
        let host = Transform::from_xyz(0.0, 1.3, 0.0);
        let bounds = arm_bounds(0, -1.0);
        let mut arm = Transform::from_xyz(-0.29, 0.55, 0.0);
        let original = arm;
        bounds.keep_above_desk(&host, &mut arm);
        assert_eq!(arm, original);
    }
}
