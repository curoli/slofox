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
    pub nose_width: f32,
    pub mouth_width: f32,
    pub mouth_y: f32,
    pub hair_length: f32,
    pub hair_part: f32,
}

impl Portrait {
    pub fn for_host(index: usize) -> Self {
        if index == 0 {
            Self {
                eye_width: 0.036,
                eye_height: 0.012,
                eye_y: 0.285,
                nose_width: 0.029,
                mouth_width: 0.064,
                mouth_y: 0.146,
                hair_length: 0.83,
                hair_part: 0.022,
            }
        } else {
            Self {
                eye_width: 0.033,
                eye_height: 0.013,
                eye_y: 0.296,
                nose_width: 0.035,
                mouth_width: 0.055,
                mouth_y: 0.135,
                hair_length: 0.46,
                hair_part: -0.052,
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

pub fn face(index: usize) -> Mesh {
    let contour = if index == 0 {
        [
            [0.015, 0.018, 0.047],
            [0.055, 0.084, 0.104],
            [0.105, 0.125, 0.128],
            [0.165, 0.158, 0.145],
            [0.225, 0.178, 0.163],
            [0.285, 0.174, 0.162],
            [0.350, 0.170, 0.154],
            [0.420, 0.157, 0.137],
            [0.480, 0.111, 0.094],
            [0.515, 0.002, 0.002],
        ]
    } else {
        [
            [0.008, 0.018, 0.047],
            [0.045, 0.087, 0.109],
            [0.095, 0.134, 0.135],
            [0.155, 0.150, 0.151],
            [0.225, 0.165, 0.162],
            [0.285, 0.168, 0.166],
            [0.350, 0.174, 0.158],
            [0.420, 0.166, 0.143],
            [0.485, 0.113, 0.097],
            [0.525, 0.002, 0.002],
        ]
    };
    let segments = 48;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for (ring, [height, width, depth]) in contour.into_iter().enumerate() {
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * PI * 2.0;
            let front = angle.cos();
            let cheek = if front > 0.0 { 0.012 } else { 0.0 };
            positions.push([width * angle.sin(), height, depth * front + cheek]);
            if ring + 1 < contour.len() && segment < segments {
                connect_ring(&mut indices, ring, segment, segments);
            }
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
            let temple = angle.sin().abs();
            let hairline = if index == 0 {
                1.89 - front.powi(3) * (0.83 - 0.12 * temple)
            } else {
                1.88 - front.powi(3) * (0.94 + 0.16 * temple)
            };
            let polar = (ring as f32 / rings as f32 * hairline).max(0.002);
            let sweep = if index == 0 { 0.008 } else { 0.018 };
            positions.push([
                0.205 * polar.sin() * angle.sin() + portrait.hair_part * 0.12 * polar.cos(),
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
            0.020 * (progress * PI * 3.0 + lock as f32 * 0.7).sin()
        };
        let horizontal = side * (0.157 + bend * 0.045 + lock as f32 * 0.011 + wave);
        let vertical = 0.455 - progress * portrait.hair_length;
        let drape = if index == 0 && side < 0.0 {
            progress * 0.13
        } else {
            0.0
        };
        let front = 0.048 - lock as f32 * 0.037 + bend * 0.008 + drape;
        let taper = (1.0 - progress).powf(0.30).max(0.06);
        let width = if highlight { 0.003 } else { 0.031 } * taper;
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
                0.47 + 0.025 * angle.sin().abs()
            } else {
                0.61 - 0.15 * front.powi(12)
            };
            let height = -0.10 + progress * (neckline + 0.10);
            let width = 0.265 + 0.031 * (progress * PI).sin();
            let depth = 0.15 + 0.012 * (progress * PI).sin();
            let horizontal = width * angle.sin();
            positions.push([horizontal, height, depth * angle.cos()]);
            let color = if index == 0 {
                let motif = (horizontal * 115.0 + (height * 37.0).sin() * 2.8).sin()
                    * (height * 95.0 + (horizontal * 40.0).cos() * 2.0).cos();
                if motif > 0.10 {
                    Color::srgb_u8(177, 91, 29)
                } else {
                    Color::srgb_u8(20, 36, 46)
                }
            } else {
                let diamond = (horizontal * 150.0).sin() * (height * 150.0).sin();
                if progress > 0.73 && angle.sin() > 0.50 {
                    Color::srgb_u8(112, 46, 39)
                } else if progress > 0.75 && angle.sin() < -0.50 {
                    Color::srgb_u8(58, 78, 58)
                } else if diamond > 0.58 {
                    Color::srgb_u8(77, 47, 39)
                } else if diamond < -0.60 {
                    Color::srgb_u8(123, 67, 53)
                } else {
                    Color::srgb_u8(194, 168, 133)
                }
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
    let positions = vec![
        [side * 0.028, 0.635, 0.116],
        [side * 0.112, 0.600, 0.153],
        [side * 0.084, 0.510, 0.172],
        [side * 0.052, 0.551, 0.164],
    ];
    let indices = if side > 0.0 {
        vec![0, 2, 1, 0, 3, 2]
    } else {
        vec![0, 1, 2, 0, 2, 3]
    };
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

#[cfg(test)]
mod tests {
    use bevy::mesh::VertexAttributeValues;

    use super::*;

    #[test]
    fn portrait_meshes_are_finite_indexed_and_bounded() {
        for index in 0..2 {
            let meshes = [
                face(index),
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
