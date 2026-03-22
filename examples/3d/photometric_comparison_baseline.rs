//! Photometric lighting comparison — BASELINE (multi-spot workaround)
//!
//! Run with: cargo run --example photometric_comparison_baseline

use bevy::{
    color::palettes::css::*,
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
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
) {
    // Ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 20.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.35),
            perceptual_roughness: 0.9,
            ..default()
        })),
    ));

    // Back wall
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(20.0, 6.0, 0.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.8, 0.8),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, 3.0, -5.0),
    ));

    // Cube on ground
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: DEEP_PINK.into(),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));

    // Warm white color (approx 3000K)
    let warm_white = Color::srgb(1.0, 0.82, 0.58);

    // === MULTI-SPOT WORKAROUND ===
    // 5 lights to approximate one road luminaire
    let pos = Vec3::new(0.0, 4.0, 0.0);

    // 1. Ambient fill
    commands.spawn((
        PointLight {
            color: warm_white,
            intensity: 100_000.0,
            range: 15.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(pos),
    ));

    // 2. Main downward spot
    commands.spawn((
        SpotLight {
            color: warm_white,
            intensity: 100_000.0,
            range: 15.0,
            outer_angle: 1.0,
            inner_angle: 0.5,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_translation(pos).looking_at(Vec3::ZERO, Vec3::Z),
    ));

    // 3. Forward throw
    commands.spawn((
        SpotLight {
            color: warm_white,
            intensity: 120_000.0,
            range: 15.0,
            outer_angle: 1.2,
            inner_angle: 0.4,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(pos)
            .looking_at(pos + Vec3::new(0.0, -3.0, 4.0), Vec3::Y),
    ));

    // 4. Backward throw
    commands.spawn((
        SpotLight {
            color: warm_white,
            intensity: 60_000.0,
            range: 12.0,
            outer_angle: 1.0,
            inner_angle: 0.3,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(pos)
            .looking_at(pos + Vec3::new(0.0, -3.0, -3.0), Vec3::Y),
    ));

    // 5. Upward spill
    commands.spawn((
        PointLight {
            color: warm_white,
            intensity: 20_000.0,
            range: 6.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y + 0.2, pos.z),
    ));

    // Info text
    commands.spawn((
        Text::new(
            "BASELINE: Multi-spot workaround\n\
             1 luminaire = 5 lights\n\
             Approximated angular distribution",
        ),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    // Camera — same setup as Bevy's lighting example
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        CameraController,
    ));
}

fn rotate_camera(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<CameraController>>,
) {
    for mut transform in &mut query {
        let angle = time.elapsed_secs() * 0.15;
        let radius = 8.0;
        transform.translation.x = angle.cos() * radius;
        transform.translation.z = angle.sin() * radius;
        transform.translation.y = 3.0;
        transform.look_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y);
    }
}
