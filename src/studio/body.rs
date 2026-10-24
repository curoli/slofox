use std::{collections::HashMap, sync::OnceLock};

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

pub struct Surface {
    pub positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
    pub weights: Vec<[f32; 4]>,
    joints: Vec<[u16; 4]>,
    colors: Vec<[f32; 4]>,
}

const SHOULDER_Y: f32 = 0.55;
const SHOULDER_RADIUS: f32 = 0.061;
const INNER_SHOULDER_RADIUS: f32 = 0.140;
pub const NECK_LOWERING: f32 = 0.025;
const NECK_TOP_Y: f32 = 0.83 - NECK_LOWERING;
const CHEST_REFERENCE_Y: f32 = 0.6183;
pub const CHEST_RADIUS_Y: f32 = 0.100;
pub const CHEST_RADIUS_X: f32 = 0.095;
pub const CHEST_CENTER_X: f32 = 0.125;
pub const CHEST_BLEND_RADIUS: f32 = 1.35;
pub const CHEST_CENTER_Y: f32 = CHEST_REFERENCE_Y - 3.0 * CHEST_RADIUS_Y;
pub const NAVEL_Y: f32 = 0.10;

pub fn wrist(side: f32) -> Vec3 {
    Vec3::new(-side * 0.01, -0.38, 0.46)
}

fn elbow(side: f32) -> Vec3 {
    Vec3::new(side * 0.12, -0.40, 0.16)
}

fn arm_start(side: f32) -> Vec3 {
    Vec3::new(side * 0.05, -0.015, 0.0)
}

pub fn shoulder(side: f32) -> Vec3 {
    Vec3::new(side * 0.29, SHOULDER_Y, 0.0)
}

pub fn clavicle_pivot() -> Vec3 {
    Vec3::new(0.0, 0.72 - NECK_LOWERING, 0.0)
}

pub fn joint_indices(point: Vec3) -> [u16; 4] {
    [0, 1, 2, if point.x < 0.0 { 3 } else { 4 }]
}

