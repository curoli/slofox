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
    pub draw_hair_part: bool,
    pub hair_back_center: f32,
    pub hair_back_half_height: f32,
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
                hair_length: 0.75,
                hair_part: 0.022,
                hair_width: 0.228,
                draw_hair_part: false,
                hair_back_center: 0.30,
                hair_back_half_height: 0.22,
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
                hair_length: 0.43,
                hair_part: -0.052,
                hair_width: 0.238,
                draw_hair_part: false,
                hair_back_center: 0.255,
                hair_back_half_height: 0.230,
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

const FINGER_ASPECT: f32 = 5.0;

fn finger_bend(amount: f32) -> f32 {
    let ease = |value: f32| {
        let value = value.clamp(0.0, 1.0);
        value * value * (3.0 - 2.0 * value)
    };
    28.0_f32.to_radians() * ease((amount - 0.40) / 0.10)
        + 18.0_f32.to_radians() * ease((amount - 0.73) / 0.08)
}

fn finger_frame(length: f32, vertical: bool) -> (Vec3, Vec3) {
    if vertical {
        return (Vec3::new(0.0, 0.0, length), Vec3::Y);
    }
    let amount = ((length + 1.0) * 0.5).clamp(0.0, 1.0);
    let step = amount / 96.0;
    let mut center = -Vec3::Z;
    for sample in 0..96 {
        let bend = finger_bend((sample as f32 + 0.5) * step);
        center += Vec3::new(0.0, -FINGER_ASPECT * bend.sin(), bend.cos()) * (2.0 * step);
    }
    let bend = finger_bend(amount);
    (
        center,
        Vec3::new(0.0, bend.cos(), bend.sin() / FINGER_ASPECT),
    )
}

pub fn finger(vertical: bool) -> Mesh {
    let segments = 20;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..49 {
        let (center, angle) = if ring <= 8 {
            (-0.8, -PI * 0.5 + ring as f32 / 8.0 * PI * 0.5)
        } else if ring <= 40 {
            (-0.8 + (ring - 8) as f32 / 32.0 * 1.6, 0.0)
        } else {
            (0.8, (ring - 40) as f32 / 8.0 * PI * 0.5)
        };
        let radius = angle.cos().max(0.0);
        let length = center + 0.2 * angle.sin();
        let (center, normal) = finger_frame(length, vertical);
        for segment in 0..=segments {
            let around = segment as f32 / segments as f32 * PI * 2.0;
            let horizontal = radius * around.sin();
            let depth = radius * around.cos();
            let point = center + Vec3::X * horizontal + normal * depth;
            positions.push(if vertical {
                [point.x, point.z, -point.y]
            } else {
                point.to_array()
            });
            if ring < 48 && segment < segments {
                connect_ring(&mut indices, ring, segment, segments);
            }
        }
    }
    for triangle in indices.as_chunks_mut::<3>().0 {
        triangle.swap(1, 2);
    }
    indexed_surface(positions, indices)
}

