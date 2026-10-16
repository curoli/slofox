use std::f32::consts::PI;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

#[derive(Clone, Copy)]
pub struct Portrait {
    pub eye_width: f32,
    pub eye_height: f32,
    pub eye_y: f32,
    pub eye_spacing: f32,
    pub nose_width: f32,
    pub mouth_width: f32,
    pub mouth_y: f32,
    pub hair_length: f32,
    pub hair_part: f32,
    pub hair_width: f32,
}

impl Portrait {
    pub fn for_host(index: usize) -> Self {
        if index == 0 {
            Self {
                eye_width: 0.043,
                eye_height: 0.013,
                eye_y: 0.282,
                eye_spacing: 0.066,
                nose_width: 0.029,
                mouth_width: 0.080,
                mouth_y: 0.153,
                hair_length: 0.83,
                hair_part: 0.022,
                hair_width: 0.205,
            }
        } else {
            Self {
                eye_width: 0.037,
                eye_height: 0.010,
                eye_y: 0.285,
                eye_spacing: 0.071,
                nose_width: 0.039,
                mouth_width: 0.068,
                mouth_y: 0.158,
                hair_length: 0.60,
                hair_part: -0.052,
                hair_width: 0.238,
            }
        }
    }
}

fn indexed_surface(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices));
    mesh.compute_area_weighted_normals();
    mesh
}

fn connect_ring(indices: &mut Vec<u32>, ring: usize, segment: usize, segments: usize) {
    let current = (ring * (segments + 1) + segment) as u32;
    let next = current + segments as u32 + 1;
    indices.extend_from_slice(&[current, current + 1, next, current + 1, next + 1, next]);
}

fn contour_sample(index: usize, height: f32) -> Vec2 {
    super::sculpt::head(index).profile(height)
}

pub fn face_front(index: usize, horizontal: f32, height: f32) -> f32 {
    super::sculpt::head(index).front(horizontal, height)
}

pub fn face(index: usize) -> Mesh {
    let (bottom, top) = super::sculpt::head(index).heights();
    let rings = 64;
    let segments = 72;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        let height = bottom + ring as f32 / rings as f32 * (top - bottom);
        let profile = contour_sample(index, height);
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * PI * 2.0;
            let front = angle.cos();
            let horizontal = profile.x * angle.sin();
            let depth = if front > 0.0 {
                face_front(index, horizontal, height)
            } else {
                profile.y * front
            };
            positions.push([horizontal, height, depth]);
            if ring < rings && segment < segments {
                connect_ring(&mut indices, ring, segment, segments);
            }
        }
    }
    for (ring, height) in [(0, bottom), (rings, top)] {
        let center = positions.len() as u32;
        positions.push([0.0, height, 0.0]);
        for segment in 0..segments {
            let current = (ring * (segments + 1) + segment) as u32;
            let next = current + 1;
            indices.extend(if ring == 0 {
                [center, next, current]
            } else {
                [center, current, next]
            });
        }
    }
    let mut mesh = indexed_surface(positions, indices);
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(normals)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
    {
        for ring in 0..=rings {
            let first = ring * (segments + 1);
            let last = first + segments;
            let normal = (Vec3::from_array(normals[first]) + Vec3::from_array(normals[last]))
                .normalize()
                .to_array();
            normals[first] = normal;
            normals[last] = normal;
        }
    }
    mesh
}

pub fn eye() -> Mesh {
    let segments = 32;
    let rows = 8;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for row in 0..=rows {
        let vertical = row as f32 / rows as f32 * 2.0 - 1.0;
        for segment in 0..=segments {
            let horizontal = segment as f32 / segments as f32 * 2.0 - 1.0;
            let height = (1.0 - horizontal.powi(2)).max(0.002).powf(0.70) * vertical;
            let depth = (1.0 - horizontal.powi(2) * 0.70 - height.powi(2) * 0.30)
                .max(0.05)
                .sqrt();
            positions.push([horizontal, height, depth]);
            if row < rows && segment < segments {
                connect_ring(&mut indices, row, segment, segments);
            }
        }
    }
    indexed_surface(positions, indices)
}

