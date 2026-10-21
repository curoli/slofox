use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::OnceLock;

use bevy::{
    camera::visibility::DynamicSkinnedMeshBounds,
    mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    prelude::*,
    render::view::NoIndirectDrawing,
};

mod attention;
mod body;
mod decor;
mod desk;
pub mod gestures;
mod portrait;
mod sculpt;

pub use attention::HeadAttention;

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
struct Ear;

#[derive(Component)]
struct BodySkin;

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
    Clavicle(f32),
    Hand(f32, usize),
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
    side: f32,
    #[cfg(test)]
    parts: [Transform; 7],
    vertices: Vec<Vec3>,
}

impl ArmBounds {
    #[cfg(test)]
    fn lowest_y(&self, host: &Transform, arm: &Transform) -> f32 {
        self.posed_lowest_y(host, arm, &self.parts)
    }

    fn posed_lowest_y(&self, host: &Transform, arm: &Transform, parts: &[Transform; 7]) -> f32 {
        let parent = host.to_matrix() * arm.to_matrix();
        parts
            .iter()
            .enumerate()
            .map(|(index, part)| {
                let matrix = parent * part.to_matrix();
                hand_minimum(matrix, index, Vec3::Y, self.side)
            })
            .chain(
                self.vertices
                    .iter()
                    .map(|vertex| parent.transform_point3(*vertex).y),
            )
            .fold(f32::INFINITY, f32::min)
    }

    #[cfg(test)]
    fn keep_above_desk(&self, host: &Transform, arm: &mut Transform) {
        self.keep_above_desk_with_hand(host, arm, &self.parts);
    }

    pub fn keep_above_desk_with_hand(
        &self,
        host: &Transform,
        arm: &mut Transform,
        parts: &[Transform; 7],
    ) -> Quat {
        if self.posed_lowest_y(host, arm, parts) >= DESK_SURFACE_Y + ARM_CLEARANCE {
            return Quat::IDENTITY;
        }
        let original = *arm;
        let mut lower = 0.0;
        let mut upper = 0.50;
        for _ in 0..16 {
            let angle = (lower + upper) * 0.5;
            let candidate = lift_arm(original, self.side, angle);
            if self.posed_lowest_y(host, &candidate, parts) < DESK_SURFACE_Y + ARM_CLEARANCE {
                lower = angle;
            } else {
                upper = angle;
            }
        }
        *arm = lift_arm(original, self.side, upper);
        Quat::from_rotation_z(self.side * upper)
    }
}

fn lift_arm(mut arm: Transform, side: f32, angle: f32) -> Transform {
    let rotation = Quat::from_rotation_z(side * angle);
    let pivot = body::clavicle_pivot();
    arm.translation = pivot + rotation * (arm.translation - pivot);
    arm.rotation = rotation * arm.rotation;
    arm
}

fn host_scale(index: usize) -> Vec3 {
    if index == 1 {
        Vec3::new(1.10, 1.055, 1.12)
    } else {
        Vec3::ONE
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
        DESK_SURFACE_Y - body::NAVEL_Y * host_scale(index).y + 0.006 * (phase * 1.7).sin(),
    )
}

fn host_rest(index: usize) -> Transform {
    let depth = if index == 0 {
        desk::BACK_EDGE_Z - 0.30
    } else {
        desk::BACK_EDGE_Z - 0.25
    };
    Transform::from_xyz(
        if index == 0 { -0.76 } else { 0.76 },
        DESK_SURFACE_Y - body::NAVEL_Y * host_scale(index).y,
        depth,
    )
    .with_scale(host_scale(index))
}

fn hand_parts(side: f32) -> [Transform; 7] {
    let wrist = body::wrist(side);
    std::array::from_fn(|index| {
        if index == 0 {
            Transform::from_translation(wrist + Vec3::new(0.0, 0.0, 0.045))
                .with_scale(Vec3::new(0.047, 0.020, 0.060))
        } else if index <= 4 {
            let finger = if side > 0.0 { index - 1 } else { 4 - index };
            let length = [0.112, 0.120, 0.116, 0.100][finger];
            Transform::from_translation(
                wrist
                    + Vec3::new(
                        ((index - 1) as f32 - 1.5) * 0.021,
                        -0.002,
                        0.085 + length * 0.5,
                    ),
            )
            .with_scale(Vec3::new(0.0095, 0.011, length * 0.5))
        } else {
            let base = wrist + Vec3::new(-side * 0.035, 0.002, 0.043);
            let joint = wrist + Vec3::new(-side * 0.065, 0.001, 0.080);
            let tip = wrist + Vec3::new(-side * 0.079, -0.003, 0.108);
            if index == 5 {
                limb_transform(base, joint, 0.015)
            } else {
                limb_transform(joint, tip, 0.013)
            }
        }
    })
}