pub fn thumb(side: f32, distal: bool, bind: Transform) -> Mesh {
    let start = Vec3::new(-side * 0.035, 0.002, 0.043);
    let control = Vec3::new(-side * 0.065, 0.001, 0.080);
    let end = Vec3::new(-side * 0.079, -0.003, 0.108);
    let segments = 20;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=40 {
        let (amount, cap, extension) = if ring < 4 {
            let angle = (4 - ring) as f32 / 4.0 * PI * 0.5;
            (0.0, angle.cos().max(0.0), -0.015 * angle.sin())
        } else if ring > 36 {
            let angle = (ring - 36) as f32 / 4.0 * PI * 0.5;
            (1.0, angle.cos().max(0.0), 0.011 * angle.sin())
        } else {
            ((ring - 4) as f32 / 32.0, 1.0, 0.0)
        };
        let tangent = ((control - start) * (1.0 - amount) + (end - control) * amount).normalize();
        let across = tangent.cross(Vec3::Y).normalize();
        let normal = tangent.cross(across).normalize();
        let center = start * (1.0 - amount).powi(2)
            + control * (2.0 * amount * (1.0 - amount))
            + end * amount.powi(2)
            + tangent * extension;
        let radius = (0.015 - 0.004 * amount) * cap;
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * PI * 2.0;
            positions
                .push((center + radius * (across * angle.sin() + normal * angle.cos())).to_array());
            if ring < 40 && segment < segments {
                connect_ring(&mut indices, ring, segment, segments);
            }
        }
    }
    for triangle in indices.as_chunks_mut::<3>().0 {
        triangle.swap(1, 2);
    }
    let mut mesh = indexed_surface(positions, indices);
    let inverse = bind.to_matrix().inverse();
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for point in positions {
            *point = inverse
                .transform_point3(Vec3::from_array(*point))
                .to_array();
        }
    }
    let normal_transform = bind.to_matrix().transpose();
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(normals)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
    {
        for normal in normals {
            *normal = normal_transform
                .transform_vector3(Vec3::from_array(*normal))
                .normalize_or_zero()
                .to_array();
        }
    }
    let indices: Vec<u32> = mesh
        .indices()
        .unwrap()
        .iter()
        .collect::<Vec<_>>()
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|triangle| {
            let first_ring = triangle.iter().min().unwrap() / (segments + 1);
            (first_ring >= 20) == distal
        })
        .flatten()
        .map(|index| *index as u32)
        .collect();
    mesh.insert_indices(Indices::U32(indices));
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

fn scalp_point(index: usize, height: f32, angle: f32, clearance: f32) -> Vec3 {
    let head = super::sculpt::head(index);
    let profile = head.profile(height);
    let horizontal = profile.x * angle.sin();
    let depth = if angle.cos() > 0.0 {
        head.front(horizontal, height)
    } else {
        profile.y * angle.cos()
    };
    Vec3::new(
        horizontal + clearance * angle.sin(),
        height,
        depth + clearance * angle.cos(),
    )
}

fn hair_shell_point(index: usize, height: f32, angle: f32, clearance: f32) -> Vec3 {
    if index != 0 {
        return scalp_point(index, height, angle, clearance);
    }
    let top = super::sculpt::head(index).heights().1;
    let upward = ((height - top + 0.11) / 0.11).clamp(0.0, 1.0);
    let upward = upward * upward * (3.0 - 2.0 * upward);
    let normal = Vec3::new(
        angle.sin() * (1.0 - upward),
        upward,
        angle.cos() * (1.0 - upward),
    )
    .normalize();
    let front = ((angle.cos() + 0.05) / 0.15).clamp(0.0, 1.0);
    let front = front * front * (3.0 - 2.0 * front);
    let cover = (-((height - 0.30) / 0.04).powi(2)).exp() * angle.sin().abs().powi(8) * front;
    scalp_point(index, height, angle, 0.0)
        + normal * (0.026 + clearance)
        + Vec3::new(angle.sin().signum() * 0.008 * cover, 0.0, 0.030 * cover)
}

fn hairline_height(index: usize, angle: f32) -> f32 {
    let front = angle.cos().max(0.0);
    let transition = ((front - 0.10) / 0.75).clamp(0.0, 1.0);
    let transition = transition * transition * (3.0 - 2.0 * transition);
    let mut height =
        0.19 + if index == 0 { 0.255 } else { 0.235 } * transition + 0.020 * front.powi(6);
    if index == 1 {
        height += 0.025 * (-((front - 0.65) / 0.20).powi(2)).exp();
    }
    height
}