pub fn skin_color(index: usize) -> Color {
    if index == 0 {
        Color::srgb_u8(202, 145, 103)
    } else {
        Color::srgb_u8(215, 171, 150)
    }
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

pub fn profile(index: usize, height: f32) -> Vec2 {
    let sections = [
        [-0.12, 0.210, 0.125],
        [0.30, 0.270, 0.150],
        [0.48, 0.292, 0.153],
        [0.55, 0.305, 0.145],
        [0.59, 0.285, 0.134],
        [0.64, 0.180, 0.095],
        [0.70 - NECK_LOWERING, 0.088, 0.076],
        [0.76 - NECK_LOWERING, 0.074, 0.075],
        [NECK_TOP_Y, 0.082, 0.078],
    ];
    let interval = sections
        .partition_point(|section| section[0] < height)
        .saturating_sub(1)
        .min(sections.len() - 2);
    let start = sections[interval];
    let end = sections[interval + 1];
    let span = end[0] - start[0];
    let fraction = ((height - start[0]) / span).clamp(0.0, 1.0);
    let squared = fraction * fraction;
    let cubed = squared * fraction;
    let sample = |component| {
        start[component] * (2.0 * cubed - 3.0 * squared + 1.0)
            + end[component] * (-2.0 * cubed + 3.0 * squared)
            + profile_slope(&sections, interval, component)
                * span
                * (cubed - 2.0 * squared + fraction)
            + profile_slope(&sections, interval + 1, component) * span * (cubed - squared)
    };
    let mut profile = Vec2::new(sample(1), sample(2));
    if index == 1 {
        profile *= Vec2::new(1.055, 1.04);
    }
    profile
}

fn profile_slope(sections: &[[f32; 3]], index: usize, component: usize) -> f32 {
    let slope = |start: usize, end: usize| {
        (sections[end][component] - sections[start][component])
            / (sections[end][0] - sections[start][0])
    };
    if index == 0 {
        return slope(0, 1);
    }
    if index == sections.len() - 1 {
        return slope(index - 1, index);
    }
    let before = slope(index - 1, index);
    let after = slope(index, index + 1);
    if before * after <= 0.0 {
        return 0.0;
    }
    let span_before = sections[index][0] - sections[index - 1][0];
    let span_after = sections[index + 1][0] - sections[index][0];
    let weight_before = 2.0 * span_after + span_before;
    let weight_after = span_after + 2.0 * span_before;
    (weight_before + weight_after) / (weight_before / before + weight_after / after)
}

pub fn front(index: usize, horizontal: f32, height: f32) -> f32 {
    let profile = profile(index, height);
    let depth = profile.y * (1.0 - (horizontal / profile.x).powi(2)).max(0.0).sqrt();
    depth + chest_offset(index, horizontal, height, depth)
}

pub fn chest_offset(index: usize, horizontal: f32, height: f32, depth: f32) -> f32 {
    if index != 0
        || depth <= 0.0
        || (height - CHEST_CENTER_Y).abs() >= CHEST_RADIUS_Y * CHEST_BLEND_RADIUS
    {
        return 0.0;
    }
    let profile = profile(index, height);
    let baseline = profile.y * (1.0 - (horizontal / profile.x).powi(2)).max(0.0).sqrt();
    let cap = |center: f32| {
        let radial_squared = ((horizontal - center) / CHEST_RADIUS_X).powi(2)
            + ((height - CHEST_CENTER_Y) / CHEST_RADIUS_Y).powi(2);
        let radius = radial_squared.sqrt();
        if radius >= CHEST_BLEND_RADIUS {
            return 0.0;
        }
        let reference = profile.y * (1.0 - (center / profile.x).powi(2)).max(0.0).sqrt();
        let transition = ((radius - 0.75) / 0.60).clamp(0.0, 1.0);
        let dome = if radius <= 0.75 {
            (1.0 - radial_squared).sqrt()
        } else {
            let height = (1.0_f32 - 0.75_f32.powi(2)).sqrt();
            let slope = -0.75 / height;
            height * (2.0 * transition.powi(3) - 3.0 * transition.powi(2) + 1.0)
                + slope * 0.60 * (transition.powi(3) - 2.0 * transition.powi(2) + transition)
        };
        let rim_blend = 1.0 - smoothstep(transition);
        (reference - baseline) * rim_blend + 0.095 * dome
    };
    (cap(-CHEST_CENTER_X) + cap(CHEST_CENTER_X)) * smoothstep(depth / 0.06)
}

fn torso_distance(index: usize, point: Vec3) -> f32 {
    let profile = profile(index, point.y);
    let depth = point.z - chest_offset(index, point.x, point.y, point.z);
    let radial =
        (Vec2::new(point.x / profile.x, depth / profile.y).length() - 1.0) * profile.min_element();
    radial.max(-0.10 - point.y).max(point.y - NECK_TOP_Y)
}

fn capsule(point: Vec3, start: Vec3, end: Vec3, start_radius: f32, end_radius: f32) -> f32 {
    let direction = end - start;
    let fraction = ((point - start).dot(direction) / direction.length_squared()).clamp(0.0, 1.0);
    point.distance(start + direction * fraction)
        - (start_radius + fraction * (end_radius - start_radius))
}

fn smooth_union(first: f32, second: f32, radius: f32) -> f32 {
    let blend = ((radius - (first - second).abs()) / radius).max(0.0);
    first.min(second) - blend * blend * radius * 0.25
}

fn convex_capsule(point: Vec3, start: Vec3, end: Vec3, start_radius: f32, end_radius: f32) -> f32 {
    let direction = (end - start).normalize();
    let length = end.distance(start);
    let relative = point - start;
    let axial = relative.dot(direction);
    let radial = (relative - direction * axial).length();
    let slope = (end_radius - start_radius) / length;
    let closest = (axial + slope * radial / (1.0 - slope * slope).sqrt()).clamp(0.0, length);
    (relative - direction * closest).length() - (start_radius + slope * closest)
}

fn arm_distance(point: Vec3, side: f32) -> f32 {
    let point = point - shoulder(side);
    let shoulder = arm_start(side);
    let elbow = elbow(side);
    let wrist = wrist(side);
    let upper_arm = capsule(point, shoulder, elbow, SHOULDER_RADIUS, 0.040);
    let lower_arm = capsule(point, elbow, wrist, 0.040, 0.029);
    smooth_union(upper_arm, lower_arm, 0.010)
}

fn body_distance(index: usize, point: Vec3) -> f32 {
    let shoulder_blend = 0.015
        + 0.085
            * smoothstep((point.y - 0.49) / 0.10)
            * (1.0 - smoothstep((point.x.abs() - 0.32) / 0.05));
    smooth_union(
        smooth_union(
            torso_distance(index, point),
            arm_distance(point, -1.0),
            shoulder_blend,
        ),
        arm_distance(point, 1.0),
        shoulder_blend,
    )
}

fn upper_body_distance(index: usize, point: Vec3) -> f32 {
    let original = body_distance(index, point);
    let amount = smoothstep((point.y - 0.42) / 0.10);
    if amount == 0.0 {
        return original;
    }
    let side = if point.x < 0.0 { -1.0 } else { 1.0 };
    let start = Vec3::new(side * 0.10, 0.665 - INNER_SHOULDER_RADIUS, 0.0);
    let end = shoulder(side) + arm_start(side);
    let lower = shoulder(side) + elbow(side);
    let mut convex = convex_capsule(point, start, end, INNER_SHOULDER_RADIUS, SHOULDER_RADIUS)
        .min(convex_capsule(point, end, lower, SHOULDER_RADIUS, 0.040))
        .min(convex_capsule(
            point,
            start,
            lower,
            INNER_SHOULDER_RADIUS,
            0.040,
        ));
    let first = end - start;
    let second = lower - start;
    let inverse_metric = Mat2::from_cols(
        Vec2::new(first.length_squared(), first.dot(second)),
        Vec2::new(first.dot(second), second.length_squared()),
    )
    .inverse();
    let relative = point - start;
    let projected = inverse_metric * Vec2::new(relative.dot(first), relative.dot(second));
    let radius_delta = Vec2::new(
        SHOULDER_RADIUS - INNER_SHOULDER_RADIUS,
        0.040 - INNER_SHOULDER_RADIUS,
    );
    let radius_gradient = inverse_metric * radius_delta;
    let normal_distance = relative.dot(first.cross(second).normalize()).abs();
    let interior = projected
        + radius_gradient * (normal_distance / (1.0 - radius_delta.dot(radius_gradient)).sqrt());
    if interior.min_element() >= 0.0 && interior.element_sum() <= 1.0 {
        convex = convex.min(
            normal_distance * (1.0 - radius_delta.dot(radius_gradient)).sqrt()
                - (INNER_SHOULDER_RADIUS + radius_delta.dot(projected)),
        );
    }
    let neck_width = 0.32 - 0.16 * smoothstep((point.y - 0.50) / 0.09);
    let neck = -smooth_union(
        -torso_distance(index, point),
        neck_width - point.x.abs(),
        0.025,
    );
    let neck_blend = 0.015 + 0.020 * (1.0 - smoothstep((point.y - 0.63) / 0.04));
    let upper_body = smooth_union(convex, neck, neck_blend);
    original + amount * (upper_body - original)
}

fn distance(index: usize, point: Vec3) -> f32 {
    let original = upper_body_distance(index, point);
    let amount = smoothstep((point.z - 0.045) / 0.030)
        * smoothstep((point.y - 0.52) / 0.03)
        * (1.0 - smoothstep((point.y - 0.61) / 0.04))
        * smoothstep((point.x.abs() - 0.10) / 0.06)
        * (1.0 - smoothstep((point.x.abs() - 0.30) / 0.07));
    if amount == 0.0 {
        return original;
    }
    let kernel = [(-0.025, 0.25), (0.0, 0.50), (0.025, 0.25)];
    let mut smoothed = 0.0;
    for (horizontal, horizontal_weight) in kernel {
        for (vertical, vertical_weight) in kernel {
            for (depth, depth_weight) in kernel {
                smoothed +=
                    upper_body_distance(index, point + Vec3::new(horizontal, vertical, depth))
                        * horizontal_weight
                        * vertical_weight
                        * depth_weight;
            }
        }
    }
    original + amount * (smoothed - original)
}

fn weights(index: usize, point: Vec3) -> [f32; 4] {
    let side = if point.x < 0.0 { -1.0 } else { 1.0 };
    let influence =
        smoothstep((torso_distance(index, point) - arm_distance(point, side) + 0.025) / 0.065);
    let clavicle = (1.0 - influence)
        * smoothstep((point.x.abs() - 0.025) / 0.075)
        * smoothstep((point.y - 0.43) / 0.17)
        * (1.0 - smoothstep((point.y - 0.70) / 0.05));
    if side < 0.0 {
        [1.0 - influence - clavicle, influence, 0.0, clavicle]
    } else {
        [1.0 - influence - clavicle, 0.0, influence, clavicle]
    }
}

fn surface_normal(index: usize, point: Vec3) -> Vec3 {
    Vec3::from_array(std::array::from_fn(|axis| {
        let mut delta = Vec3::ZERO;
        delta[axis] = 0.0005;
        distance(index, point + delta) - distance(index, point - delta)
    }))
    .normalize()
}

pub fn shirt_color(point: Vec3) -> Color {
    let patch = ((point.x * 9.0 + (point.y * 8.0).sin() * 0.35).floor() as i32
        + (point.y * 8.0).floor() as i32 * 3)
        .rem_euclid(5);
    let palette = [
        Color::srgb_u8(210, 183, 139),
        Color::srgb_u8(31, 103, 100),
        Color::srgb_u8(143, 53, 51),
        Color::srgb_u8(187, 127, 45),
        Color::srgb_u8(44, 63, 100),
    ];
    let ornament = (point.x * 145.0 + (point.y * 39.0).sin()).sin()
        * (point.y * 140.0 + (point.x * 45.0).cos()).sin();
    if ornament > 0.64 {
        Color::srgb_u8(232, 207, 159)
    } else if ornament < -0.68 {
        Color::srgb_u8(58, 40, 35)
    } else {
        palette[patch as usize]
    }
}

fn sleeve_coverage(point: Vec3) -> f32 {
    let side = if point.x < 0.0 { -1.0 } else { 1.0 };
    let start = shoulder(side) + arm_start(side);
    let direction = elbow(side) - arm_start(side);
    let progress = (point - start).dot(direction) / direction.length_squared();
    1.0 - smoothstep((progress - 0.44) / 0.06)
}

fn color(index: usize, point: Vec3, weights: [f32; 4]) -> [f32; 4] {
    let mut color = skin_color(index).to_linear();
    if index == 1 {
        let profile = profile(index, point.y);
        let front = (point.x / profile.x)
            .atan2(point.z / profile.y)
            .cos()
            .max(0.0);
        let neckline =
            0.61 - 0.09 * front.powi(12) + 0.025 * smoothstep((point.x.abs() - 0.12) / 0.10);
        let torso_coverage = 1.0 - smoothstep((point.y - neckline) / 0.012);
        let arm_weight = weights[1] + weights[2];
        let coverage = torso_coverage * (1.0 - arm_weight) + sleeve_coverage(point) * arm_weight;
        color = color.mix(&shirt_color(point).to_linear(), coverage);
    }
    [color.red, color.green, color.blue, 1.0]
}

impl Surface {
    fn generate(index: usize) -> Self {
        let origin = Vec3::new(-0.648, -0.132, -0.228);
        let step = 0.013;
        let dimensions = [101, 77, 62];
        let grid_index = |horizontal: usize, vertical: usize, depth: usize| {
            horizontal + dimensions[0] * (vertical + dimensions[1] * depth)
        };
        let mut grid = Vec::new();
        for depth in 0..dimensions[2] {
            for vertical in 0..dimensions[1] {
                for horizontal in 0..dimensions[0] {
                    let point =
                        origin + Vec3::new(horizontal as f32, vertical as f32, depth as f32) * step;
                    grid.push((point, distance(index, point)));
                }
            }
        }
        let mut surface = Self {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            weights: Vec::new(),
            joints: Vec::new(),
            colors: Vec::new(),
        };
        let mut edges = HashMap::new();
        for depth in 0..dimensions[2] - 1 {
            for vertical in 0..dimensions[1] - 1 {
                for horizontal in 0..dimensions[0] - 1 {
                    let corners = [
                        grid_index(horizontal, vertical, depth),
                        grid_index(horizontal + 1, vertical, depth),
                        grid_index(horizontal + 1, vertical + 1, depth),
                        grid_index(horizontal, vertical + 1, depth),
                        grid_index(horizontal, vertical, depth + 1),
                        grid_index(horizontal + 1, vertical, depth + 1),
                        grid_index(horizontal + 1, vertical + 1, depth + 1),
                        grid_index(horizontal, vertical + 1, depth + 1),
                    ];
                    if corners.iter().all(|&corner| grid[corner].1 > 0.0)
                        || corners.iter().all(|&corner| grid[corner].1 <= 0.0)
                    {
                        continue;
                    }
                    for tetrahedron in [
                        [0, 5, 1, 6],
                        [0, 1, 2, 6],
                        [0, 2, 3, 6],
                        [0, 3, 7, 6],
                        [0, 7, 4, 6],
                        [0, 4, 5, 6],
                    ] {
                        let mut polygon = Vec::with_capacity(4);
                        for [first, second] in [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]] {
                            let first = corners[tetrahedron[first]];
                            let second = corners[tetrahedron[second]];
                            if (grid[first].1 > 0.0) == (grid[second].1 > 0.0) {
                                continue;
                            }
                            let key = (first.min(second), first.max(second));
                            let vertex = *edges.entry(key).or_insert_with(|| {
                                let (start, start_distance) = grid[key.0];
                                let (end, end_distance) = grid[key.1];
                                let mut fraction = start_distance / (start_distance - end_distance);
                                let mut lower = 0.0;
                                let mut upper = 1.0;
                                for _ in 0..16 {
                                    let sample = start.lerp(end, fraction);
                                    if (distance(index, sample) > 0.0) == (start_distance > 0.0) {
                                        lower = fraction;
                                    } else {
                                        upper = fraction;
                                    }
                                    fraction = (lower + upper) * 0.5;
                                }
                                let point = start.lerp(end, fraction);
                                let vertex = surface.positions.len() as u32;
                                let weights = weights(index, point);
                                surface.positions.push(point.to_array());
                                surface.normals.push([0.0; 3]);
                                surface.weights.push(weights);
                                surface.joints.push(joint_indices(point));
                                surface.colors.push(color(index, point, weights));
                                vertex
                            });
                            let (start, start_distance) = grid[key.0];
                            let (end, end_distance) = grid[key.1];
                            let linear_point =
                                start.lerp(end, start_distance / (start_distance - end_distance));
                            polygon.push((vertex, linear_point));
                        }
                        if polygon.len() < 3 {
                            continue;
                        }
                        let center = polygon.iter().map(|&(_, point)| point).sum::<Vec3>()
                            / polygon.len() as f32;
                        let (origin, origin_distance) = grid[corners[tetrahedron[0]]];
                        let offsets: [Vec3; 3] = std::array::from_fn(|axis| {
                            grid[corners[tetrahedron[axis + 1]]].0 - origin
                        });
                        let differences = Vec3::from_array(std::array::from_fn(|axis| {
                            grid[corners[tetrahedron[axis + 1]]].1 - origin_distance
                        }));
                        let normal = (Mat3::from_cols(offsets[0], offsets[1], offsets[2])
                            .transpose()
                            .inverse()
                            * differences)
                            .normalize();
                        let tangent = normal.any_orthonormal_vector();
                        let bitangent = normal.cross(tangent);
                        polygon.sort_by(|&(_, first), &(_, second)| {
                            let first = first - center;
                            let second = second - center;
                            first
                                .dot(bitangent)
                                .atan2(first.dot(tangent))
                                .total_cmp(&second.dot(bitangent).atan2(second.dot(tangent)))
                        });
                        for triangle in 1..polygon.len() - 1 {
                            surface.indices.extend_from_slice(&[
                                polygon[0].0,
                                polygon[triangle].0,
                                polygon[triangle + 1].0,
                            ]);
                        }
                    }
                }
            }
        }
        let mut mesh = surface.mesh();
        mesh.compute_area_weighted_normals();
        if let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        {
            surface.normals.clone_from(normals);
        }
        for (point, normal) in surface.positions.iter().zip(&mut surface.normals) {
            let point = Vec3::from_array(*point);
            if (-0.08..NECK_TOP_Y - 0.02).contains(&point.y) {
                *normal = surface_normal(index, point).to_array();
            }
        }
        surface
    }

    pub fn mesh(&self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors.clone())
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(self.joints.clone()),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, self.weights.clone())
        .with_inserted_indices(Indices::U32(self.indices.clone()));
        mesh.generate_skinned_mesh_bounds()
            .expect("body mesh must have valid joint indices and weights");
        mesh
    }
}