fn hand_vertices(side: f32, index: usize) -> &'static [Vec3] {
    static VERTICES: OnceLock<[Vec<Vec3>; 5]> = OnceLock::new();
    let vertices = VERTICES.get_or_init(|| {
        std::array::from_fn(|mesh| {
            let shape = if mesh == 0 {
                portrait::finger(false)
            } else {
                let side = if mesh <= 2 { -1.0 } else { 1.0 };
                let part = 5 + (mesh - 1) % 2;
                let mut bind = hand_parts(side)[part];
                bind.translation -= body::wrist(side);
                portrait::thumb(side, part == 6, bind)
            };
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                shape.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                unreachable!()
            };
            positions.iter().copied().map(Vec3::from_array).collect()
        })
    });
    &vertices[if index <= 4 {
        0
    } else {
        1 + usize::from(side > 0.0) * 2 + index - 5
    }]
}

fn hand_extent(matrix: Mat4, index: usize, axis: Vec3, side: f32) -> f32 {
    let extent = Vec3::new(
        matrix.x_axis.truncate().dot(axis),
        matrix.y_axis.truncate().dot(axis),
        matrix.z_axis.truncate().dot(axis),
    );
    if index == 0 {
        extent.length()
    } else {
        hand_sections(side, index)
            .iter()
            .map(|section| {
                extent.dot(section[0]).abs()
                    + Vec2::new(extent.dot(section[1]), extent.dot(section[2])).length()
            })
            .fold(0.0_f32, f32::max)
    }
}

fn hand_sections(side: f32, index: usize) -> &'static [[Vec3; 3]] {
    static SECTIONS: OnceLock<[Vec<[Vec3; 3]>; 5]> = OnceLock::new();
    let sections = SECTIONS.get_or_init(|| {
        std::array::from_fn(|mesh| {
            let (side, part) = if mesh == 0 {
                (-1.0, 1)
            } else {
                (if mesh <= 2 { -1.0 } else { 1.0 }, 5 + (mesh - 1) % 2)
            };
            hand_vertices(side, part)
                .as_chunks::<21>()
                .0
                .iter()
                .map(|ring| {
                    let center = (ring[0] + ring[10]) * 0.5;
                    [center, ring[0] - center, ring[5] - center]
                })
                .collect()
        })
    });
    &sections[if index <= 4 {
        0
    } else {
        1 + usize::from(side > 0.0) * 2 + index - 5
    }]
}

fn hand_minimum(matrix: Mat4, index: usize, axis: Vec3, side: f32) -> f32 {
    let center = matrix.w_axis.truncate().dot(axis);
    if index == 0 {
        return center - hand_extent(matrix, index, axis, side);
    }
    let direction = Vec3::new(
        matrix.x_axis.truncate().dot(axis),
        matrix.y_axis.truncate().dot(axis),
        matrix.z_axis.truncate().dot(axis),
    );
    center
        + hand_sections(side, index)
            .iter()
            .map(|section| {
                direction.dot(section[0])
                    - Vec2::new(direction.dot(section[1]), direction.dot(section[2])).length()
            })
            .fold(f32::INFINITY, f32::min)
}

pub fn posed_hand_parts(side: f32, gesture: gestures::HandGesture) -> [Transform; 7] {
    let wrist = body::wrist(side);
    let mut parts = hand_parts(side);
    for (index, part) in parts.iter_mut().enumerate() {
        if (1..=4).contains(&index) {
            let pivot = wrist + Vec3::new(part.translation.x - wrist.x, -0.002, 0.085);
            let rotation = Quat::from_rotation_x(gesture.curl * (0.9 + index as f32 * 0.06));
            part.translation = pivot + rotation * (part.translation - pivot);
            part.rotation = rotation * part.rotation;
        }
        part.translation = wrist + gesture.wrist_rotation * (part.translation - wrist);
        part.rotation = gesture.wrist_rotation * part.rotation;
    }
    parts
}