pub fn hair_cap(index: usize) -> Mesh {
    let top = super::sculpt::head(index).heights().1;
    let rings = 64;
    let segments = 64;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * PI * 2.0;
            let height = hairline_height(index, angle) + if index == 0 { 0.012 } else { 0.0 };
            let progress = ring as f32 / rings as f32;
            let height = top + 0.006 + (height - top - 0.006) * (progress * PI * 0.5).sin();
            let point = hair_shell_point(index, height, angle, 0.006);
            positions.push(point.to_array());
            if ring < rings && segment < segments {
                connect_ring(&mut indices, ring, segment, segments);
            }
        }
    }
    if index == 0 {
        for rim in 1..=6 {
            let rounding = rim as f32 / 6.0 * PI * 0.5;
            for segment in 0..=segments {
                let angle = segment as f32 / segments as f32 * PI * 2.0;
                let height = hairline_height(index, angle) + 0.012 * (1.0 - rounding.sin());
                let skin = scalp_point(index, height, angle, 0.0);
                let shell = hair_shell_point(index, height, angle, 0.006);
                let point = skin
                    + (shell - skin) * rounding.cos()
                    + (scalp_point(index, height, angle, 0.001) - skin) * (1.0 - rounding.cos());
                positions.push(point.to_array());
                if segment < segments {
                    connect_ring(&mut indices, rings + rim - 1, segment, segments);
                }
            }
        }
        let center = positions.len() as u32;
        let height = positions[..segments + 1]
            .iter()
            .map(|point| point[1])
            .fold(f32::NEG_INFINITY, f32::max);
        positions.push([0.0, height, 0.0]);
        for segment in 0..segments {
            indices.extend_from_slice(&[center, segment as u32 + 1, segment as u32]);
        }
    }
    for triangle in indices.as_chunks_mut::<3>().0 {
        triangle.swap(1, 2);
    }
    indexed_surface(positions, indices)
}

