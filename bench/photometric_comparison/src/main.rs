//! Photometric Lighting Comparison
//!
//! Controls:
//!   1 — Plain PointLights
//!   2 — Multi-spot workaround (5 lights/luminaire)
//!   3 — Native per-fragment photometric (1 light/luminaire)
//!   4 — Side-by-side: all three on parallel roads
//!
//! Run: cargo run --manifest-path bench/photometric_comparison/Cargo.toml

use std::f32::consts::PI;

use bevy::{
    light::{ColorTemperature, PhotometricLight, PhotometricPlugin},
    prelude::*,
};

const ROAD_LENGTH: f32 = 60.0;
const LANE_WIDTH: f32 = 3.5;
const NUM_LANES: u32 = 2;
const SIDEWALK_WIDTH: f32 = 2.0;
const MOUNTING_HEIGHT: f32 = 8.0;
const POLE_SPACING: f32 = MOUNTING_HEIGHT * 3.5;

fn road_width() -> f32 { NUM_LANES as f32 * LANE_WIDTH }

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
enum RenderMode { Plain, MultiSpot, Native, SideBySide }
impl Default for RenderMode { fn default() -> Self { RenderMode::SideBySide } }

/// Despawned on mode switch.
#[derive(Component)]
struct SceneEntity;

#[derive(Component)]
struct InfoText;

#[derive(Component)]
struct OrbitCamera { focus: Vec3, radius: f32, yaw: f32, pitch: f32 }

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhotometricPlugin))
        .init_resource::<RenderMode>()
        .add_systems(Startup, setup_camera)
        .add_systems(Update, (handle_input, orbit_camera))
        .add_systems(Update, rebuild_scene.run_if(resource_changed::<RenderMode>))
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Text::new("Loading..."),
        Node { position_type: PositionType::Absolute, top: Val::Px(12.0), left: Val::Px(12.0), ..default() },
        InfoText,
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-15.0, 12.0, 25.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
        OrbitCamera { focus: Vec3::new(0.0, 2.0, 0.0), radius: 40.0, yaw: -0.5, pitch: 0.4 },
    ));
}

fn handle_input(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<RenderMode>) {
    if keys.just_pressed(KeyCode::Digit1) { *mode = RenderMode::Plain; }
    else if keys.just_pressed(KeyCode::Digit2) { *mode = RenderMode::MultiSpot; }
    else if keys.just_pressed(KeyCode::Digit3) { *mode = RenderMode::Native; }
    else if keys.just_pressed(KeyCode::Digit4) { *mode = RenderMode::SideBySide; }
}

/// Rebuild everything on mode change.
fn rebuild_scene(
    mut commands: Commands,
    mode: Res<RenderMode>,
    old: Query<Entity, With<SceneEntity>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut text_query: Query<&mut Text, With<InfoText>>,
) {
    for e in &old { commands.entity(e).despawn(); }

    let profile = asset_server.load("photometric/acme_road.ldt");
    let warm = Color::srgb(1.0, 0.72, 0.42);

    match *mode {
        RenderMode::SideBySide => {
            let gap = 20.0;
            let (_, l1) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, -gap, RenderMode::Plain, warm, &profile, "Plain");
            let (_, l2) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, 0.0, RenderMode::MultiSpot, warm, &profile, "Multi-spot");
            let (p3, l3) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, gap, RenderMode::Native, warm, &profile, "Native");
            for mut t in &mut text_query {
                *t = Text::new(format!(
                    "SIDE-BY-SIDE COMPARISON\n\
                     Left: Plain ({l1} lights)  |  Center: Multi-spot ({l2} lights)  |  Right: Native ({l3} lights)\n\
                     Press 1-4 to switch"
                ));
            }
        }
        _ => {
            let label = match *mode {
                RenderMode::Plain => "PLAIN",
                RenderMode::MultiSpot => "MULTI-SPOT",
                RenderMode::Native => "NATIVE",
                _ => unreachable!(),
            };
            let (pi, tl) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, 0.0, *mode, warm, &profile, label);
            for mut t in &mut text_query {
                *t = Text::new(format!(
                    "{label}: {pi} luminaires, {tl} lights\nPress 1-4 to switch"
                ));
            }
        }
    }
}