fn arm_bounds(index: usize, side: f32) -> ArmBounds {
    let surface = body::surface(index);
    let joint = if side < 0.0 { 1 } else { 2 };
    ArmBounds {
        side,
        #[cfg(test)]
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
    let rim_depth = mouth_depth + 0.002;
    let mouth_rest = Transform::from_xyz(0.0, portrait.mouth_y, rim_depth).with_scale(Vec3::new(
        portrait.mouth_width,
        0.0005,
        0.0008,
    ));
    let upper_rest = Transform::from_xyz(0.0, portrait.mouth_y + 0.004, rim_depth + 0.0015)
        .with_scale(Vec3::new(portrait.mouth_width + 0.002, 0.006, 0.004));
    let lip_rest = Transform::from_xyz(0.0, portrait.mouth_y - 0.004, rim_depth + 0.0015)
        .with_scale(Vec3::new(portrait.mouth_width - 0.001, 0.007, 0.004));
    let teeth_rest = Transform::from_xyz(0.0, portrait.mouth_y - 0.001, rim_depth + 0.001)
        .with_scale(Vec3::new(
            portrait.mouth_width * if index == 0 { 0.75 } else { 0.73 },
            if index == 0 { 0.010 } else { 0.004 },
            0.0002,
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
    let mut garment_mesh = portrait::garment(index);
    body::skin_garment(index, &mut garment_mesh);
    let garment = meshes.add(garment_mesh);
    let garment_entity = object(
        commands,
        Some(root),
        &garment,
        if index == 1 {
            &palette.body
        } else {
            &palette.white
        },
        Transform::default(),
    );
    if index == 1 {
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

    let head_rest = Transform::from_xyz(0.0, 0.74 - body::NECK_LOWERING, 0.0);
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
        let ear = ellipsoid(
            commands,
            geometry,
            head,
            &palette.skin,
            if index == 0 {
                Vec3::new(side * 0.219, 0.265, 0.015)
            } else {
                Vec3::new(side * 0.18, 0.24, -0.01)
            },
            if index == 0 {
                Vec3::new(0.033, 0.058, 0.033)
            } else {
                Vec3::new(0.027, 0.049, 0.02)
            },
        );
        commands.entity(ear).insert(Ear);
        if index == 0 {
            ellipsoid(
                commands,
                geometry,
                head,
                &palette.crease,
                Vec3::new(side * 0.234, 0.267, 0.039),
                Vec3::new(0.011, 0.034, 0.007),
            );
        }
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

    for segment in 0..if portrait.draw_hair_part { 8 } else { 0 } {
        let start_polar = 0.12 + segment as f32 * 0.105;
        let end_polar = start_polar + 0.105;
        let radial = (1.0 - (portrait.hair_part / portrait.hair_width).powi(2)).sqrt();
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
    let back = meshes.add(portrait::back_hair(index));
    object(
        commands,
        Some(head),
        &back,
        &palette.hair,
        Transform::default(),
    );
    if index == 0 {
        let tail = meshes.add(portrait::ponytail());
        object(
            commands,
            Some(head),
            &tail,
            &palette.hair,
            Transform::default(),
        );
    }
    for side in [-1.0, 1.0] {
        for lock in 0..if index == 0 { 0 } else { 5 } {
            let hair = meshes.add(portrait::hair_lock(side, lock, false));
            object(
                commands,
                Some(head),
                &hair,
                &palette.hair,
                Transform::default(),
            );
            if lock % 2 == 0 {
                let streak = meshes.add(portrait::hair_lock(side, lock, true));
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
    let finger_shape = meshes.add(portrait::finger(false));
    for side in [-1.0, 1.0] {
        let arm_rest = Transform::from_translation(body::shoulder(side));
        let arm = commands.spawn((arm_rest, Visibility::default())).id();
        commands.entity(root).add_child(arm);
        animated(commands, arm, index, PartKind::Arm(side), arm_rest);
        for (part, transform) in hand_parts(side).into_iter().enumerate() {
            let thumb_shape = if part >= 5 {
                let mut bind = transform;
                bind.translation -= body::wrist(side);
                Some(meshes.add(portrait::thumb(side, part == 6, bind)))
            } else {
                None
            };
            let hand = object(
                commands,
                Some(arm),
                if part == 0 {
                    &geometry.sphere
                } else if part <= 4 {
                    &finger_shape
                } else {
                    thumb_shape.as_ref().unwrap()
                },
                &palette.skin,
                transform,
            );
            animated(commands, hand, index, PartKind::Hand(side, part), transform);
        }
        commands.entity(arm).insert(arm_bounds(index, side));
        joints.push(arm);
        inverse_bindposes.push(Mat4::from_translation(-body::shoulder(side)));
    }
    let skin = meshes.add(body::surface(index).mesh());
    for side in [-1.0, 1.0] {
        let rest = Transform::from_translation(body::clavicle_pivot());
        let joint = commands.spawn((rest, Visibility::default())).id();
        commands.entity(root).add_child(joint);
        animated(commands, joint, index, PartKind::Clavicle(side), rest);
        joints.push(joint);
        inverse_bindposes.push(Mat4::from_translation(-body::clavicle_pivot()));
    }
    let surface = object(
        commands,
        Some(root),
        &skin,
        &palette.body,
        Transform::default(),
    );
    let rig = SkinnedMesh {
        inverse_bindposes: bindposes.add(inverse_bindposes),
        joints,
    };
    commands
        .entity(surface)
        .insert((rig.clone(), DynamicSkinnedMeshBounds, BodySkin));
    commands
        .entity(garment_entity)
        .insert((rig, DynamicSkinnedMeshBounds));
}

pub fn setup(
    mut commands: Commands,
    signs: Res<slofox::signs::StudioSigns>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
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
        Transform::from_xyz(0.0, 1.94, -1.60).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    for column in -3_i32..=3 {
        object(
            &mut commands,
            None,
            &geometry.cube,
            &teal,
            Transform::from_xyz(column as f32 * 0.07, 1.94, -1.54).with_scale(Vec3::new(
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

    decor::setup(
        &mut commands,
        &geometry,
        &mut meshes,
        &mut images,
        &mut materials,
        &signs,
    );

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
        bevy::post_process::bloom::Bloom {
            intensity: 0.08,
            ..bevy::post_process::bloom::Bloom::NATURAL
        },
        NoIndirectDrawing,
        Msaa::Sample4,
        Transform::from_xyz(0.0, 2.32, 4.25).looking_at(Vec3::new(0.0, 1.64, 0.0), Vec3::Y),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rig(
        index: usize,
        elapsed: f32,
        speech: f32,
        host: &Transform,
        bounds: &[ArmBounds; 2],
    ) -> [Mat4; 5] {
        let root = host.to_matrix();
        let mut matrices = [root; 5];
        for (hand, side) in [(0, -1.0), (1, 1.0)] {
            let pose = gestures::sample(elapsed, index, side, speech);
            let mut arm =
                Transform::from_translation(body::shoulder(side)).with_rotation(pose.arm_rotation);
            let lift = bounds[hand].keep_above_desk_with_hand(
                host,
                &mut arm,
                &posed_hand_parts(side, pose),
            );
            matrices[hand + 1] = root * arm.to_matrix();
            matrices[hand + 3] = root
                * Transform::from_translation(body::clavicle_pivot())
                    .with_rotation(lift)
                    .to_matrix();
        }
        matrices
    }

    #[test]
    fn heads_have_midpoint_rear_depths_and_partially_covered_browser_ears() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<slofox::signs::StudioSigns>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
            .add_systems(Startup, setup);
        app.update();
        let world = app.world_mut();
        let mut ear_bounds = [[f32::INFINITY, f32::NEG_INFINITY]; 2];
        let mut ear_depths = [0.0; 2];
        let mut ear_count = [0; 2];
        for (transform, parent) in world
            .query_filtered::<(&Transform, &ChildOf), With<Ear>>()
            .iter(world)
        {
            let head = world.get::<AnimatedPart>(parent.parent()).unwrap();
            assert!(matches!(head.kind, PartKind::Head));
            ear_count[head.host] += 1;
            ear_depths[head.host] = transform.translation.z;
            ear_bounds[head.host][0] =
                ear_bounds[head.host][0].min(transform.translation.x - transform.scale.x);
            ear_bounds[head.host][1] =
                ear_bounds[head.host][1].max(transform.translation.x + transform.scale.x);
            if head.host == 0 {
                let skin = sculpt::head(0).profile(transform.translation.y).x;
                assert!(transform.translation.x.abs() - transform.scale.x < skin);
                assert!(transform.translation.x.abs() + transform.scale.x > skin + 0.04);
                assert!((transform.translation.y - 0.265).abs() < 0.00001);
            }
        }
        assert_eq!(ear_count, [2, 2]);
        for (index, bounds) in ear_bounds.iter().enumerate() {
            let mesh = portrait::face(index);
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("missing head positions")
            };
            let mut minimum = Vec3::splat(f32::INFINITY);
            for position in positions {
                minimum = minimum.min(Vec3::from_array(*position));
            }
            let scale = host_scale(index);
            let rear_distance = (ear_depths[index] - minimum.z) * scale.z;
            let midpoint_depth = if index == 0 { 0.2268 } else { 0.25925 };
            let expected_distance = (ear_depths[index] + midpoint_depth) * scale.z;
            assert!(
                (rear_distance - expected_distance).abs() < 0.001,
                "host {index}: rear {rear_distance}, expected {expected_distance}"
            );
            if index == 0 {
                let hair = portrait::hair_cap(index);
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                    hair.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("missing hair positions")
                };
                let hair_edge = positions
                    .iter()
                    .filter(|point| {
                        (0.23..0.30).contains(&point[1]) && (-0.01..0.10).contains(&point[2])
                    })
                    .map(|point| point[0])
                    .fold(0.0_f32, f32::max);
                assert!(hair_edge > 0.224);
                assert!(hair_edge < bounds[1] - 0.010);
                assert!((ear_depths[index] - 0.015).abs() < 0.00001);
            } else {
                assert!((ear_depths[index] + 0.01).abs() < 0.00001);
            }
        }
    }

    #[test]
    fn thumb_halves_share_a_smooth_continuous_curve_and_joint_normals() {
        use bevy::mesh::VertexAttributeValues;

        for side in [-1.0, 1.0] {
            let mut binds = [hand_parts(side)[5], hand_parts(side)[6]];
            for bind in &mut binds {
                bind.translation -= body::wrist(side);
            }
            let meshes = [
                portrait::thumb(side, false, binds[0]),
                portrait::thumb(side, true, binds[1]),
            ];
            let positions: Vec<_> = meshes
                .iter()
                .map(|mesh| {
                    let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        panic!("thumb positions")
                    };
                    positions
                })
                .collect();
            let normals: Vec<_> = meshes
                .iter()
                .map(|mesh| {
                    let Some(VertexAttributeValues::Float32x3(normals)) =
                        mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                    else {
                        panic!("thumb normals")
                    };
                    normals
                })
                .collect();
            for segment in 0..=20 {
                let vertex = 20 * 21 + segment;
                let points: [Vec3; 2] = std::array::from_fn(|half| {
                    binds[half].transform_point(Vec3::from_array(positions[half][vertex]))
                });
                assert!(points[0].distance(points[1]) < 0.000001);
                let normal: [Vec3; 2] = std::array::from_fn(|half| {
                    binds[half]
                        .to_matrix()
                        .inverse()
                        .transpose()
                        .transform_vector3(Vec3::from_array(normals[half][vertex]))
                        .normalize()
                });
                assert!(normal[0].dot(normal[1]) > 0.99999);
            }
        }
    }

    #[test]
    fn skin_and_clothing_share_five_joint_rigs_with_dynamic_bounds() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<slofox::signs::StudioSigns>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
            .add_systems(Startup, setup);
        app.update();
        let world = app.world_mut();
        let mut rigs: [Vec<Entity>; 2] = std::array::from_fn(|_| Vec::new());
        let mut counts = [0; 2];
        for (mesh, skin, parent) in world
            .query_filtered::<(&Mesh3d, &SkinnedMesh, &ChildOf), With<DynamicSkinnedMeshBounds>>()
            .iter(world)
        {
            let host = world.get::<Host>(parent.parent()).unwrap().index;
            assert_eq!(skin.joints.len(), 5);
            if rigs[host].is_empty() {
                rigs[host] = skin.joints.clone();
            } else {
                assert_eq!(rigs[host], skin.joints);
            }
            assert!(
                world
                    .resource::<Assets<Mesh>>()
                    .get(&mesh.0)
                    .unwrap()
                    .skinned_mesh_bounds()
                    .is_some()
            );
            counts[host] += 1;
        }
        assert_eq!(counts, [2, 2]);
    }

    #[test]
    fn hands_have_flatter_palms_and_approximately_palm_length_fingers() {
        for side in [-1.0, 1.0] {
            let parts = hand_parts(side);
            let palm = parts[0];
            assert!(palm.scale.y / palm.scale.x < 0.45);
            for finger in &parts[1..=4] {
                assert!((0.8..=1.05).contains(&(finger.scale.z / palm.scale.z)));
            }
        }
    }

    #[test]
    fn capsule_hand_bounds_enclose_actual_finger_vertices_when_rotated() {
        use bevy::mesh::VertexAttributeValues;

        let finger = portrait::finger(false);
        for side in [-1.0, 1.0] {
            for frame in 0..80 {
                let gesture = gestures::sample(frame as f32 * 0.7, 0, side, 0.8);
                let parts = posed_hand_parts(side, gesture);
                for (index, part) in parts.iter().enumerate().skip(1) {
                    let mut bind = hand_parts(side)[index];
                    bind.translation -= body::wrist(side);
                    let thumb;
                    let mesh = if index <= 4 {
                        &finger
                    } else {
                        thumb = portrait::thumb(side, index == 6, bind);
                        &thumb
                    };
                    let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        panic!("missing finger positions")
                    };
                    let matrix = Mat4::from_quat(gesture.arm_rotation) * part.to_matrix();
                    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                        let center = matrix.w_axis.truncate().dot(axis);
                        let extent = hand_extent(matrix, index, axis, side);
                        for position in positions {
                            let point = matrix.transform_point3(Vec3::from_array(*position));
                            assert!((point.dot(axis) - center).abs() <= extent + 0.000001);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn hands_have_connected_mirrored_thumbs_outside_the_palm() {
        for side in [-1.0, 1.0] {
            let parts = hand_parts(side);
            let palm = parts[0];
            let base = parts[5];
            let tip = parts[6];
            assert!(-side * (tip.translation.x - palm.translation.x) > palm.scale.x);
            assert!(base.translation.z < tip.translation.z);
            assert!(tip.translation.z < parts[1].translation.z);
            let base_start = base.transform_point(-Vec3::Y);
            assert!(
                palm.to_matrix()
                    .inverse()
                    .transform_point3(base_start)
                    .length()
                    < 1.0
            );
            let base_end = base.transform_point(Vec3::Y);
            let tip_start = tip.transform_point(-Vec3::Y);
            assert!(base_end.distance(tip_start) < 0.01);
            let opposite = hand_parts(-side);
            for index in [5, 6] {
                assert!(
                    (parts[index].translation.x + opposite[index].translation.x).abs() < 0.00001
                );
                assert_eq!(
                    parts[index].translation.yz(),
                    opposite[index].translation.yz()
                );
                assert_eq!(parts[index].scale, opposite[index].scale);
            }
        }
    }

    #[test]
    fn lowered_heads_keep_their_neck_overlap_in_the_production_scene() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<slofox::signs::StudioSigns>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
            .add_systems(Startup, setup);
        app.update();
        let world = app.world_mut();
        let mut query = world.query::<(&AnimatedPart, &Transform)>();
        let mut heads = 0;
        for (part, transform) in query.iter(world) {
            if !matches!(part.kind, PartKind::Head) {
                continue;
            }
            assert!((transform.translation.y - 0.715).abs() < 0.00001);
            assert_eq!(*transform, part.rest);
            let neck_top = body::surface(part.host)
                .positions
                .iter()
                .map(|point| point[1])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((neck_top - transform.translation.y - 0.09).abs() < 0.0001);
            heads += 1;
        }
        assert_eq!(heads, 2);
    }

    #[test]
    fn production_body_material_preserves_full_skin_vertex_colors() {
        use bevy::mesh::VertexAttributeValues;

        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<slofox::signs::StudioSigns>()
            .init_resource::<Assets<Image>>()
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
        ), With<BodySkin>>();
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
            assert_eq!(generated.aabbs.len(), 5);
            let (static_min, static_max) = surface.positions.iter().fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(minimum, maximum), point| {
                    let point = Vec3::from_array(*point);
                    (minimum.min(point), maximum.max(point))
                },
            );
            let mut world = World::new();
            let joints: Vec<_> = (0..5)
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
                Mat4::from_translation(-body::clavicle_pivot()),
                Mat4::from_translation(-body::clavicle_pivot()),
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
                    let joints = test_rig(index, elapsed, speech, &host, &arm_bounds);
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
                    let matrices: [Mat4; 5] = std::array::from_fn(|joint| {
                        root.inverse() * joints[joint] * inverse_bindposes[joint]
                    });
                    let minimum = Vec3::from(bounds.min) - Vec3::splat(0.00001);
                    let maximum = Vec3::from(bounds.max) + Vec3::splat(0.00001);
                    for (point, weights) in surface.positions.iter().zip(&surface.weights) {
                        let point = Vec3::from_array(*point);
                        let indices = body::joint_indices(point);
                        let skinned: Vec3 = (0..4)
                            .map(|slot| {
                                matrices[indices[slot] as usize].transform_point3(point)
                                    * weights[slot]
                            })
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
    fn both_scaled_hosts_keep_their_torso_and_shirt_behind_the_desk_while_hands_reach_it() {
        use bevy::mesh::VertexAttributeValues;

        for index in 0..2 {
            let surface = body::surface(index);
            let garment = portrait::garment(index);
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
            let bounds = [arm_bounds(index, -1.0), arm_bounds(index, 1.0)];
            for frame in 0..960 {
                let elapsed = frame as f32 * 0.125;
                for speech_step in 0..=10 {
                    let speech = speech_step as f32 / 10.0;
                    let (rotation, height) = host_motion(elapsed, index, speech);
                    let mut host = host_rest(index).with_rotation(rotation);
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
                            "host {index}: torso intersects desk: time {elapsed}, speech {speech}, corner {point:?}"
                        );
                    }
                    for (joint, side) in [(0, -1.0), (1, 1.0)] {
                        let mut arm = Transform::from_translation(body::shoulder(side))
                            .with_rotation(
                                gestures::sample(elapsed, index, side, speech).arm_rotation,
                            );
                        bounds[joint].keep_above_desk_with_hand(
                            &host,
                            &mut arm,
                            &posed_hand_parts(side, gestures::sample(elapsed, index, side, speech)),
                        );
                        for (part, hand) in
                            posed_hand_parts(side, gestures::sample(elapsed, index, side, speech))
                                .into_iter()
                                .enumerate()
                        {
                            let matrix = host.to_matrix() * arm.to_matrix() * hand.to_matrix();
                            assert!(
                                hand_minimum(matrix, part, Vec3::Z, side)
                                    >= desk::BACK_EDGE_Z + ARM_CLEARANCE,
                                "host {index}: hand no longer reaches desk: time {elapsed}, speech {speech}, side {side}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn posed_hands_clear_the_desk_and_rest_on_it_across_the_gesture_repertoire() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                let bounds = arm_bounds(index, side);
                for frame in 0..480 {
                    let elapsed = frame as f32 * 0.5;
                    for engagement in [0.0, 0.35, 1.0] {
                        let gesture = gestures::sample(elapsed, index, side, engagement);
                        let parts = posed_hand_parts(side, gesture);
                        let (rotation, height) = host_motion(elapsed, index, engagement);
                        let mut host = host_rest(index).with_rotation(rotation);
                        host.translation.y = height;
                        let mut arm = Transform::from_translation(body::shoulder(side))
                            .with_rotation(gesture.arm_rotation);
                        let original = arm.translation;
                        bounds.keep_above_desk_with_hand(&host, &mut arm, &parts);
                        assert!(
                            bounds.posed_lowest_y(&host, &arm, &parts)
                                >= DESK_SURFACE_Y + ARM_CLEARANCE - 0.00001
                        );
                        assert!(
                            (arm.translation - original).length() < 0.08
                                && arm.translation.y - original.y < 0.07,
                            "correction {}: host {index}, side {side}, time {elapsed}, engagement {engagement}",
                            (arm.translation - original).length()
                        );
                        let parent = host.to_matrix() * arm.to_matrix();
                        let lowest_hand = parts
                            .iter()
                            .enumerate()
                            .map(|(index, part)| {
                                let matrix = parent * part.to_matrix();
                                hand_minimum(matrix, index, Vec3::Y, side)
                            })
                            .fold(f32::INFINITY, f32::min);
                        let idle = gestures::sample(elapsed, index, side, 0.0);
                        if gesture.arm_rotation.angle_between(idle.arm_rotation) < 0.001 {
                            assert!(
                                lowest_hand < DESK_SURFACE_Y + 0.025,
                                "resting hand floats: host {index}, side {side}, time {elapsed}, gap {}",
                                lowest_hand - DESK_SURFACE_Y
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn constrained_hand_motion_stays_smooth_through_mode_and_speech_changes() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                let bounds = arm_bounds(index, side);
                let mut state = gestures::GestureState::default();
                let mut previous: Option<Vec3> = None;
                for frame in 0..3600 {
                    let elapsed = frame as f32 / 30.0;
                    let jaw = if ((elapsed / 7.0) as usize).is_multiple_of(2) {
                        0.8
                    } else {
                        0.0
                    };
                    let engagement = state.update(1.0 / 30.0, jaw);
                    let gesture = gestures::sample(elapsed, index, side, engagement);
                    let parts = posed_hand_parts(side, gesture);
                    let (rotation, height) = host_motion(elapsed, index, jaw);
                    let mut host = host_rest(index).with_rotation(rotation);
                    host.translation.y = height;
                    let mut arm = Transform::from_translation(body::shoulder(side))
                        .with_rotation(gesture.arm_rotation);
                    bounds.keep_above_desk_with_hand(&host, &mut arm, &parts);
                    let palm = host.transform_point(arm.transform_point(parts[0].translation));
                    if let Some(previous) = previous {
                        assert!(
                            palm.distance(previous) < 0.025,
                            "hand jumped {}: host {index}, side {side}, time {elapsed}",
                            palm.distance(previous)
                        );
                    }
                    previous = Some(palm);
                }
            }
        }
    }

    #[test]
    fn curled_hands_keep_the_wrist_and_thumb_attached() {
        for side in [-1.0, 1.0] {
            for frame in 0..120 {
                let gesture = gestures::sample(frame as f32 * 0.5, 0, side, 0.7);
                let parts = posed_hand_parts(side, gesture);
                let palm = parts[0];
                let wrist = body::wrist(side);
                assert!(palm.to_matrix().inverse().transform_point3(wrist).length() < 1.0);
                let base_end = parts[5].transform_point(Vec3::Y);
                let tip_start = parts[6].transform_point(-Vec3::Y);
                assert!(base_end.distance(tip_start) < 0.01);
                for finger in &parts[1..=4] {
                    let root = finger.transform_point(-Vec3::Z);
                    assert!(palm.to_matrix().inverse().transform_point3(root).length() < 1.0);
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
                            gestures::sample(elapsed, index, side, speech).arm_rotation,
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
                    let rig = test_rig(index, elapsed, speech, &host, &bounds);
                    let matrices: [Mat4; 5] = std::array::from_fn(|joint| {
                        let bind = match joint {
                            0 => Vec3::ZERO,
                            1 => body::shoulder(-1.0),
                            2 => body::shoulder(1.0),
                            _ => body::clavicle_pivot(),
                        };
                        rig[joint] * Mat4::from_translation(-bind)
                    });
                    for (point, weights) in surface.positions.iter().zip(&surface.weights) {
                        if weights[1] + weights[2] + weights[3] == 0.0 {
                            continue;
                        }
                        let point = Vec3::from_array(*point);
                        let indices = body::joint_indices(point);
                        let skinned: Vec3 = (0..4)
                            .map(|slot| {
                                matrices[indices[slot] as usize].transform_point3(point)
                                    * weights[slot]
                            })
                            .sum();
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
        let host = Transform::from_xyz(0.0, DESK_SURFACE_Y - 0.08, 0.0)
            .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.15, 0.2, -0.12))
            .with_scale(Vec3::new(1.1, 0.9, 1.2));
        let bounds = arm_bounds(0, 1.0);
        let mut arm = Transform::from_xyz(0.29, 0.55, 0.0);
        assert!(bounds.lowest_y(&host, &arm) < DESK_SURFACE_Y);
        bounds.keep_above_desk(&host, &mut arm);
        assert!(
            (bounds.lowest_y(&host, &arm) - DESK_SURFACE_Y - ARM_CLEARANCE).abs() < 0.00001,
            "floor gap {}",
            bounds.lowest_y(&host, &arm) - DESK_SURFACE_Y - ARM_CLEARANCE
        );
        let corrected = arm.translation;
        bounds.keep_above_desk(&host, &mut arm);
        assert!((corrected - arm.translation).length() < 0.00001);
    }

    #[test]
    fn lips_overlap_the_mouth_rim_in_depth_instead_of_floating_in_front() {
        use bevy::mesh::VertexAttributeValues;

        for index in 0..2 {
            let [mouth, upper, lower, _] = mouth_geometry(index);
            for (upper_lip, rest) in [(true, upper), (false, lower)] {
                let mesh = portrait::lip(index, upper_lip);
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("missing lip positions")
                };
                for round_step in 0..=10 {
                    let round = round_step as f32 / 10.0;
                    let rim_depth = mouth.translation.z + round * 0.008;
                    let mut pose = rest;
                    pose.translation.z += round * 0.008;
                    let offset = pose.translation.z - rim_depth;
                    assert!((0.0..0.002).contains(&offset));
                    assert!(pose.translation.z - pose.scale.z < rim_depth);
                    assert!(pose.translation.z + pose.scale.z - rim_depth < 0.006);
                    let mut embedded_vertices = 0;
                    for position in positions {
                        if position[0].abs() < 0.5 && position[2] < -0.75 {
                            let point = pose.transform_point(Vec3::from_array(*position));
                            assert!(point.z < rim_depth);
                            embedded_vertices += 1;
                        }
                    }
                    assert!(embedded_vertices > 10);
                }
            }
        }
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
            assert!(
                (rest.transform_point(Vec3::Y * body::NAVEL_Y).y - DESK_SURFACE_Y).abs() < 0.00001
            );
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
                    .with_rotation(gestures::sample(0.0, index, side, 0.0).arm_rotation);
                bounds.keep_above_desk(&rest, &mut arm);
                let shoulder = rest.transform_point(arm.translation);
                let wrist = rest.transform_point(arm.transform_point(body::wrist(side)));
                assert!(shoulder.y - wrist.y > 0.35);
                assert!((DESK_SURFACE_Y..DESK_SURFACE_Y + 0.12).contains(&wrist.y));
            }
        }
    }

    #[test]
    fn male_host_is_taller_and_broader_without_moving_the_navel_above_the_desk() {
        let female = host_rest(0);
        let male = host_rest(1);
        assert_eq!(female.scale, Vec3::ONE);
        let crown = Vec3::new(0.0, 0.74 - body::NECK_LOWERING + 0.522, 0.0);
        assert!(male.transform_point(crown).y > female.transform_point(crown).y + 0.05);
        let shoulder_span = body::shoulder(1.0) - body::shoulder(-1.0);
        assert!(
            male.to_matrix().transform_vector3(shoulder_span).length()
                > shoulder_span.length() * 1.09
        );
        assert!(male.scale.z > female.scale.z * 1.10);
        assert!((male.transform_point(Vec3::Y * body::NAVEL_Y).y - DESK_SURFACE_Y).abs() < 0.00001);
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