pub fn skin_garment(index: usize, mesh: &mut Mesh) {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        unreachable!()
    };
    let indices: Vec<_> = positions
        .iter()
        .map(|point| joint_indices(Vec3::from_array(*point)))
        .collect();
    let weights: Vec<_> = positions
        .iter()
        .map(|point| weights(index, Vec3::from_array(*point)))
        .collect();
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(indices),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    mesh.generate_skinned_mesh_bounds()
        .expect("garment skin bounds");
}

pub fn surface(index: usize) -> &'static Surface {
    static SURFACES: [OnceLock<Surface>; 2] = [OnceLock::new(), OnceLock::new()];
    SURFACES[index].get_or_init(|| Surface::generate(index))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    #[test]
    fn chest_sits_below_shoulders_with_full_upper_contour_coverage() {
        let chest_top = CHEST_CENTER_Y + CHEST_RADIUS_Y;
        let chest_bottom = CHEST_CENTER_Y - CHEST_RADIUS_Y;
        let chest_height = chest_top - chest_bottom;
        let shoulder_top = surface(0)
            .positions
            .iter()
            .filter(|point| (0.24..0.34).contains(&point[0].abs()) && point[2].abs() < 0.07)
            .map(|point| point[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let gap = shoulder_top - chest_top;
        assert!(
            (gap - chest_height - 0.010).abs() < 0.001,
            "gap {gap}, chest height {chest_height}"
        );
        let garment = super::super::portrait::garment(0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            garment.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing garment positions")
        };
        for side in [-1.0_f32, 1.0] {
            let neckline = positions
                .iter()
                .filter(|point| (point[0] - side * 0.125).abs() < 0.015 && point[2] > 0.0)
                .map(|point| point[1])
                .fold(f32::NEG_INFINITY, f32::max);
            let chest_edge = CHEST_CENTER_Y + CHEST_RADIUS_Y * CHEST_BLEND_RADIUS;
            assert!(
                (neckline - chest_edge).abs() < 0.002,
                "neckline: {neckline}"
            );
        }
    }

    #[test]
    fn short_shirt_sleeves_leave_the_lower_upper_arm_and_forearm_bare() {
        let skin = skin_color(1).to_linear();
        let expected = [skin.red, skin.green, skin.blue, 1.0];
        for side in [-1.0, 1.0] {
            let start = shoulder(side) + arm_start(side);
            let end = shoulder(side) + elbow(side);
            let weights = if side < 0.0 {
                [0.0, 1.0, 0.0, 0.0]
            } else {
                [0.0, 0.0, 1.0, 0.0]
            };
            for fraction in [0.0, 0.20, 0.40] {
                let point = start.lerp(end, fraction);
                assert_eq!(sleeve_coverage(point), 1.0);
                assert_ne!(color(1, point, weights), expected);
            }
            for point in [start.lerp(end, 0.60), end, shoulder(side) + wrist(side)] {
                assert_eq!(sleeve_coverage(point), 0.0);
                assert_eq!(color(1, point, weights), expected);
            }
        }
        let mut palette = BTreeSet::new();
        for horizontal in -30..=30 {
            for height in 10..=60 {
                let color = shirt_color(Vec3::new(
                    horizontal as f32 * 0.01,
                    height as f32 * 0.01,
                    0.1,
                ))
                .to_srgba();
                palette.insert([
                    (color.red * 255.0) as u8,
                    (color.green * 255.0) as u8,
                    (color.blue * 255.0) as u8,
                ]);
            }
        }
        assert!(palette.len() >= 7);
    }

    #[test]
    fn torso_keeps_its_full_contour_below_the_armpits() {
        for index in 0..2 {
            for step in 0..=80 {
                let height = 0.36 + step as f32 * 0.002;
                let profile = profile(index, height);
                for segment in 0..64 {
                    let angle = segment as f32 / 64.0 * std::f32::consts::TAU;
                    let horizontal = profile.x * angle.cos();
                    let depth = profile.y * angle.sin();
                    let point = Vec3::new(
                        horizontal,
                        height,
                        depth + chest_offset(index, horizontal, height, depth),
                    );
                    assert!(
                        distance(index, point) < 0.00005,
                        "pinched torso: host {index}, point {point:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn neck_root_does_not_swell_beyond_the_tapered_profile() {
        for index in 0..2 {
            for step in 0..=80 {
                let height = 0.70 - NECK_LOWERING + step as f32 * 0.002;
                let profile = profile(index, height);
                for segment in 0..64 {
                    let angle = segment as f32 / 64.0 * std::f32::consts::TAU;
                    let point = Vec3::new(
                        (profile.x + 0.002) * angle.cos(),
                        height,
                        (profile.y + 0.002) * angle.sin(),
                    );
                    assert!(
                        distance(index, point) > 0.0,
                        "swollen neck: host {index}, point {point:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn analytic_shoulders_reduce_surface_normal_variation() {
        let variation = |index, field: fn(usize, Vec3) -> f32| {
            let mut total = 0.0;
            for side in [-1.0, 1.0] {
                for height in [0.50, 0.54, 0.58, 0.60] {
                    let mut previous: Option<Vec3> = None;
                    for step in 0..=150 {
                        let horizontal = side * (0.20 + step as f32 * 0.001);
                        if field(index, Vec3::new(horizontal, height, 0.0)) >= 0.0 {
                            previous = None;
                            continue;
                        }
                        let mut inside = 0.0;
                        let mut outside = 0.20;
                        for _ in 0..20 {
                            let depth = (inside + outside) * 0.5;
                            if field(index, Vec3::new(horizontal, height, depth)) < 0.0 {
                                inside = depth;
                            } else {
                                outside = depth;
                            }
                        }
                        let point = Vec3::new(horizontal, height, (inside + outside) * 0.5);
                        let normal = Vec3::from_array(std::array::from_fn(|axis| {
                            let mut delta = Vec3::ZERO;
                            delta[axis] = 0.0005;
                            field(index, point + delta) - field(index, point - delta)
                        }))
                        .normalize();
                        if let Some(previous) = previous {
                            total += normal.distance_squared(previous);
                        }
                        previous = Some(normal);
                    }
                }
            }
            total
        };
        for index in 0..2 {
            let original = variation(index, body_distance);
            let unfiltered = variation(index, upper_body_distance);
            let smoothed = variation(index, distance);
            assert!(
                smoothed < original * 0.80,
                "host {index}: {smoothed} vs {original}"
            );
            assert!(
                smoothed < unfiltered * 0.90,
                "front smoothing: host {index}: {smoothed} vs {unfiltered}"
            );
        }
    }

    #[test]
    fn inner_shoulder_outline_has_no_step_or_flat_shelf() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                let mut previous_height: Option<f32> = None;
                let mut previous_slope: Option<f32> = None;
                for step in 0..=180 {
                    let horizontal = side * (0.12 + step as f32 * 0.001);
                    let mut below = 0.50;
                    let mut above = 0.80;
                    for _ in 0..24 {
                        let height = (below + above) * 0.5;
                        if distance(index, Vec3::new(horizontal, height, 0.0)) < 0.0 {
                            below = height;
                        } else {
                            above = height;
                        }
                    }
                    let height = (below + above) * 0.5;
                    if let Some(previous) = previous_height {
                        let slope = (height - previous) / 0.001;
                        assert!(
                            (-0.45..=-0.12).contains(&slope),
                            "shoulder step: host {index}, x {horizontal}, slope {slope}"
                        );
                        if let Some(previous) = previous_slope {
                            assert!((slope - previous).abs() < 0.03);
                        }
                        previous_slope = Some(slope);
                    }
                    previous_height = Some(height);
                }
            }
        }
    }

    #[test]
    fn shoulder_tops_fall_outward_with_one_curvature_direction() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                for depth in [-0.04, 0.0, 0.04] {
                    let mut previous = f32::INFINITY;
                    let mut previous_slope = f32::INFINITY;
                    for step in 0..=180 {
                        let horizontal = side * (0.20 + step as f32 * 0.001);
                        let mut below = (0..140)
                            .map(|step| 0.80 - step as f32 * 0.005)
                            .find(|&height| {
                                distance(index, Vec3::new(horizontal, height, depth)) < 0.0
                            })
                            .expect("missing shoulder surface");
                        let mut above = below + 0.005;
                        for _ in 0..20 {
                            let height = (below + above) * 0.5;
                            if distance(index, Vec3::new(horizontal, height, depth)) < 0.0 {
                                below = height;
                            } else {
                                above = height;
                            }
                        }
                        let height = (below + above) * 0.5;
                        assert!(
                            height <= previous + 0.00001,
                            "host {index}, side {side}, depth {depth}, x {horizontal}: {previous} -> {height}"
                        );
                        if previous.is_finite() {
                            let slope = (height - previous) / 0.001;
                            assert!(
                                slope <= previous_slope + 0.0002,
                                "curvature reversal: host {index}, side {side}, depth {depth}, x {horizontal}: {previous_slope} -> {slope}"
                            );
                            previous_slope = slope;
                        }
                        previous = height;
                    }
                }
            }
        }
    }

    #[test]
    fn armpits_open_above_the_upper_arm_midpoint() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                let point = Vec3::new(side * 0.29, 0.42, 0.06);
                assert!(distance(index, point) > 0.0, "host {index}, side {side}");
                let shoulder_join = shoulder(side) + arm_start(side);
                let attachment = Vec3::new(side * 0.30, shoulder_join.y, 0.0);
                assert!(distance(index, attachment) < 0.0);
            }
        }
    }

    #[test]
    fn shoulder_roots_fill_the_upper_torso_depth_without_a_thin_step() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                for height in [0.50, 0.52, 0.54] {
                    for depth in [-0.105, 0.105] {
                        let point = Vec3::new(side * 0.22, height, depth);
                        assert!(
                            distance(index, point) < 0.0,
                            "thin shoulder: host {index}, {point:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn combined_upper_arm_surface_keeps_a_regular_exterior_taper() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                let start = arm_start(side);
                let end = elbow(side);
                let direction = (end - start).normalize();
                let outward = (Vec3::X * side - direction * direction.x * side).normalize();
                let tangent = direction.cross(outward);
                for segment in 0..=16 {
                    let angle = (segment as f32 / 16.0 - 0.5) * std::f32::consts::PI;
                    let radial = outward * angle.cos() + tangent * angle.sin();
                    let mut previous_radius = f32::INFINITY;
                    for step in 5..=85 {
                        let fraction = step as f32 / 100.0;
                        let center = shoulder(side) + start.lerp(end, fraction);
                        let expected = SHOULDER_RADIUS + fraction * (0.040 - SHOULDER_RADIUS);
                        let mut inside = 0.0;
                        let mut outside = 0.15;
                        for _ in 0..20 {
                            let radius = (inside + outside) * 0.5;
                            if distance(index, center + radial * radius) <= 0.0 {
                                inside = radius;
                            } else {
                                outside = radius;
                            }
                        }
                        let radius = (inside + outside) * 0.5;
                        let shoulder_fullness =
                            0.012 * (1.0 - smoothstep((fraction - 0.05) / 0.20));
                        assert!(
                            radius >= expected - 0.002
                                && radius <= expected + 0.002 + shoulder_fullness,
                            "host {index}, side {side}, angle {angle}, fraction {fraction}: {radius} vs {expected}"
                        );
                        assert!(radius <= previous_radius + 0.00001);
                        previous_radius = radius;
                    }
                }
            }
        }
    }

    #[test]
    fn upper_arms_taper_without_a_midpoint_bulge() {
        for side in [-1.0, 1.0] {
            let start = arm_start(side);
            let end = elbow(side);
            let direction = (end - start).normalize();
            let tangent = direction.any_orthonormal_vector();
            let bitangent = direction.cross(tangent);
            let mut previous_radius = f32::INFINITY;
            for step in 10..=85 {
                let fraction = step as f32 / 100.0;
                let center = shoulder(side) + start.lerp(end, fraction);
                let expected_radius = SHOULDER_RADIUS + fraction * (0.040 - SHOULDER_RADIUS);
                let mut largest_radius = 0.0_f32;
                for segment in 0..32 {
                    let angle = segment as f32 / 32.0 * std::f32::consts::TAU;
                    let radial = tangent * angle.cos() + bitangent * angle.sin();
                    let mut inside = 0.0;
                    let mut outside = 0.10;
                    for _ in 0..20 {
                        let radius = (inside + outside) * 0.5;
                        if arm_distance(center + radial * radius, side) <= 0.0 {
                            inside = radius;
                        } else {
                            outside = radius;
                        }
                    }
                    let radius = (inside + outside) * 0.5;
                    assert!((radius - expected_radius).abs() < 0.00001);
                    largest_radius = largest_radius.max(radius);
                }
                assert!(largest_radius <= previous_radius);
                previous_radius = largest_radius;
            }
        }
    }

    #[test]
    fn elbows_do_not_form_a_rounded_joint_bulge() {
        for side in [-1.0, 1.0] {
            let upper = arm_start(side);
            let elbow = elbow(side);
            let wrist = wrist(side);
            let normal = (upper - elbow).cross(wrist - elbow).normalize();
            let center = shoulder(side) + elbow;
            for direction in [-1.0, 1.0] {
                assert!(arm_distance(center + normal * direction * 0.040, side) < 0.0);
                assert!(arm_distance(center + normal * direction * 0.043, side) > 0.0);
            }
        }
    }

    #[test]
    fn chest_caps_have_rounded_hemisphere_profiles_and_smooth_rims() {
        for center in [-0.125, 0.125] {
            for radius in [0.0_f32, 0.25, 0.5, 0.6] {
                for segment in 0..16 {
                    let angle = segment as f32 / 16.0 * std::f32::consts::TAU;
                    let horizontal = center + 0.095 * radius * angle.cos();
                    let height = CHEST_CENTER_Y + 0.100 * radius * angle.sin();
                    let profile = profile(0, height);
                    let reference = profile.y * (1.0 - (center / profile.x).powi(2)).sqrt();
                    let expected = reference + 0.095 * (1.0 - radius.powi(2)).sqrt();
                    assert!((front(0, horizontal, height) - expected).abs() < 0.000001);
                }
            }
            for boundary in [0.75_f32, 1.35] {
                let horizontal = center;
                let height = CHEST_CENTER_Y + 0.100 * boundary;
                let offset = chest_offset(0, horizontal, height, 0.15);
                let before =
                    (offset - chest_offset(0, horizontal, height - 0.00001, 0.15)) / 0.00001;
                let after =
                    (chest_offset(0, horizontal, height + 0.00001, 0.15) - offset) / 0.00001;
                assert!((before - after).abs() < 0.02);
            }
        }
    }

    #[test]
    fn chest_has_two_subtle_front_contours_and_a_central_valley() {
        for height in [
            CHEST_CENTER_Y - 0.05,
            CHEST_CENTER_Y - 0.02,
            CHEST_CENTER_Y + 0.01,
        ] {
            let center = front(0, 0.0, height);
            let left = front(0, -0.125, height);
            let right = front(0, 0.125, height);
            assert!((left - right).abs() < 0.000001);
            assert!(left > center + 0.020);
            assert!(left < 0.240);
            assert!(torso_distance(0, Vec3::new(0.0, height, center)).abs() < 0.000001);
            assert!(torso_distance(0, Vec3::new(0.125, height, right)).abs() < 0.000001);
        }
        let upper_center = front(0, 0.0, CHEST_CENTER_Y + 0.04);
        let upper_contour = front(0, 0.125, CHEST_CENTER_Y + 0.04);
        assert!(upper_contour > upper_center + 0.025);
        assert!(upper_contour > 0.205 && upper_contour < 0.225);
        let (peak_position, peak_depth) = (1..=190)
            .map(|step| {
                let horizontal = step as f32 / 1000.0;
                (horizontal, front(0, horizontal, CHEST_CENTER_Y))
            })
            .max_by(|first, second| first.1.total_cmp(&second.1))
            .unwrap();
        assert!((0.110..0.135).contains(&peak_position));
        assert!((0.225..0.235).contains(&peak_depth));
        let peak_height = (-70..=70)
            .map(|step| CHEST_CENTER_Y + step as f32 / 1000.0)
            .max_by(|first, second| front(0, 0.125, *first).total_cmp(&front(0, 0.125, *second)))
            .unwrap();
        assert!((peak_height - CHEST_CENTER_Y).abs() < 0.015);
        for horizontal in [-0.2, -0.11, 0.0, 0.11, 0.2] {
            assert_eq!(chest_offset(0, horizontal, CHEST_CENTER_Y, -0.15), 0.0);
            assert_eq!(chest_offset(0, horizontal, 0.70, 0.15), 0.0);
            assert_eq!(chest_offset(1, horizontal, CHEST_CENTER_Y, 0.15), 0.0);
        }
        let garment = super::super::portrait::garment(0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            garment.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing garment positions")
        };
        for position in positions {
            let point = Vec3::from_array(*position);
            if point.y > -0.09 && point.z > 0.0 {
                assert!(
                    torso_distance(0, point) > 0.0,
                    "garment cuts into chest: {point:?}"
                );
            }
        }
    }

    #[test]
    fn forearms_taper_from_the_single_elbow_to_the_wrist_without_an_extra_joint() {
        for side in [-1.0, 1.0] {
            let elbow = elbow(side);
            let wrist = wrist(side);
            let direction = (wrist - elbow).normalize();
            let tangent = direction.any_orthonormal_vector();
            let bitangent = direction.cross(tangent);
            let mut previous_radius = f32::INFINITY;
            for step in 30..=90 {
                let fraction = step as f32 / 100.0;
                let center = shoulder(side) + elbow.lerp(wrist, fraction);
                let expected_radius = 0.040 + fraction * (0.029 - 0.040);
                let mut largest_radius = 0.0_f32;
                for segment in 0..32 {
                    let angle = segment as f32 / 32.0 * std::f32::consts::TAU;
                    let radial = tangent * angle.cos() + bitangent * angle.sin();
                    let mut inside = 0.0;
                    let mut outside = 0.08;
                    for _ in 0..20 {
                        let radius = (inside + outside) * 0.5;
                        if arm_distance(center + radial * radius, side) <= 0.0 {
                            inside = radius;
                        } else {
                            outside = radius;
                        }
                    }
                    let radius = (inside + outside) * 0.5;
                    assert!(
                        (radius - expected_radius).abs() < 0.00001,
                        "side {side}, fraction {fraction}, angle {angle}: {radius} vs {expected_radius}"
                    );
                    largest_radius = largest_radius.max(radius);
                }
                assert!(largest_radius <= previous_radius);
                previous_radius = largest_radius;
            }
        }
    }

    #[test]
    fn neck_and_shoulder_profiles_taper_without_horizontal_steps() {
        for index in 0..2 {
            let mut previous = profile(index, 0.55);
            for step in 1..=1000 {
                let current = profile(index, 0.55 + step as f32 * (0.21 - NECK_LOWERING) / 1000.0);
                assert!(current.is_finite() && current.min_element() > 0.07);
                assert!(current.x <= previous.x + 0.000001);
                assert!(current.y <= previous.y + 0.000001);
                previous = current;
            }
            for height in [0.59, 0.64, 0.70 - NECK_LOWERING] {
                let center = profile(index, height);
                let before = (center - profile(index, height - 0.0001)) / 0.0001;
                let after = (profile(index, height + 0.0001) - center) / 0.0001;
                assert!((before - after).abs().max_element() < 0.08);
                assert!(before.x < -0.05 && after.x < -0.05);
            }
        }
    }

    #[test]
    fn upper_bodies_are_closed_connected_surfaces_with_outward_normals() {
        for index in 0..2 {
            let surface = surface(index);
            assert!(
                surface.positions.len() < 50_000,
                "{} vertices",
                surface.positions.len()
            );
            let mut edges = BTreeMap::new();
            let mut neighbors = vec![Vec::new(); surface.positions.len()];
            for triangle in surface.indices.as_chunks::<3>().0 {
                let points =
                    triangle.map(|vertex| Vec3::from_array(surface.positions[vertex as usize]));
                let cross = (points[1] - points[0]).cross(points[2] - points[0]);
                let average_normal: Vec3 = triangle
                    .iter()
                    .map(|&vertex| Vec3::from_array(surface.normals[vertex as usize]))
                    .sum();
                assert!(
                    cross.dot(average_normal) >= -0.00000001,
                    "inward triangle: {points:?}, normals {average_normal:?}, cross {cross:?}"
                );
                for (start, end) in [
                    (triangle[0], triangle[1]),
                    (triangle[1], triangle[2]),
                    (triangle[2], triangle[0]),
                ] {
                    let entry = edges
                        .entry((start.min(end), start.max(end)))
                        .or_insert((0, 0));
                    entry.0 += 1;
                    entry.1 += if start < end { 1 } else { -1 };
                    neighbors[start as usize].push(end);
                    neighbors[end as usize].push(start);
                }
            }
            let invalid: Vec<_> = edges
                .iter()
                .filter(|entry| entry.1.0 != 2 || entry.1.1 != 0)
                .take(5)
                .collect();
            assert!(invalid.is_empty(), "invalid edges: {invalid:?}");
            let mut visited = BTreeSet::new();
            let mut pending = vec![0];
            while let Some(vertex) = pending.pop() {
                if visited.insert(vertex) {
                    pending.extend(&neighbors[vertex as usize]);
                }
            }
            assert_eq!(visited.len(), surface.positions.len());
            for (point, vertex_normal) in surface.positions.iter().zip(&surface.normals) {
                let point = Vec3::from_array(*point);
                let vertex_normal = Vec3::from_array(*vertex_normal);
                assert!(point.is_finite() && point.abs().max_element() < 0.85);
                assert!(
                    vertex_normal.is_finite() && (vertex_normal.length() - 1.0).abs() < 0.00001
                );
                assert!(
                    distance(index, point).abs() < 0.0005,
                    "surface sample {point:?}: {}",
                    distance(index, point)
                );
            }
        }
    }

    #[test]
    fn clavicle_lift_moves_the_whole_shoulder_edge_without_a_ridge() {
        for index in 0..2 {
            for side in [-1.0, 1.0] {
                let rotation = Quat::from_rotation_z(side * 0.18);
                let pivot = clavicle_pivot();
                let mut previous_height = f32::INFINITY;
                let mut previous_lift = 0.0;
                for step in 0..=100 {
                    let horizontal = side * (0.12 + step as f32 * 0.0018);
                    let mut below = 0.50;
                    let mut above = 0.80;
                    for _ in 0..24 {
                        let middle = (below + above) * 0.5;
                        if distance(index, Vec3::new(horizontal, middle, 0.0)) < 0.0 {
                            below = middle;
                        } else {
                            above = middle;
                        }
                    }
                    let point = Vec3::new(horizontal, above, 0.0);
                    let weights = weights(index, point);
                    let moved = point * weights[0]
                        + (pivot + rotation * (point - pivot)) * (1.0 - weights[0]);
                    let lift = moved.y - point.y;
                    assert!(
                        lift > 0.001,
                        "inner shoulder did not lift: host {index}, x {horizontal}"
                    );
                    assert!(lift >= previous_lift - 0.0001);
                    assert!(
                        moved.y <= previous_height + 0.0001,
                        "shoulder ridge: host {index}, x {horizontal}"
                    );
                    previous_height = moved.y;
                    previous_lift = lift;
                }
                assert!(previous_lift > 0.045);
                assert_eq!(
                    weights(index, Vec3::new(0.0, 0.76, 0.0)),
                    [1.0, 0.0, 0.0, 0.0]
                );
                assert_eq!(
                    weights(index, Vec3::new(0.0, 0.32, 0.15)),
                    [1.0, 0.0, 0.0, 0.0]
                );
            }
        }
    }

    #[test]
    fn skin_weights_preserve_the_bind_pose_and_blend_only_at_shoulders() {
        for index in 0..2 {
            let surface = surface(index);
            let mut blended = 0;
            for (point, weights) in surface.positions.iter().zip(&surface.weights) {
                let point = Vec3::from_array(*point);
                assert!(weights.iter().all(|weight| (0.0..=1.0).contains(weight)));
                assert!((weights.iter().sum::<f32>() - 1.0).abs() < 0.000001);
                let mut restored = point * (weights[0] + weights[3]);
                for (joint, side) in [(1, -1.0), (2, 1.0)] {
                    let inverse_bindpose = Mat4::from_translation(-shoulder(side));
                    let joint_pose = Mat4::from_translation(shoulder(side));
                    restored +=
                        (joint_pose * inverse_bindpose).transform_point3(point) * weights[joint];
                }
                assert!(restored.distance(point) < 0.000001);
                if point.y > 0.75 {
                    assert_eq!(weights[0], 1.0);
                }
                if point.z > 0.35 {
                    assert_eq!(weights[0], 0.0);
                }
                if weights[0] > 0.0 && weights[0] < 1.0 {
                    blended += 1;
                }
            }
            assert!(blended > 100);
        }
    }
}