/// Spawn a complete road strip (geometry + poles + lights) at the given X offset.
fn spawn_road_strip(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    x_offset: f32,
    mode: RenderMode,
    warm: Color,
    profile: &Handle<bevy::light::PhotometricProfile>,
    label: &str,
) -> (u32, u32) {
    let rw = road_width();

    let road_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.15, 0.15),
        perceptual_roughness: 0.9, ..default()
    });
    let sidewalk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.55, 0.55),
        perceptual_roughness: 0.8, ..default()
    });
    let marking_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(0.2, 0.2, 0.2, 1.0), ..default()
    });
    let pole_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.4, 0.4),
        metallic: 0.7, ..default()
    });

    // Road surface
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(rw, ROAD_LENGTH))),
        MeshMaterial3d(road_mat),
        Transform::from_xyz(x_offset, 0.0, 0.0),
        SceneEntity,
    ));

    // Sidewalks
    for side in [-1.0f32, 1.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(SIDEWALK_WIDTH, 0.15, ROAD_LENGTH))),
            MeshMaterial3d(sidewalk_mat.clone()),
            Transform::from_xyz(x_offset + side * (rw / 2.0 + SIDEWALK_WIDTH / 2.0), 0.075, 0.0),
            SceneEntity,
        ));
    }

    // Center dashes
    let mut z = -ROAD_LENGTH / 2.0 + 1.0;
    while z < ROAD_LENGTH / 2.0 - 1.0 {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.12, 0.015, 3.0))),
            MeshMaterial3d(marking_mat.clone()),
            Transform::from_xyz(x_offset, 0.008, z + 1.5),
            SceneEntity,
        ));
        z += 7.0;
    }

    // Edge lines
    for side in [-1.0f32, 1.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.12, 0.015, ROAD_LENGTH - 2.0))),
            MeshMaterial3d(marking_mat.clone()),
            Transform::from_xyz(x_offset + side * (rw / 2.0 - 0.15), 0.008, 0.0),
            SceneEntity,
        ));
    }

    // Label text in 3D (floating above the road)
    // (skip for now — the 2D text overlay shows mode info)

    // Poles + lights
    let mut pz = -ROAD_LENGTH / 2.0 + POLE_SPACING / 2.0;
    let mut pi = 0u32;
    let mut total_lights = 0u32;

    while pz < ROAD_LENGTH / 2.0 {
        let side: f32 = if pi % 2 == 0 { -1.0 } else { 1.0 };
        let px = x_offset + side * (rw / 2.0 + 0.5);
        let tc = -side;
        let arm = 1.5;
        let hx = px + tc * arm;
        let pos = Vec3::new(hx, MOUNTING_HEIGHT, pz);

        // Pole
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.06, MOUNTING_HEIGHT))),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_xyz(px, MOUNTING_HEIGHT / 2.0, pz),
            SceneEntity,
        ));
        // Arm
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.03, arm))),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_xyz(px + tc * arm / 2.0, MOUNTING_HEIGHT, pz)
                .with_rotation(Quat::from_rotation_z(PI / 2.0)),
            SceneEntity,
        ));
        // Housing
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.6, 0.08, 0.3))),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_translation(pos),
            SceneEntity,
        ));

        // Lights
        match mode {
            RenderMode::Plain | RenderMode::SideBySide => {
                // For SideBySide this branch shouldn't be called directly,
                // but handle it as Plain fallback
                commands.spawn((
                    PointLight {
                        color: warm,
                        intensity: 200_000.0,
                        range: 20.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(pos),
                    SceneEntity,
                ));
                total_lights += 1;
            }
            RenderMode::MultiSpot => {
                // 5-light approximation
                commands.spawn((PointLight { color: warm, intensity: 80_000.0, range: 20.0, shadow_maps_enabled: false, ..default() }, Transform::from_translation(pos), SceneEntity));
                commands.spawn((SpotLight { color: warm, intensity: 100_000.0, range: 18.0, outer_angle: 1.1, inner_angle: 0.4, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(pos).looking_at(pos - Vec3::Y * 5.0, Vec3::Z), SceneEntity));
                commands.spawn((SpotLight { color: warm, intensity: 120_000.0, range: 20.0, outer_angle: 1.2, inner_angle: 0.3, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(pos).looking_at(pos + Vec3::new(0.0, -5.0, 8.0), Vec3::Y), SceneEntity));
                commands.spawn((SpotLight { color: warm, intensity: 60_000.0, range: 15.0, outer_angle: 1.0, inner_angle: 0.3, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(pos).looking_at(pos + Vec3::new(0.0, -5.0, -5.0), Vec3::Y), SceneEntity));
                commands.spawn((SpotLight { color: warm, intensity: 80_000.0, range: 15.0, outer_angle: 0.9, inner_angle: 0.3, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(pos).looking_at(pos + Vec3::new(-side * 5.0, -6.0, 0.0), Vec3::Z), SceneEntity));
                total_lights += 5;
            }
            RenderMode::Native => {
                commands.spawn((
                    PointLight { intensity: 300_000.0, range: 25.0, shadow_maps_enabled: false, ..default() },
                    PhotometricLight { profile: profile.clone() },
                    ColorTemperature::new(2000.0),
                    Transform::from_translation(pos),
                    SceneEntity,
                ));
                total_lights += 1;
            }
        }

        pz += POLE_SPACING;
        pi += 1;
    }

    (pi, total_lights)
}

fn orbit_camera(time: Res<Time>, mut q: Query<(&mut Transform, &mut OrbitCamera)>) {
    for (mut t, mut o) in &mut q {
        o.yaw += time.delta_secs() * 0.08;
        let x = o.focus.x + o.radius * o.pitch.cos() * o.yaw.cos();
        let y = o.focus.y + o.radius * o.pitch.sin();
        let z = o.focus.z + o.radius * o.pitch.cos() * o.yaw.sin();
        t.translation = Vec3::new(x, y, z);
        t.look_at(o.focus, Vec3::Y);
    }
}
