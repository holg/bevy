//! Photometric lighting comparison — NATIVE (per-fragment GPU lookup)
//!
//! Same road scene as the baseline, but each luminaire is a single PointLight
//! with a PhotometricLight component referencing the ACME road luminaire LDT.
//! The angular intensity distribution is sampled per-fragment on the GPU.
//!
//! Compare with `photometric_comparison_baseline`:
//! - 6 lights instead of 30
//! - Exact angular distribution from real measured data
//! - ColorTemperature component instead of manual RGB
//!
//! Controls:
//! - Mouse: orbit camera
//! - Scroll: zoom
//!
//! Run with: cargo run --example photometric_comparison_native \
//!           --features bevy/pbr_photometric_lights,bevy/pbr_clustered_decals

use std::f32::consts::PI;

use bevy::{
    light::{ColorTemperature, PhotometricLight, PhotometricPlugin},
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhotometricPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, orbit_camera)
        .run();
}

#[derive(Component)]
struct OrbitCamera {
    focus: Vec3,
    radius: f32,
    yaw: f32,
    pitch: f32,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let road_length = 60.0;
    let lane_width = 3.5;
    let num_lanes = 2;
    let sidewalk_width = 2.0;
    let road_width = num_lanes as f32 * lane_width;
    let _total_width = road_width + 2.0 * sidewalk_width;
    let mounting_height = 8.0;
    let pole_spacing = mounting_height * 3.5; // EN 13201

    // --- Materials ---
    let road_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.15, 0.15),
        perceptual_roughness: 0.9,
        ..default()
    });
    let sidewalk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.55, 0.55),
        perceptual_roughness: 0.8,
        ..default()
    });
    let marking_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(0.2, 0.2, 0.2, 1.0),
        ..default()
    });
    let pole_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.4, 0.4),
        metallic: 0.7,
        ..default()
    });

    // --- Road surface ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(road_width, road_length))),
        MeshMaterial3d(road_mat),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // --- Sidewalks ---
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(sidewalk_width, 0.15, road_length))),
        MeshMaterial3d(sidewalk_mat.clone()),
        Transform::from_xyz(-(road_width / 2.0 + sidewalk_width / 2.0), 0.075, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(sidewalk_width, 0.15, road_length))),
        MeshMaterial3d(sidewalk_mat),
        Transform::from_xyz(road_width / 2.0 + sidewalk_width / 2.0, 0.075, 0.0),
    ));

    // --- Center line markings ---
    let dash_len = 3.0;
    let gap_len = 4.0;
    let mut z = -road_length / 2.0 + 1.0;
    while z < road_length / 2.0 - 1.0 {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.12, 0.015, dash_len))),
            MeshMaterial3d(marking_mat.clone()),
            Transform::from_xyz(0.0, 0.008, z + dash_len / 2.0),
        ));
        z += dash_len + gap_len;
    }

    // --- Edge lines ---
    for side in [-1.0, 1.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.12, 0.015, road_length - 2.0))),
            MeshMaterial3d(marking_mat.clone()),
            Transform::from_xyz(side * (road_width / 2.0 - 0.15), 0.008, 0.0),
        ));
    }

    // Load ACME road luminaire profile (real measured LDT data)
    let road_profile = asset_server.load("photometric/acme_road.ldt");

    // === NATIVE PHOTOMETRIC LUMINAIRES (staggered) ===
    let mut pole_z = -road_length / 2.0 + pole_spacing / 2.0;
    let mut pole_idx = 0;
    while pole_z < road_length / 2.0 {
        let side = if pole_idx % 2 == 0 { -1.0 } else { 1.0 };
        let pole_x = side * (road_width / 2.0 + 0.5);

        // Pole
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.06, mounting_height))),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_xyz(pole_x, mounting_height / 2.0, pole_z),
        ));

        // Arm extending toward road center
        let arm_length = 1.5;
        let toward_center = -side; // flip sign to go toward center
        let arm_x = pole_x + toward_center * arm_length / 2.0;
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.03, arm_length))),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_xyz(arm_x, mounting_height, pole_z)
                .with_rotation(Quat::from_rotation_z(PI / 2.0)),
        ));

        // Luminaire housing (over the road)
        let housing_x = pole_x + toward_center * arm_length;
        let housing_pos = Vec3::new(housing_x, mounting_height, pole_z);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.6, 0.08, 0.3))),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_translation(housing_pos),
        ));

        // === SINGLE LIGHT with photometric profile ===
        commands.spawn((
            PointLight {
                intensity: 300_000.0,
                range: 25.0,
                shadow_maps_enabled: pole_idx == 0,
                ..default()
            },
            PhotometricLight {
                profile: road_profile.clone(),
            },
            // 2000K SON-TPP lamp (from the LDT metadata)
            ColorTemperature::new(2000.0),
            Transform::from_translation(housing_pos),
        ));

        pole_z += pole_spacing;
        pole_idx += 1;
    }

    // === INFO TEXT ===
    commands.spawn((
        Text::new(format!(
            "NATIVE: Per-fragment photometric lookup\n\
             {} luminaires x 1 light = {} lights total\n\
             ACME Road Runner LDT (real measured data)\n\
             ColorTemperature: 2000K (SON-TPP lamp)\n\
             Staggered poles, {}m spacing",
            pole_idx, pole_idx, pole_spacing
        )),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-12.0, 10.0, 20.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
        OrbitCamera {
            focus: Vec3::new(0.0, 2.0, 0.0),
            radius: 25.0,
            yaw: -0.5,
            pitch: 0.4,
        },
    ));
}

fn orbit_camera(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut OrbitCamera)>,
) {
    for (mut transform, mut orbit) in &mut query {
        orbit.yaw += time.delta_secs() * 0.08;

        let x = orbit.focus.x + orbit.radius * orbit.pitch.cos() * orbit.yaw.cos();
        let y = orbit.focus.y + orbit.radius * orbit.pitch.sin();
        let z = orbit.focus.z + orbit.radius * orbit.pitch.cos() * orbit.yaw.sin();
        transform.translation = Vec3::new(x, y, z);
        transform.look_at(orbit.focus, Vec3::Y);
    }
}
