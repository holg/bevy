//! Photometric lighting comparison — NATIVE (per-fragment GPU lookup)
//!
//! This example uses Bevy's native photometric lighting pipeline.
//! Each luminaire is a single PointLight with a PhotometricLight component
//! that references an IES profile. The angular intensity distribution is
//! sampled per-fragment on the GPU — no multi-spot approximation needed.
//!
//! Compare with `photometric_comparison_baseline` to see the difference:
//! - 2 lights instead of 10
//! - Correct angular distribution instead of splotchy spots
//! - Real color temperature via ColorTemperature component
//!
//! Run with: cargo run --example photometric_comparison_native \
//!           --features bevy/pbr_photometric_lights,bevy/pbr_clustered_decals

use bevy::{
    camera::Exposure,
    color::palettes::css::*,
    light::{ColorTemperature, PhotometricLight, PhotometricPlugin},
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhotometricPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, rotate_camera)
        .run();
}

#[derive(Component)]
struct CameraController;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // --- Ground plane (road surface) ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 40.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.35),
            perceptual_roughness: 0.9,
            ..default()
        })),
    ));

    // --- Walls to see light patterns on ---
    // Back wall
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(40.0, 8.0, 0.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.8, 0.8),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, 4.0, -10.0),
    ));

    // Side wall
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, 8.0, 20.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.8, 0.8),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(-10.0, 4.0, 0.0),
    ));

    // --- Reference objects ---
    for i in 0..4 {
        let x = -3.0 + i as f32 * 2.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.5, 1.0, 0.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.9, 0.9, 0.9),
                ..default()
            })),
            Transform::from_xyz(x, 0.5, -2.0),
        ));
    }

    // --- Light pole mesh (visual only) ---
    let pole_material = materials.add(StandardMaterial {
        base_color: DARK_GRAY.into(),
        metallic: 0.8,
        ..default()
    });

    // Load photometric profiles
    let road_profile = asset_server.load("photometric/road_luminaire.ies");
    let downlight_profile = asset_server.load("photometric/downlight.ies");

    // === NATIVE PHOTOMETRIC LIGHTS ===
    // One PointLight per luminaire. The full angular distribution is
    // evaluated per-fragment on the GPU via texture lookup.

    let luminaire_positions = [
        Vec3::new(-5.0, 6.0, 0.0),
        Vec3::new(5.0, 6.0, 0.0),
    ];

    for (idx, &pos) in luminaire_positions.iter().enumerate() {
        // Pole
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.05, pos.y * 2.0))),
            MeshMaterial3d(pole_material.clone()),
            Transform::from_xyz(pos.x, pos.y / 2.0, pos.z),
        ));

        // Luminaire housing
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.6, 0.08, 0.3))),
            MeshMaterial3d(pole_material.clone()),
            Transform::from_translation(pos),
        ));

        // === SINGLE LIGHT with photometric profile ===
        commands.spawn((
            PointLight {
                intensity: 300_000.0,
                range: 25.0,
                shadow_maps_enabled: idx == 0,
                ..default()
            },
            PhotometricLight {
                profile: road_profile.clone(),
            },
            ColorTemperature::new(3000.0),
            Transform::from_translation(pos),
        ));
    }

    // Add a couple of symmetric downlights for variety
    for i in 0..3 {
        let x = -2.0 + i as f32 * 2.0;
        commands.spawn((
            PointLight {
                intensity: 80_000.0,
                range: 12.0,
                shadow_maps_enabled: false,
                ..default()
            },
            PhotometricLight {
                profile: downlight_profile.clone(),
            },
            ColorTemperature::new(4000.0),
            Transform::from_xyz(x, 3.5, -5.0),
        ));
    }

    // === INFO TEXT ===
    commands.spawn((
        Text::new(
            "NATIVE: Per-fragment photometric lookup\n\
             2 road luminaires + 3 downlights = 5 lights total\n\
             Exact angular distribution from IES profiles\n\
             ColorTemperature: road=3000K, downlights=4000K",
        ),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    // Camera — use high exposure for outdoor night scene
    commands.spawn((
        Camera3d::default(),
        Exposure::INDOOR,
        Transform::from_xyz(8.0, 5.0, 12.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        CameraController,
    ));
}

fn rotate_camera(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<CameraController>>,
) {
    for mut transform in &mut query {
        let angle = time.elapsed_secs() * 0.1;
        let radius = 15.0;
        transform.translation.x = angle.cos() * radius;
        transform.translation.z = angle.sin() * radius;
        transform.look_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
    }
}
