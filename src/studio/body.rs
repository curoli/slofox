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

pub fn shoulder(side: f32) -> Vec3 {
    Vec3::new(side * 0.29, 0.55, 0.0)
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
        [0.64, 0.200, 0.108],
        [0.70, 0.095, 0.078],
        [0.76, 0.074, 0.075],
        [0.83, 0.082, 0.078],
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
    profile.y * (1.0 - (horizontal / profile.x).powi(2)).max(0.0).sqrt()
}

fn torso_distance(index: usize, point: Vec3) -> f32 {
    let profile = profile(index, point.y);
    let radial = (Vec2::new(point.x / profile.x, point.z / profile.y).length() - 1.0)
        * profile.min_element();
    radial.max(0.34 - point.y).max(point.y - 0.83)
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

fn arm_distance(point: Vec3, side: f32) -> f32 {
    let point = point - shoulder(side);
    let shoulder = Vec3::new(-side * 0.025, 0.005, 0.0);
    let upper = Vec3::new(side * 0.12, -0.055, 0.075);
    let elbow = Vec3::new(side * 0.24, -0.095, 0.16);
    let forearm = Vec3::new(side * 0.15, -0.080, 0.29);
    let wrist = Vec3::new(-side * 0.01, -0.055, 0.46);
    let upper_arm = smooth_union(
        capsule(point, shoulder, upper, 0.073, 0.061),
        capsule(point, upper, elbow, 0.061, 0.047),
        0.018,
    );
    let lower_arm = smooth_union(
        capsule(point, elbow, forearm, 0.047, 0.048),
        capsule(point, forearm, wrist, 0.048, 0.029),
        0.016,
    );
    smooth_union(upper_arm, lower_arm, 0.024)
}

fn distance(index: usize, point: Vec3) -> f32 {
    smooth_union(
        smooth_union(
            torso_distance(index, point),
            arm_distance(point, -1.0),
            0.045,
        ),
        arm_distance(point, 1.0),
        0.045,
    )
}

fn weights(index: usize, point: Vec3) -> [f32; 4] {
    let side = if point.x < 0.0 { -1.0 } else { 1.0 };
    let influence =
        smoothstep((torso_distance(index, point) - arm_distance(point, side) + 0.025) / 0.065);
    if side < 0.0 {
        [1.0 - influence, influence, 0.0, 0.0]
    } else {
        [1.0 - influence, 0.0, influence, 0.0]
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

fn color(index: usize, point: Vec3, weights: [f32; 4]) -> [f32; 4] {
    let mut color = skin_color(index).to_linear();
    if index == 1 {
        let sleeve = smoothstep((weights[1] + weights[2] - 0.2) / 0.6)
            * (1.0 - smoothstep((point.z - 0.14) / 0.06));
        let motif = (point.x * 150.0).sin() * (point.y * 150.0).sin();
        let cloth = if motif > 0.58 {
            Color::srgb_u8(77, 47, 39)
        } else {
            Color::srgb_u8(194, 168, 133)
        }
        .to_linear();
        color = color.mix(&cloth, sleeve);
    }
    [color.red, color.green, color.blue, 1.0]
}

impl Surface {
    fn generate(index: usize) -> Self {
        let origin = Vec3::new(-0.648, 0.300, -0.228);
        let step = 0.012;
        let dimensions = [109, 47, 66];
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
                                let fraction = start_distance / (start_distance - end_distance);
                                let point = start.lerp(end, fraction);
                                let vertex = surface.positions.len() as u32;
                                let weights = weights(index, point);
                                surface.positions.push(point.to_array());
                                surface.normals.push([0.0; 3]);
                                surface.weights.push(weights);
                                surface.joints.push([0, 1, 2, 0]);
                                surface.colors.push(color(index, point, weights));
                                vertex
                            });
                            polygon.push(vertex);
                        }
                        if polygon.len() < 3 {
                            continue;
                        }
                        let center = polygon
                            .iter()
                            .map(|&vertex| Vec3::from_array(surface.positions[vertex as usize]))
                            .sum::<Vec3>()
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
                        polygon.sort_by(|&first, &second| {
                            let first =
                                Vec3::from_array(surface.positions[first as usize]) - center;
                            let second =
                                Vec3::from_array(surface.positions[second as usize]) - center;
                            first
                                .dot(bitangent)
                                .atan2(first.dot(tangent))
                                .total_cmp(&second.dot(bitangent).atan2(second.dot(tangent)))
                        });
                        for triangle in 1..polygon.len() - 1 {
                            surface.indices.extend_from_slice(&[
                                polygon[0],
                                polygon[triangle],
                                polygon[triangle + 1],
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
            if (0.36..0.81).contains(&point.y) {
                *normal = surface_normal(index, point).to_array();
            }
        }
        surface
    }

    pub fn mesh(&self) -> Mesh {
        Mesh::new(
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
        .with_inserted_indices(Indices::U32(self.indices.clone()))
    }
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
    fn neck_and_shoulder_profiles_taper_without_horizontal_steps() {
        for index in 0..2 {
            let mut previous = profile(index, 0.55);
            for step in 1..=1000 {
                let current = profile(index, 0.55 + step as f32 * 0.00021);
                assert!(current.is_finite() && current.min_element() > 0.07);
                assert!(current.x <= previous.x + 0.000001);
                assert!(current.y <= previous.y + 0.000001);
                previous = current;
            }
            for height in [0.59, 0.64, 0.70] {
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
                    distance(index, point).abs() < 0.012,
                    "surface sample {point:?}: {}",
                    distance(index, point)
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
                assert_eq!(weights[3], 0.0);
                let mut restored = point * weights[0];
                for (joint, side) in [(1, -1.0), (2, 1.0)] {
                    let inverse_bindpose = Mat4::from_translation(-shoulder(side));
                    let joint_pose = Mat4::from_translation(shoulder(side));
                    restored +=
                        (joint_pose * inverse_bindpose).transform_point3(point) * weights[joint];
                }
                assert!(restored.distance(point) < 0.000001);
                if point.y > 0.67 {
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