pub fn eyelid(index: usize, upper: bool) -> Mesh {
    let portrait = Portrait::for_host(index);
    let segments = 32;
    let side = if upper { 1.0 } else { -1.0 };
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for row in 0..=1 {
        for segment in 0..=segments {
            let horizontal = segment as f32 / segments as f32 * 2.0 - 1.0;
            let arch = (1.0 - horizontal.powi(2)).max(0.002).powf(0.70);
            positions.push([
                horizontal * portrait.eye_width,
                side * (arch * portrait.eye_height + row as f32 * 0.003),
                0.009 * (1.0 - horizontal.powi(2) * 0.70 - arch.powi(2) * 0.30).sqrt() + 0.001
                    - row as f32 * 0.002,
            ]);
            if row == 0 && segment < segments {
                connect_ring(&mut indices, row, segment, segments);
            }
        }
    }
    if !upper {
        for triangle in indices.as_chunks_mut::<3>().0 {
            triangle.swap(1, 2);
        }
    }
    indexed_surface(positions, indices)
}

pub fn hair_cap(index: usize) -> Mesh {
    let portrait = Portrait::for_host(index);
    let rings = 18;
    let segments = 64;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * PI * 2.0;
            let front = angle.cos().max(0.0);
            let transition = ((front - 0.10) / 0.75).clamp(0.0, 1.0);
            let transition = transition * transition * (3.0 - 2.0 * transition);
            let mut height =
                0.19 + if index == 0 { 0.205 } else { 0.235 } * transition + 0.020 * front.powi(6);
            if index == 1 {
                height += 0.025 * (-((front - 0.65) / 0.20).powi(2)).exp();
            }
            let hairline = ((height - 0.264) / 0.280_f32).acos();
            let polar = (ring as f32 / rings as f32 * hairline).max(0.002);
            let sweep = if index == 0 { 0.008 } else { 0.018 };
            positions.push([
                portrait.hair_width * polar.sin() * angle.sin()
                    + portrait.hair_part * 0.12 * polar.cos(),
                0.264 + 0.280 * polar.cos() + sweep * angle.sin() * polar.sin(),
                0.207 * polar.sin() * angle.cos(),
            ]);
            if ring < rings && segment < segments {
                connect_ring(&mut indices, ring, segment, segments);
            }
        }
    }
    for triangle in indices.as_chunks_mut::<3>().0 {
        triangle.swap(1, 2);
    }
    indexed_surface(positions, indices)
}

pub fn hair_lock(index: usize, side: f32, lock: usize, highlight: bool) -> Mesh {
    let portrait = Portrait::for_host(index);
    let rings = 22;
    let segments = 12;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        let progress = ring as f32 / rings as f32;
        let bend = (progress * PI).sin();
        let wave = if index == 0 {
            0.008 * (progress * PI * 2.0 + lock as f32).sin()
        } else {
            0.016 * (progress * PI * 2.0 + lock as f32 * 0.7).sin()
        };
        let horizontal = side * (0.157 + bend * 0.045 + lock as f32 * 0.011 + wave);
        let vertical = 0.455 - progress * portrait.hair_length;
        let drape = if index == 0 && side < 0.0 {
            progress * 0.13
        } else {
            0.0
        };
        let front = 0.048 - lock as f32 * 0.037 + bend * 0.008 + drape
            - if index == 1 {
                0.035 + progress * 0.04
            } else {
                0.0
            };
        let taper = (1.0 - progress).powf(0.30).max(0.06);
        let width = if highlight {
            0.003
        } else if index == 1 {
            0.024
        } else {
            0.031
        } * taper;
        let depth = if highlight { 0.003 } else { 0.026 } * taper;
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * PI * 2.0;
            positions.push([
                horizontal + width * angle.sin(),
                vertical,
                front + depth * angle.cos() + if highlight { 0.026 * taper } else { 0.0 },
            ]);
            if ring < rings && segment < segments {
                connect_ring(&mut indices, ring, segment, segments);
            }
        }
    }
    for triangle in indices.as_chunks_mut::<3>().0 {
        triangle.swap(1, 2);
    }
    indexed_surface(positions, indices)
}