pub fn hair_lock(side: f32, lock: usize, highlight: bool) -> Mesh {
    let portrait = Portrait::for_host(1);
    let rings = 32;
    let segments = 12;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        let progress = ring as f32 / rings as f32;
        let rounding = (progress / 0.14).clamp(0.0, 1.0);
        let root_taper = (rounding * rounding * (3.0 - 2.0 * rounding)).max(0.025);
        let vertical = 0.455 - progress * portrait.hair_length;
        let center = scalp_point(
            1,
            vertical,
            side * (PI * 0.46 + lock as f32 * 0.16),
            0.003 * root_taper,
        );
        let taper = (1.0 - progress).powf(0.30).max(0.06) * root_taper;
        let width = if highlight { 0.003 } else { 0.009 } * taper;
        let depth = if highlight { 0.002 } else { 0.009 } * taper;
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * PI * 2.0;
            positions.push([
                center.x + width * angle.sin(),
                vertical,
                center.z + depth * angle.cos() + if highlight { 0.008 * taper } else { 0.0 },
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

pub fn back_hair(index: usize) -> Mesh {
    let portrait = Portrait::for_host(index);
    let rings = 32;
    let segments = 48;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        let progress = ring as f32 / rings as f32;
        let bottom = portrait.hair_back_center - portrait.hair_back_half_height;
        let height = 0.40 + (bottom - 0.40) * progress;
        for segment in 0..=segments {
            let angle = PI * 0.5 + segment as f32 / segments as f32 * PI;
            positions.push(hair_shell_point(index, height, angle, 0.007).to_array());
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

pub fn ponytail() -> Mesh {
    let rings = 48;
    let segments = 32;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        let progress = ring as f32 / rings as f32;
        let fullness = (progress * PI).sin().max(0.0).powf(0.55);
        let width = 0.025 * (1.0 - progress) + 0.125 * fullness + 0.002;
        let depth = 0.025 * (1.0 - progress) + 0.075 * fullness + 0.002;
        let horizontal = 0.11 * (progress * PI * 0.85).sin();
        let height = 0.33 - progress * Portrait::for_host(0).hair_length;
        let back = -contour_sample(0, 0.33).y - 0.026 + 0.022 - 0.16 * (progress * PI * 0.8).sin();
        for segment in 0..=segments {
            let angle = segment as f32 / segments as f32 * PI * 2.0;
            let wave = 1.0 + 0.035 * (angle * 9.0 + progress * 5.0).cos() * fullness;
            positions.push([
                horizontal + width * wave * angle.sin(),
                height,
                back + depth * wave * angle.cos(),
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
    let top = super::sculpt::head(index).heights().1;
    let segments = 24;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for step in 0..=segments {
        let progress = step as f32 / segments as f32;
        let height = top - 0.010 - (0.055 + strand as f32 * 0.010) * progress;
        for edge in [-1.0, 1.0] {
            let angle = side * (0.20 + progress * 1.10) + edge * 0.012;
            let normal = Vec3::new(angle.sin(), 0.0, angle.cos());
            let point = hair_shell_point(index, height, angle, 0.008);
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
    fn finger_phalanges_bend_at_both_internal_joints_without_tilting_the_root() {
        let mesh = finger(false);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("finger positions")
        };
        let centers: Vec<_> = positions
            .as_chunks::<21>()
            .0
            .iter()
            .map(|ring| {
                let center = (Vec3::from_array(ring[0]) + Vec3::from_array(ring[10])) * 0.5;
                Vec3::new(center.x, center.y / FINGER_ASPECT, center.z)
            })
            .collect();
        let proximal = (centers[16] - centers[12]).normalize();
        let middle = (centers[30] - centers[26]).normalize();
        let distal = (centers[44] - centers[40]).normalize();
        assert!(proximal.angle_between(Vec3::Z) < 0.01);
        assert!((proximal.angle_between(middle) - 28.0_f32.to_radians()).abs() < 0.01);
        assert!((middle.angle_between(distal) - 18.0_f32.to_radians()).abs() < 0.01);
        assert!(centers[48].y < -0.5);
        for ring in 10..40 {
            let before = (centers[ring] - centers[ring - 1]).normalize();
            let after = (centers[ring + 1] - centers[ring]).normalize();
            assert!(before.angle_between(after) < 0.20);
        }
    }

    #[test]
    fn fingers_have_rounded_ends_and_unpinched_joint_cross_sections() {
        for vertical in [false, true] {
            let mesh = finger(vertical);
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("missing finger positions")
            };
            let Some(VertexAttributeValues::Float32x3(normals)) =
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            else {
                panic!("missing finger normals")
            };
            for (ring, points) in positions.as_chunks::<21>().0.iter().enumerate() {
                let (center, angle) = if ring <= 8 {
                    (-0.8, -PI * 0.5 + ring as f32 / 8.0 * PI * 0.5)
                } else if ring <= 40 {
                    (-0.8 + (ring - 8) as f32 / 32.0 * 1.6, 0.0)
                } else {
                    (0.8, (ring - 40) as f32 / 8.0 * PI * 0.5)
                };
                let (center, normal) = finger_frame(center + 0.2 * angle.sin(), vertical);
                for (segment, point) in points.iter().enumerate() {
                    let point = Vec3::from_array(*point);
                    let point = if vertical {
                        Vec3::new(point.x, -point.z, point.y)
                    } else {
                        point
                    };
                    let offset = point - center;
                    let radial = Vec2::new(offset.x, offset.y / normal.y);
                    assert!((radial.length() - angle.cos().max(0.0)).abs() < 0.00001);
                    if (8..=40).contains(&ring) {
                        assert!((radial.length() - 1.0).abs() < 0.00001);
                    }
                    assert!(point.is_finite() && point.abs().max_element() < 6.0);
                    let outward = Vec3::from_array(normals[ring * 21 + segment]);
                    let outward = if vertical {
                        Vec3::new(outward.x, -outward.z, outward.y)
                    } else {
                        outward
                    };
                    assert!(outward.dot(offset) >= -0.00001);
                }
            }
            assert!(normals.iter().flatten().all(|normal| normal.is_finite()));
        }
    }

    #[test]
    fn thicker_browser_hair_has_a_closed_rim_meeting_the_scalp() {
        let mesh = hair_cap(0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing hair positions")
        };
        let triangles: Vec<_> = mesh.indices().unwrap().iter().collect();
        let shell_vertices = 65 * 65;
        assert_eq!(positions.len(), shell_vertices + 6 * 65 + 1);
        for segment in 0..64 {
            let angle = segment as f32 / 64.0 * PI * 2.0;
            let outer = 64 * 65 + segment;
            let inner = shell_vertices + 5 * 65 + segment;
            let skin = scalp_point(0, hairline_height(0, angle), angle, 0.0);
            assert!((Vec3::from_array(positions[inner]) - skin).length() < 0.002);
            let thickness = Vec3::from_array(positions[outer]).distance(scalp_point(
                0,
                hairline_height(0, angle) + 0.012,
                angle,
                0.0,
            ));
            assert!((0.030..0.060).contains(&thickness));
            for rim in 0..6 {
                let start = outer + rim * 65;
                let end = start + 65;
                assert!(
                    triangles
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .any(|triangle| { triangle.contains(&start) && triangle.contains(&end) })
                );
            }
            let first =
                Vec3::from_array(positions[outer + 65]) - Vec3::from_array(positions[outer]);
            let last = Vec3::from_array(positions[inner]) - Vec3::from_array(positions[inner - 65]);
            assert!(first.normalize().dot(last.normalize()) < 0.85);
        }
        assert!(
            positions
                .iter()
                .any(|point| (0.28..0.32).contains(&point[1])
                    && point[0] > 0.22
                    && point[2] > 0.05)
        );
    }

    #[test]
    fn browser_hair_has_a_high_hairline_and_voluminous_gathered_tail() {
        let cap = hair_cap(0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            cap.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing cap positions")
        };
        let front_hairline = positions[64 * 65];
        assert!(front_hairline[1] > 0.46);
        assert!(positions.iter().any(|point| point[1] > 0.50));
        assert!(positions.iter().all(|point| point[0].abs() < 0.24));
        assert!(front_hairline[2] > face_front(0, front_hairline[0], front_hairline[1]));
        let tail = ponytail();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            tail.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing ponytail positions")
        };
        assert!(positions.iter().all(|point| point[2] < -0.08));
        let root_front = positions[..33]
            .iter()
            .map(|point| point[2])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            root_front > -contour_sample(0, 0.33).y,
            "detached ponytail root"
        );
        assert!(positions.iter().any(|point| point[1] < -0.41));
        let middle = &positions[24 * 33..25 * 33];
        let width = middle
            .iter()
            .map(|point| point[0])
            .fold(f32::NEG_INFINITY, f32::max)
            - middle
                .iter()
                .map(|point| point[0])
                .fold(f32::INFINITY, f32::min);
        assert!(width > 0.26);
    }

    #[test]
    fn ponytail_mesh_is_finite_and_outward_facing() {
        let mesh = ponytail();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing ponytail positions")
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("missing ponytail normals")
        };
        assert!(positions.iter().flatten().all(|value| value.is_finite()));
        assert!(
            normals
                .iter()
                .all(|normal| Vec3::from_array(*normal).is_normalized())
        );
        assert!(mesh.count_vertices() < 2000);
        for ring in 1..48 {
            let front = ring * 33;
            let back = front + 16;
            assert!(normals[front][2] > 0.0);
            assert!(normals[back][2] < 0.0);
        }
    }

    #[test]
    fn microphone_hair_roots_are_tapered_and_embedded_in_the_cap() {
        for side in [-1.0, 1.0] {
            for lock in 0..5 {
                for highlight in [false, true] {
                    let mesh = hair_lock(side, lock, highlight);
                    let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        panic!("missing hair positions")
                    };
                    for &[horizontal, height, depth] in &positions[..13] {
                        let expected =
                            scalp_point(1, height, side * (PI * 0.46 + lock as f32 * 0.16), 0.0);
                        assert!(
                            Vec2::new(horizontal - expected.x, depth - expected.z).length() < 0.002,
                            "exposed hair root: side {side}, lock {lock}, point {horizontal}, {height}, {depth}"
                        );
                    }
                    let root_width = positions[..13]
                        .iter()
                        .map(|point| point[0])
                        .fold(f32::NEG_INFINITY, f32::max)
                        - positions[..13]
                            .iter()
                            .map(|point| point[0])
                            .fold(f32::INFINITY, f32::min);
                    assert!(root_width < 0.003);
                }
            }
        }
    }

    #[test]
    fn hair_shells_follow_the_authored_scalp_with_the_intended_thickness() {
        for index in 0..2 {
            for (mesh, segments, start, sweep) in [
                (hair_cap(index), 64, 0.0, PI * 2.0),
                (back_hair(index), 48, PI * 0.5, PI),
            ] {
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("missing hair positions")
                };
                let shell_vertices = if segments == 64 {
                    65 * 65
                } else {
                    positions.len()
                };
                for (vertex, &[horizontal, height, depth]) in
                    positions.iter().take(shell_vertices).enumerate()
                {
                    let angle = start + (vertex % (segments + 1)) as f32 / segments as f32 * sweep;
                    let surface = scalp_point(index, height, angle, 0.0);
                    if index == 1 {
                        let gap = Vec2::new(horizontal - surface.x, depth - surface.z).length();
                        assert!(gap < 0.015, "floating hair: host {index}, gap {gap}");
                    } else if height < super::super::sculpt::head(index).heights().1 - 0.12 {
                        let gap = Vec2::new(horizontal - surface.x, depth - surface.z).length();
                        assert!((0.031..0.060).contains(&gap));
                    }
                    assert!(
                        height
                            <= super::super::sculpt::head(index).heights().1
                                + if index == 0 { 0.040 } else { 0.007 }
                    );
                }
            }
        }
    }

    #[test]
    fn microphone_hair_drapes_close_to_the_head_without_flared_ends() {
        for side in [-1.0, 1.0] {
            for lock in 0..5 {
                for highlight in [false, true] {
                    let mesh = hair_lock(side, lock, highlight);
                    let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        panic!("missing hair positions")
                    };
                    for &[horizontal, height, _] in positions {
                        if height <= 0.24 {
                            assert!(horizontal.abs() <= 0.22);
                        }
                        if height <= 0.05 {
                            assert!(horizontal.abs() <= 0.19);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn microphone_hair_ends_near_the_chin_without_a_drawn_part() {
        let male = Portrait::for_host(1);
        assert!(!male.draw_hair_part);
        assert!(!Portrait::for_host(0).draw_hair_part);
        assert!((male.hair_back_center - male.hair_back_half_height - 0.025).abs() < 0.00001);
        for side in [-1.0, 1.0] {
            for lock in 0..5 {
                for highlight in [false, true] {
                    let mesh = hair_lock(side, lock, highlight);
                    let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        panic!("missing hair positions")
                    };
                    let bottom = positions
                        .iter()
                        .map(|point| point[1])
                        .fold(f32::INFINITY, f32::min);
                    assert!((bottom - 0.025).abs() < 0.00001);
                }
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
            let mut meshes = vec![
                face(index),
                eyelid(index, true),
                eyelid(index, false),
                hair_cap(index),
                back_hair(index),
                garment(index),
            ];
            if index == 1 {
                meshes.extend([hair_lock(-1.0, 0, false), hair_lock(1.0, 4, true)]);
            }
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
                assert!(sample.x < 0.21);
                assert!(sample.y < 0.33);
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
