use std::f32::consts::PI;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

pub const SURFACE_Y: f32 = 1.17;
const THICKNESS: f32 = 0.10;
const CENTER_Z: f32 = 0.65;
const HALF_STRAIGHT: f32 = 1.35;
const RADIUS: f32 = 0.50;
const INSET: f32 = 0.03;

pub fn tabletop() -> Mesh {
    extrusion(SURFACE_Y - THICKNESS, SURFACE_Y, RADIUS)
}

pub fn apron() -> Mesh {
    extrusion(0.58, SURFACE_Y - THICKNESS, RADIUS - INSET)
}

fn extrusion(bottom: f32, top: f32, radius: f32) -> Mesh {
    let segments = 40;
    let mut outline = Vec::new();
    for side in [1.0, -1.0] {
        for segment in 0..=segments {
            let angle = -PI * 0.5
                + segment as f32 / segments as f32 * PI
                + if side < 0.0 { PI } else { 0.0 };
            outline.push(Vec2::new(
                side * HALF_STRAIGHT + radius * angle.cos(),
                CENTER_Z + radius * angle.sin(),
            ));
        }
    }
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for (height, normal) in [(top, Vec3::Y), (bottom, Vec3::NEG_Y)] {
        let center = positions.len() as u32;
        positions.push([0.0, height, CENTER_Z]);
        normals.push(normal.to_array());
        for point in &outline {
            positions.push([point.x, height, point.y]);
            normals.push(normal.to_array());
        }
        for segment in 0..outline.len() {
            let current = center + 1 + segment as u32;
            let next = center + 1 + ((segment + 1) % outline.len()) as u32;
            indices.extend(if height == top {
                [center, next, current]
            } else {
                [center, current, next]
            });
        }
    }
    for segment in 0..outline.len() {
        let start = outline[segment];
        let end = outline[(segment + 1) % outline.len()];
        let direction = end - start;
        let normal = Vec3::new(direction.y, 0.0, -direction.x).normalize();
        let base = positions.len() as u32;
        positions.extend_from_slice(&[
            [start.x, bottom, start.y],
            [start.x, top, start.y],
            [end.x, bottom, end.y],
            [end.x, top, end.y],
        ]);
        normals.extend_from_slice(&[normal.to_array(); 4]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use bevy::mesh::VertexAttributeValues;

    use super::*;

    #[test]
    fn desk_solids_are_closed_outward_and_have_no_duplicate_faces() {
        for mesh in [tabletop(), apron()] {
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
            let mut edges = BTreeMap::new();
            let point_key =
                |position: [f32; 3]| position.map(|value| (value * 100_000.0).round() as i32);
            let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
            for &[first, second, third] in indices.as_chunks::<3>().0 {
                let first_point = Vec3::from_array(positions[first]);
                let second_point = Vec3::from_array(positions[second]);
                let third_point = Vec3::from_array(positions[third]);
                let cross = (second_point - first_point).cross(third_point - first_point);
                assert!(cross.length() > 0.00001);
                assert!(cross.dot(Vec3::from_array(normals[first])) > 0.0);
                for (start, end) in [(first, second), (second, third), (third, first)] {
                    let start = point_key(positions[start]);
                    let end = point_key(positions[end]);
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
    fn apron_is_inset_and_meets_only_the_tabletop_underside() {
        let top = tabletop();
        let base = apron();
        let Some(VertexAttributeValues::Float32x3(top_positions)) =
            top.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing positions")
        };
        let Some(VertexAttributeValues::Float32x3(base_positions)) =
            base.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing positions")
        };
        let underside = top_positions
            .iter()
            .map(|point| point[1])
            .fold(f32::INFINITY, f32::min);
        let base_top = base_positions
            .iter()
            .map(|point| point[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(underside, base_top);
        for position in base_positions {
            let distance = Vec2::new(
                (position[0].abs() - HALF_STRAIGHT).max(0.0),
                position[2] - CENTER_Z,
            )
            .length();
            assert!(distance <= RADIUS - INSET + 0.00001);
        }
        assert!(
            top_positions
                .iter()
                .any(|point| (point[1] - SURFACE_Y).abs() < 0.00001)
        );
    }
}