pub fn crown_strand(index: usize, side: f32, strand: usize) -> Mesh {
    let portrait = Portrait::for_host(index);
    let sweep = if index == 0 { 0.008 } else { 0.018 };
    let segments = 24;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for step in 0..=segments {
        let progress = step as f32 / segments as f32;
        let horizontal = portrait.hair_part * (1.0 - progress) + side * 0.173 * progress;
        let depth = 0.025 + strand as f32 * 0.021 + progress * 0.012;
        for edge in [-1.0, 1.0] {
            let horizontal = horizontal + edge * 0.0020;
            let unit_z = depth / 0.207;
            let mut unit_x = horizontal / portrait.hair_width;
            let mut unit_y = (1.0 - unit_x.powi(2) - unit_z.powi(2)).max(0.001).sqrt();
            for _ in 0..3 {
                unit_x = (horizontal - portrait.hair_part * 0.12 * unit_y) / portrait.hair_width;
                unit_y = (1.0 - unit_x.powi(2) - unit_z.powi(2)).max(0.001).sqrt();
            }
            let normal = Vec3::new(unit_x / 0.205, unit_y / 0.280, unit_z / 0.207).normalize();
            let point = Vec3::new(horizontal, 0.264 + 0.280 * unit_y + sweep * unit_x, depth)
                + normal * 0.0020;
            positions.push(point.to_array());
            normals.push(normal.to_array());
        }
        if step < segments {
            let base = step as u32 * 2;
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }
    }
    for triangle in indices.as_chunks_mut::<3>().0 {
        let first = Vec3::from_array(positions[triangle[0] as usize]);
        let second = Vec3::from_array(positions[triangle[1] as usize]);
        let third = Vec3::from_array(positions[triangle[2] as usize]);
        if (second - first)
            .cross(third - first)
            .dot(Vec3::from_array(normals[triangle[0] as usize]))
            < 0.0
        {
            triangle.swap(1, 2);
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

pub const TOOTH_TOP: f32 = 0.91 * (1.0 - 0.12 * 0.1175);

pub fn teeth() -> Mesh {
    let outline = [
        Vec2::new(-0.099, 0.91),
        Vec2::new(0.099, 0.91),
        Vec2::new(0.115, 0.76),
        Vec2::new(0.115, -0.76),
        Vec2::new(0.099, -0.91),
        Vec2::new(-0.099, -0.91),
        Vec2::new(-0.115, -0.76),
        Vec2::new(-0.115, 0.76),
    ];
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for tooth in 0..8 {
        let center = (tooth as f32 - 3.5) * 0.235;
        let height = 1.0 - 0.12 * center.abs();
        let depth = 0.14 * (1.0 - center.powi(2));
        let front = positions.len() as u32;
        positions.push([center, 0.0, depth + 0.74]);
        let back = front + 1;
        positions.push([center, 0.0, depth - 0.20]);
        for point in outline {
            positions.push([center + point.x, point.y * height, depth + 0.58]);
            positions.push([center + point.x, point.y * height, depth - 0.20]);
        }
        for corner in 0..8 {
            let current = front + 2 + corner * 2;
            let next = front + 2 + (corner + 1) % 8 * 2;
            indices.extend_from_slice(&[
                front,
                next,
                current,
                back,
                current + 1,
                next + 1,
                current,
                next,
                current + 1,
                current + 1,
                next,
                next + 1,
            ]);
        }
    }
    indexed_surface(positions, indices)
}

pub fn lip(index: usize, upper: bool) -> Mesh {
    let mut mesh = if upper {
        Sphere::new(1.0).mesh().uv(32, 16)
    } else {
        smile(index)
    };
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for position in positions {
            let horizontal = position[0];
            if upper {
                position[1] += 0.25 * ((1.0 - horizontal.powi(2)).max(0.0).sqrt() - 1.0);
                position[1] += 0.24 * (-((horizontal.abs() - 0.25) / 0.18).powi(2)).exp();
            } else {
                position[1] -= 0.12 * (1.0 - horizontal.powi(2));
            }
        }
    }
    mesh.compute_area_weighted_normals();
    mesh
}

pub fn garment(index: usize) -> Mesh {
    let rings = 48;
    let segments = 96;
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        let progress = ring as f32 / rings as f32;
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * PI * 2.0;
            let front = angle.cos().max(0.0);
            let neckline = if index == 0 {
                super::body::CHEST_CENTER_Y + 0.025 * angle.sin().abs()
            } else {
                0.61 - 0.09 * front.powi(12)
            };
            let height = -0.10 + progress * (neckline + 0.10);
            let profile = super::body::profile(index, height);
            let width = profile.x + 0.004;
            let depth = profile.y + 0.004;
            let horizontal = width * angle.sin();
            let depth = depth * angle.cos();
            let depth = depth + super::body::chest_offset(index, horizontal, height, depth);
            positions.push([horizontal, height, depth]);
            let color = if index == 0 {
                let motif = (horizontal * 115.0 + (height * 37.0).sin() * 2.8).sin()
                    * (height * 95.0 + (horizontal * 40.0).cos() * 2.0).cos();
                if motif > 0.10 {
                    Color::srgb_u8(177, 91, 29)
                } else {
                    Color::srgb_u8(20, 36, 46)
                }
            } else {
                super::body::shirt_color(Vec3::new(horizontal, height, depth))
            };
            let linear = color.to_linear();
            colors.push([linear.red, linear.green, linear.blue, 1.0]);
            if ring < rings && segment < segments {
                connect_ring(&mut indices, ring, segment, segments);
            }
        }
    }
    indexed_surface(positions, indices).with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
}

