use bevy::prelude::*;

use super::{DESK_SURFACE_Y, Geometry, matte, object};

const TITLE: &str = "AI Talks with Jati & Oliver";
const SUBTITLE: &str = "Sonderausgabe: Iris hat Geburtstag";

#[derive(Component)]
struct SetPiece;

fn rod_transform(start: Vec3, end: Vec3, radius: f32) -> Transform {
    let direction = end - start;
    Transform::from_translation((start + end) * 0.5)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, direction.normalize()))
        .with_scale(Vec3::new(radius, direction.length(), radius))
}

fn block(
    commands: &mut Commands,
    geometry: &Geometry,
    parent: Entity,
    material: &Handle<StandardMaterial>,
    position: Vec3,
    size: Vec3,
) {
    object(
        commands,
        Some(parent),
        &geometry.cube,
        material,
        Transform::from_translation(position).with_scale(size),
    );
}

fn group(commands: &mut Commands, name: &'static str, position: Vec3) -> Entity {
    commands
        .spawn((
            SetPiece,
            Name::new(name),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .id()
}

struct Sign {
    name: &'static str,
    label: String,
    bytes: &'static [u8],
    position: Vec3,
    size: Vec2,
    glowing: bool,
}

fn sign_image(bytes: &[u8]) -> Image {
    Image::from_buffer(
        bytes,
        bevy::image::ImageType::Extension("png"),
        bevy::image::CompressedImageFormats::NONE,
        true,
        bevy::image::ImageSampler::linear(),
        bevy::asset::RenderAssetUsages::default(),
    )
    .expect("bundled studio sign must be a valid PNG")
}

fn broadcast_graphics(
    commands: &mut Commands,
    geometry: &Geometry,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) {
    let backing = matte(materials, Color::srgb_u8(38, 34, 30));
    let signs = [
        Sign {
            name: "titles",
            label: format!("{TITLE}\n{SUBTITLE}"),
            bytes: include_bytes!("../../assets/studio/title.png"),
            position: Vec3::new(0.0, 2.90, -1.57),
            size: Vec2::new(3.0, 0.515625),
            glowing: false,
        },
        Sign {
            name: "on-air",
            label: "On Air".into(),
            bytes: include_bytes!("../../assets/studio/on-air.png"),
            position: Vec3::new(0.0, 2.50, -1.57),
            size: Vec2::new(0.66, 0.22),
            glowing: true,
        },
        Sign {
            name: "calendar",
            label: "OKTOBER\n17\n2026".into(),
            bytes: include_bytes!("../../assets/studio/calendar.png"),
            position: Vec3::new(-2.36, 2.22, -1.57),
            size: Vec2::new(0.60, 0.825),
            glowing: false,
        },
        Sign {
            name: "credit",
            label: "Powered by Slofox".into(),
            bytes: include_bytes!("../../assets/studio/credit.png"),
            position: Vec3::new(0.90, 0.72, 1.145),
            size: Vec2::new(0.78, 0.121875),
            glowing: false,
        },
    ];
    for sign in signs {
        let root = group(commands, sign.name, sign.position);
        block(
            commands,
            geometry,
            root,
            &backing,
            Vec3::ZERO,
            Vec3::new(sign.size.x + 0.025, sign.size.y + 0.025, 0.05),
        );
        let texture = images.add(sign_image(sign.bytes));
        let material = materials.add(StandardMaterial {
            base_color: if sign.glowing {
                Color::BLACK
            } else {
                Color::WHITE
            },
            base_color_texture: Some(texture.clone()),
            emissive: if sign.glowing {
                LinearRgba::rgb(8.0, 0.3, 0.3)
            } else {
                LinearRgba::BLACK
            },
            emissive_texture: sign.glowing.then_some(texture),
            unlit: !sign.glowing,
            perceptual_roughness: 0.8,
            ..default()
        });
        let face = meshes.add(Rectangle::new(sign.size.x, sign.size.y));
        let entity = object(
            commands,
            Some(root),
            &face,
            &material,
            Transform::from_xyz(0.0, 0.0, 0.031),
        );
        commands.entity(entity).insert(Name::new(sign.label));
        if sign.glowing {
            commands.spawn((
                PointLight {
                    color: Color::srgb_u8(255, 28, 35),
                    intensity: 35.0,
                    range: 0.85,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.12),
                ChildOf(root),
            ));
        }
    }
}

pub(super) fn setup(
    commands: &mut Commands,
    geometry: &Geometry,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) {
    let oak = matte(materials, Color::srgb_u8(130, 82, 49));
    let cream = matte(materials, Color::srgb_u8(245, 228, 193));
    let teal = matte(materials, Color::srgb_u8(42, 115, 119));
    let rust = matte(materials, Color::srgb_u8(165, 67, 44));
    let gold = matte(materials, Color::srgb_u8(220, 166, 71));
    let navy = matte(materials, Color::srgb_u8(38, 56, 75));
    let green = matte(materials, Color::srgb_u8(48, 115, 63));
    let light_green = matte(materials, Color::srgb_u8(91, 146, 67));
    let soil = matte(materials, Color::srgb_u8(47, 32, 25));
    let sky = matte(materials, Color::srgb_u8(140, 178, 171));
    let colors = [&teal, &rust, &gold, &navy, &cream];

    for side in [-1.0, 1.0] {
        let picture = group(commands, "picture", Vec3::new(side * 1.55, 2.24, -1.58));
        block(
            commands,
            geometry,
            picture,
            &oak,
            Vec3::ZERO,
            Vec3::new(0.83, 0.64, 0.08),
        );
        block(
            commands,
            geometry,
            picture,
            if side < 0.0 { &sky } else { &cream },
            Vec3::new(0.0, 0.0, 0.05),
            Vec3::new(0.73, 0.54, 0.015),
        );
        object(
            commands,
            Some(picture),
            &geometry.sphere,
            &gold,
            Transform::from_xyz(-0.16 * side, 0.12, 0.065)
                .with_scale(Vec3::new(0.085, 0.085, 0.006)),
        );
        for hill in 0..3 {
            object(
                commands,
                Some(picture),
                &geometry.sphere,
                if hill == 1 { &teal } else { &green },
                Transform::from_xyz(
                    -0.22 + hill as f32 * 0.22,
                    -0.14,
                    0.07 + hill as f32 * 0.006,
                )
                .with_scale(Vec3::new(0.14, 0.08 + hill as f32 * 0.015, 0.005)),
            );
        }
        block(
            commands,
            geometry,
            picture,
            &navy,
            Vec3::new(0.0, -0.23, 0.095),
            Vec3::new(0.73, 0.08, 0.012),
        );
    }

    let shelf = group(commands, "bookshelf", Vec3::new(-2.55, 0.0, -1.23));
    for side in [-1.0, 1.0] {
        block(
            commands,
            geometry,
            shelf,
            &oak,
            Vec3::new(side * 0.38, 0.87, 0.0),
            Vec3::new(0.055, 1.74, 0.34),
        );
    }
    block(
        commands,
        geometry,
        shelf,
        &navy,
        Vec3::new(0.0, 0.87, -0.16),
        Vec3::new(0.71, 1.68, 0.025),
    );
    for level in 0..=3 {
        let height = 0.10 + level as f32 * 0.52;
        block(
            commands,
            geometry,
            shelf,
            &oak,
            Vec3::new(0.0, height, 0.0),
            Vec3::new(0.81, 0.045, 0.36),
        );
        if level < 3 {
            for book in 0..8 {
                let book_height = 0.25 + ((book * 3 + level) % 5) as f32 * 0.035;
                let horizontal = -0.30 + book as f32 * 0.085;
                block(
                    commands,
                    geometry,
                    shelf,
                    colors[(book + level) % colors.len()],
                    Vec3::new(horizontal, height + 0.025 + book_height * 0.5, 0.025),
                    Vec3::new(0.068, book_height, 0.24),
                );
                block(
                    commands,
                    geometry,
                    shelf,
                    &gold,
                    Vec3::new(horizontal, height + book_height * 0.72, 0.150),
                    Vec3::new(0.046, 0.008, 0.005),
                );
            }
        }
    }

    let plant = group(commands, "floor-plant", Vec3::new(2.75, 0.0, -0.55));
    object(
        commands,
        Some(plant),
        &geometry.cylinder,
        &rust,
        Transform::from_xyz(0.0, 0.19, 0.0).with_scale(Vec3::new(0.20, 0.38, 0.20)),
    );
    object(
        commands,
        Some(plant),
        &geometry.cylinder,
        &gold,
        Transform::from_xyz(0.0, 0.365, 0.0).with_scale(Vec3::new(0.215, 0.04, 0.215)),
    );
    object(
        commands,
        Some(plant),
        &geometry.cylinder,
        &soil,
        Transform::from_xyz(0.0, 0.389, 0.0).with_scale(Vec3::new(0.185, 0.01, 0.185)),
    );
    for leaf in 0..16 {
        let angle = leaf as f32 * 2.4;
        let height = 0.85 + (leaf % 5) as f32 * 0.14;
        let tip = Vec3::new(angle.cos() * 0.30, height, angle.sin() * 0.30);
        object(
            commands,
            Some(plant),
            &geometry.cylinder,
            &green,
            rod_transform(Vec3::new(0.0, 0.39, 0.0), tip, 0.008),
        );
        let direction = Vec3::new(angle.cos() * 0.6, 0.7, angle.sin() * 0.6).normalize();
        object(
            commands,
            Some(plant),
            &geometry.sphere,
            if leaf % 3 == 0 { &light_green } else { &green },
            Transform::from_translation(tip)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, direction))
                .with_scale(Vec3::new(0.085, 0.24, 0.025)),
        );
    }

    let desktop = group(
        commands,
        "desk-accessories",
        Vec3::new(0.0, DESK_SURFACE_Y, 0.0),
    );
    block(
        commands,
        geometry,
        desktop,
        &rust,
        Vec3::new(0.16, 0.012, 0.62),
        Vec3::new(0.31, 0.024, 0.22),
    );
    block(
        commands,
        geometry,
        desktop,
        &cream,
        Vec3::new(0.16, 0.030, 0.62),
        Vec3::new(0.29, 0.012, 0.20),
    );
    block(
        commands,
        geometry,
        desktop,
        &rust,
        Vec3::new(0.16, 0.040, 0.62),
        Vec3::new(0.31, 0.007, 0.22),
    );
    block(
        commands,
        geometry,
        desktop,
        &gold,
        Vec3::new(0.10, 0.045, 0.62),
        Vec3::new(0.012, 0.003, 0.18),
    );
    for note in 0..3 {
        let horizontal = -0.17 + note as f32 * 0.025;
        let depth = 0.88 + note as f32 * 0.015;
        let height = 0.004 + note as f32 * 0.003;
        block(
            commands,
            geometry,
            desktop,
            &cream,
            Vec3::new(horizontal, height, depth),
            Vec3::new(0.21, 0.002, 0.14),
        );
        if note == 2 {
            for line in 0..4 {
                block(
                    commands,
                    geometry,
                    desktop,
                    &navy,
                    Vec3::new(
                        horizontal,
                        height + 0.002,
                        depth - 0.04 + line as f32 * 0.022,
                    ),
                    Vec3::new(0.12, 0.001, 0.002),
                );
            }
        }
    }
    object(
        commands,
        Some(desktop),
        &geometry.cylinder,
        &teal,
        Transform::from_xyz(-1.52, 0.07, 0.68).with_scale(Vec3::new(0.042, 0.14, 0.042)),
    );
    for (pen, color) in colors.iter().enumerate() {
        let horizontal = -1.54 + pen as f32 * 0.009;
        object(
            commands,
            Some(desktop),
            &geometry.cylinder,
            color,
            rod_transform(
                Vec3::new(horizontal, 0.04, 0.68),
                Vec3::new(horizontal + 0.02, 0.22, 0.68 + (pen as f32 - 2.0) * 0.01),
                0.0035,
            ),
        );
    }
    broadcast_graphics(commands, geometry, meshes, images, materials);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cylindrical_stems_and_pens_reach_both_endpoints() {
        for (start, end, radius) in [
            (Vec3::new(0.0, 0.39, 0.0), Vec3::new(0.3, 1.41, -0.2), 0.008),
            (
                Vec3::new(-1.54, 0.04, 0.68),
                Vec3::new(-1.52, 0.22, 0.66),
                0.0035,
            ),
        ] {
            let transform = rod_transform(start, end, radius);
            assert!(transform.transform_point(Vec3::NEG_Y * 0.5).distance(start) < 0.00001);
            assert!(transform.transform_point(Vec3::Y * 0.5).distance(end) < 0.00001);
        }
    }

    #[test]
    fn birthday_studio_has_all_requested_labels_and_set_pieces() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>()
            .add_systems(Startup, super::super::setup);
        app.update();
        let world = app.world_mut();
        let labels: Vec<_> = world
            .query_filtered::<&Name, With<Mesh3d>>()
            .iter(world)
            .map(|name| name.as_str().to_owned())
            .collect();
        for expected in [
            &format!("{TITLE}\n{SUBTITLE}"),
            "On Air",
            "OKTOBER\n17\n2026",
            "Powered by Slofox",
        ] {
            assert!(
                labels.iter().any(|text| text == expected),
                "missing {expected}"
            );
        }
        let pieces: Vec<_> = world
            .query_filtered::<&Name, With<SetPiece>>()
            .iter(world)
            .map(|piece| piece.as_str())
            .collect();
        for expected in ["bookshelf", "floor-plant", "desk-accessories", "calendar"] {
            assert!(pieces.contains(&expected));
        }
        assert_eq!(pieces.iter().filter(|name| **name == "picture").count(), 2);
        assert_eq!(world.query::<&Node>().iter(world).count(), 0);
        let faces: Vec<_> = world
            .query::<(
                &Name,
                &Mesh3d,
                &MeshMaterial3d<StandardMaterial>,
                &ChildOf,
                &Transform,
            )>()
            .iter(world)
            .map(|(name, mesh, material, parent, transform)| {
                (
                    name.as_str().to_owned(),
                    mesh.0.clone(),
                    material.0.clone(),
                    parent.parent(),
                    *transform,
                )
            })
            .collect();
        assert_eq!(faces.len(), 4);
        for (name, mesh, material, parent, transform) in faces {
            assert!(world.get::<SetPiece>(parent).is_some());
            let root = world.get::<Transform>(parent).unwrap();
            if name.starts_with(TITLE) {
                let mesh = world.resource::<Assets<Mesh>>().get(&mesh).unwrap();
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("missing sign vertices")
                };
                for (portrait, camera) in [
                    (false, Vec3::new(0.0, 2.32, 4.25)),
                    (false, Vec3::new(-2.3, 2.25, 3.7)),
                    (false, Vec3::new(2.3, 2.25, 3.7)),
                    (true, Vec3::new(0.0, 2.13, 2.2)),
                    (true, Vec3::new(-1.25, 2.13, 2.2)),
                    (true, Vec3::new(1.25, 2.13, 2.2)),
                ] {
                    let target = Vec3::new(0.0, if portrait { 1.96 } else { 1.64 }, 0.0);
                    let view = Transform::from_translation(camera)
                        .looking_at(target, Vec3::Y)
                        .to_matrix()
                        .inverse();
                    let projection = Mat4::perspective_rh(
                        std::f32::consts::FRAC_PI_4,
                        1280.0 / 720.0,
                        0.1,
                        1000.0,
                    );
                    let clip_from_sign =
                        projection * view * root.to_matrix() * transform.to_matrix();
                    for position in positions {
                        let clip = clip_from_sign * Vec3::from_array(*position).extend(1.0);
                        assert!(
                            clip.w > 0.0 && clip.x.abs() < clip.w && clip.y.abs() < clip.w,
                            "title clipped from {camera:?}: {clip:?}"
                        );
                    }
                }
            }
            assert!((transform.translation.z - 0.031).abs() < 0.00001);
            if name == "Powered by Slofox" {
                assert!(root.translation.z > 1.12);
                assert!((0.58..1.07).contains(&root.translation.y));
            } else {
                assert!((root.translation.z + 1.57).abs() < 0.00001);
            }
            let material = world
                .resource::<Assets<StandardMaterial>>()
                .get(&material)
                .unwrap();
            assert!(material.base_color_texture.is_some());
            if name == "On Air" {
                assert!(material.emissive_texture.is_some());
                assert!(material.emissive.red > 1.0);
            }
        }
        assert_eq!(world.resource::<Assets<Image>>().len(), 4);
        for transform in world.query::<&Transform>().iter(world) {
            assert!(transform.translation.is_finite());
            assert!(transform.scale.is_finite());
            assert!(transform.scale.min_element() > 0.0);
        }
    }
}