pub fn collar(side: f32) -> Mesh {
    let corners = [
        Vec2::new(side * 0.028, 0.635),
        Vec2::new(side * 0.112, 0.600),
        Vec2::new(side * 0.084, 0.510),
        Vec2::new(side * 0.052, 0.551),
    ];
    let segments = 8;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for row in 0..=segments {
        let progress = row as f32 / segments as f32;
        let inner = corners[0].lerp(corners[3], progress);
        let outer = corners[1].lerp(corners[2], progress);
        for column in 0..=segments {
            let point = inner.lerp(outer, column as f32 / segments as f32);
            positions.push([
                point.x,
                point.y,
                super::body::front(1, point.x, point.y) + 0.010,
            ]);
            if row < segments && column < segments {
                connect_ring(&mut indices, row, column, segments);
            }
        }
    }
    if side > 0.0 {
        for triangle in indices.as_chunks_mut::<3>().0 {
            triangle.swap(1, 2);
        }
    }
    indexed_surface(positions, indices)
}

pub fn smile(index: usize) -> Mesh {
    let mut mesh = Sphere::new(1.0).mesh().uv(32, 16);
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for position in positions {
            position[1] += position[0].powi(2) * if index == 0 { 1.4 } else { 0.65 };
        }
    }
    mesh.compute_area_weighted_normals();
    mesh
}

pub fn tooth_visibility(index: usize, jaw: f32) -> f32 {
    if index == 0 {
        0.60 * jaw.clamp(0.0, 1.0)
    } else {
        jaw.clamp(0.0, 0.60)
    }
}

#[cfg(test)]
mod tests {
    use bevy::mesh::VertexAttributeValues;

    use super::*;

    #[test]
    fn shirt_collar_faces_follow_the_torso_without_skin_poking_through() {
        for side in [-1.0, 1.0] {
            let mesh = collar(side);
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("missing collar positions")
            };
            let Some(Indices::U32(indices)) = mesh.indices() else {
                panic!("missing collar triangles")
            };
            for triangle in indices.as_chunks::<3>().0 {
                let center = triangle
                    .iter()
                    .map(|&index| Vec3::from_array(positions[index as usize]))
                    .sum::<Vec3>()
                    / 3.0;
                assert!(center.z > super::super::body::front(1, center.x, center.y) + 0.005);
            }
        }
    }

    #[test]
    fn microphone_hair_cap_covers_the_widened_forehead() {
        let mesh = hair_cap(1);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing hair positions")
        };
        let mut checked = 0;
        for &[horizontal, height, depth] in positions {
            if (0.36..0.52).contains(&height)
                && depth >= 0.0
                && horizontal.abs() < contour_sample(1, height).x
            {
                assert!(
                    depth + 0.002 >= face_front(1, horizontal, height),
                    "forehead pokes through hair at {horizontal}, {height}, {depth}"
                );
                checked += 1;
            }
        }
        assert!(checked > 100);
    }

    #[test]
    fn microphone_host_has_a_broader_jaw_forehead_and_hooded_eyes() {
        let head = super::super::sculpt::head(1);
        assert!(head.profile(0.095).x > 0.150);
        assert!(head.profile(0.397).x > 0.190);
        assert!(head.front(0.0, 0.216) > 0.230);
        let male = Portrait::for_host(1);
        let female = Portrait::for_host(0);
        assert!(male.eye_height / male.eye_width < female.eye_height / female.eye_width);
        assert!(male.hair_length < female.hair_length);
    }

    #[test]
    fn portrait_meshes_are_finite_indexed_and_bounded() {
        for index in 0..2 {
            let meshes = [
                face(index),
                eyelid(index, true),
                eyelid(index, false),
                hair_cap(index),
                hair_lock(index, -1.0, 0, false),
                hair_lock(index, 1.0, 4, true),
                garment(index),
                collar(-1.0),
                collar(1.0),
            ];
            for mesh in meshes {
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("missing positions");
                };
                let Some(VertexAttributeValues::Float32x3(normals)) =
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                else {
                    panic!("missing normals");
                };
                assert_eq!(positions.len(), normals.len());
                assert!(positions.iter().flatten().all(|value| value.is_finite()));
                assert!(normals.iter().flatten().all(|value| value.is_finite()));
                assert!(
                    normals
                        .iter()
                        .all(|normal| Vec3::from_array(*normal).length() > 0.99)
                );
                assert!(positions.iter().flatten().all(|value| value.abs() < 0.8));
                assert!(
                    mesh.indices()
                        .unwrap()
                        .iter()
                        .all(|value| value < positions.len())
                );
                assert!(mesh.count_vertices() < 5000);
            }
        }
    }

    #[test]
    fn visible_surfaces_have_outward_normals() {
        for index in 0..2 {
            for mesh in [face(index), hair_cap(index), garment(index)] {
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("missing positions");
                };
                let Some(VertexAttributeValues::Float32x3(normals)) =
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                else {
                    panic!("missing normals");
                };
                let mut checked = 0;
                for (position, normal) in positions.iter().zip(normals) {
                    if position[0].abs() < 0.015 && position[2] > 0.08 {
                        assert!(
                            normal[2] > 0.0,
                            "inward front surface: {position:?} {normal:?}"
                        );
                        checked += 1;
                    }
                }
                assert!(checked > 5);
            }
        }
        for side in [-1.0, 1.0] {
            let mesh = collar(side);
            let Some(VertexAttributeValues::Float32x3(normals)) =
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            else {
                panic!("missing normals");
            };
            assert!(normals.iter().all(|normal| normal[2] > 0.0));
        }
    }

    #[test]
    fn almond_eye_has_tapered_corners_and_outward_normals() {
        let mesh = eye();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing positions");
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("missing normals");
        };
        assert_eq!(positions.len(), normals.len());
        assert!(normals.iter().all(|normal| normal[2] > 0.0));
        assert!(positions.iter().flatten().all(|value| value.is_finite()));
        assert!(
            positions
                .iter()
                .filter(|position| position[0].abs() > 0.99)
                .all(|position| position[1].abs() < 0.02)
        );
        assert!(
            positions
                .iter()
                .any(|position| position[0].abs() < 0.01 && position[1] > 0.99)
        );
    }

    #[test]
    fn sculpted_features_match_the_face_surface_and_remain_bounded() {
        for index in 0..2 {
            let portrait = Portrait::for_host(index);
            let eye_depth = face_front(index, portrait.eye_spacing, portrait.eye_y);
            assert!(eye_depth.is_finite() && (0.12..0.18).contains(&eye_depth));
            for step in 0..1000 {
                let height = 0.015 + step as f32 * 0.00050;
                let sample = contour_sample(index, height);
                assert!(sample.is_finite() && sample.min_element() > 0.0);
                assert!(sample.max_element() < 0.21);
                let depth = face_front(index, 0.0, height);
                assert!(depth.is_finite() && (0.0..0.25).contains(&depth));
            }
        }
    }

    #[test]
    fn smile_lips_remain_finite_and_have_raised_corners() {
        for index in 0..2 {
            let mesh = smile(index);
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("missing positions");
            };
            let Some(VertexAttributeValues::Float32x3(normals)) =
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            else {
                panic!("missing normals");
            };
            assert!(positions.iter().flatten().all(|value| value.is_finite()));
            assert!(normals.iter().flatten().all(|value| value.is_finite()));
            assert!(
                positions
                    .iter()
                    .filter(|position| position[0].abs() > 0.99)
                    .all(|position| position[1] > 0.5)
            );
        }
    }

    #[test]
    fn head_centerline_has_continuous_shading() {
        for index in 0..2 {
            let mesh = face(index);
            let Some(VertexAttributeValues::Float32x3(normals)) =
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            else {
                panic!("missing normals")
            };
            for ring in 0..=64 {
                assert_eq!(normals[ring * 73], normals[ring * 73 + 72]);
            }
        }
    }

    #[test]
    fn authored_head_surfaces_are_closed() {
        use std::collections::BTreeMap;

        for index in 0..2 {
            let mesh = face(index);
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("missing positions")
            };
            let mut edges = BTreeMap::new();
            let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
            for &[first, second, third] in indices.as_chunks::<3>().0 {
                for (start, end) in [(first, second), (second, third), (third, first)] {
                    let start = positions[start].map(|value| (value * 1_000_000.0).round() as i32);
                    let end = positions[end].map(|value| (value * 1_000_000.0).round() as i32);
                    let key = if start < end {
                        (start, end)
                    } else {
                        (end, start)
                    };
                    let entry = edges.entry(key).or_insert((0, 0));
                    entry.0 += 1;
                    entry.1 += if start < end { 1 } else { -1 };
                }
            }
            assert!(
                edges
                    .values()
                    .all(|&(count, winding)| count == 2 && winding == 0)
            );
        }
    }

    #[test]
    fn tooth_crowns_are_finite_and_remain_inside_the_animated_mouth_bounds() {
        let mesh = teeth();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing positions")
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("missing normals")
        };
        assert!(
            positions
                .iter()
                .flatten()
                .all(|value| value.is_finite() && value.abs() <= 1.0)
        );
        assert_eq!(positions.len(), normals.len());
        assert!(
            normals
                .iter()
                .all(|normal| (Vec3::from_array(*normal).length() - 1.0).abs() < 0.00001)
        );
        assert!(
            mesh.indices()
                .unwrap()
                .iter()
                .all(|index| index < positions.len())
        );
    }

    #[test]
    fn resting_smiles_preserve_bounded_speech_driven_teeth() {
        assert_eq!(tooth_visibility(0, 0.0), 0.0);
        assert_eq!(tooth_visibility(1, 0.0), 0.0);
        for index in 0..2 {
            for step in 0..100 {
                let jaw = step as f32 / 99.0;
                assert!((0.0..=1.0).contains(&tooth_visibility(index, jaw)));
                assert!(tooth_visibility(index, jaw) >= tooth_visibility(index, 0.0));
            }
        }
    }

    #[test]
    fn crown_strands_lips_and_teeth_are_finite_and_face_outward() {
        let mut meshes = vec![teeth()];
        for index in 0..2 {
            meshes.extend([lip(index, true), lip(index, false)]);
            for side in [-1.0, 1.0] {
                for strand in 0..4 {
                    meshes.push(crown_strand(index, side, strand));
                }
            }
        }
        for mesh in meshes {
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("missing positions")
            };
            let Some(VertexAttributeValues::Float32x3(normals)) =
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            else {
                panic!("missing normals")
            };
            assert_eq!(positions.len(), normals.len());
            assert!(
                positions
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite() && value.abs() < 2.0)
            );
            assert!(normals.iter().flatten().all(|value| value.is_finite()));
            let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
            assert!(indices.iter().all(|&index| index < positions.len()));
            for &[first, second, third] in indices.as_chunks::<3>().0 {
                let first_point = Vec3::from_array(positions[first]);
                let cross = (Vec3::from_array(positions[second]) - first_point)
                    .cross(Vec3::from_array(positions[third]) - first_point);
                assert!(cross.dot(Vec3::from_array(normals[first])) >= -0.00000001);
            }
        }
    }

    #[test]
    fn hosts_have_distinct_silhouettes_and_surface_patterns() {
        assert_ne!(
            face(0).attribute(Mesh::ATTRIBUTE_POSITION),
            face(1).attribute(Mesh::ATTRIBUTE_POSITION)
        );
        assert!(Portrait::for_host(0).hair_length > Portrait::for_host(1).hair_length);
        for index in 0..2 {
            let mesh = garment(index);
            let Some(VertexAttributeValues::Float32x4(colors)) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("missing garment colors");
            };
            assert_eq!(colors.len(), mesh.count_vertices());
            assert!(
                colors
                    .iter()
                    .flatten()
                    .all(|value| (0.0..=1.0).contains(value))
            );
            assert!(colors.iter().any(|color| color != &colors[0]));
        }
    }
}
