//! Photometric Lighting Comparison
//!
//! Controls:
//!   1-5     — Mode: Plain / Multi-spot (eulumdat-bevy) / Native / Side-by-side / Unity/UE
//!   B       — Toggle bollards (vertical reference objects)
//!   G       — Toggle building facades (side walls)
//!   P       — Toggle person-scale figures
//!   WASD    — Pan camera
//!   Arrows  — Orbit camera
//!   R/F     — Zoom in/out
//!   Space   — Toggle auto-orbit
//!
//! Run: cargo run --manifest-path bench/photometric_comparison/Cargo.toml

use std::f32::consts::PI;

use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    light::{ColorTemperature, PhotometricLight, PhotometricPlugin,
            photometric::{LdtData, parse_ldt, sample_ldt}},
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
enum RenderMode {
    Plain,
    MultiSpot,
    Native,
    SideBySide,
    /// Single SpotLight per luminaire with beam angle from IES — typical Unity/Unreal approach
    CubemapCookie,
}
impl Default for RenderMode { fn default() -> Self { RenderMode::SideBySide } }

/// Toggleable visualization helpers.
#[derive(Resource)]
struct VisHelpers {
    bollards: bool,
    facades: bool,
    persons: bool,
    heatmap: bool,
}
impl Default for VisHelpers {
    fn default() -> Self { Self { bollards: true, facades: true, persons: true, heatmap: false } }
}

/// Despawned on mode/vis switch.
#[derive(Component)]
struct SceneEntity;

#[derive(Component)]
struct InfoText;

#[derive(Component)]
struct FpsText;

#[derive(Component)]
struct OrbitCamera { focus: Vec3, radius: f32, yaw: f32, pitch: f32, auto_orbit: bool }

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhotometricPlugin, FrameTimeDiagnosticsPlugin::default()))
        .init_resource::<RenderMode>()
        .init_resource::<VisHelpers>()
        .add_systems(Startup, setup_camera)
        .add_systems(Update, (handle_input, orbit_camera, update_fps))
        .add_systems(Update, rebuild_scene.run_if(
            resource_changed::<RenderMode>.or_else(resource_changed::<VisHelpers>)
        ))
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Text::new("Loading..."),
        Node { position_type: PositionType::Absolute, top: Val::Px(12.0), left: Val::Px(12.0), ..default() },
        InfoText,
    ));
    commands.spawn((
        Text::new("FPS: --"),
        Node { position_type: PositionType::Absolute, top: Val::Px(12.0), right: Val::Px(12.0), ..default() },
        FpsText,
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-15.0, 12.0, 25.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
        OrbitCamera { focus: Vec3::new(0.0, 2.0, 0.0), radius: 40.0, yaw: -0.5, pitch: 0.4, auto_orbit: true },
    ));
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<RenderMode>,
    mut vis: ResMut<VisHelpers>,
) {
    if keys.just_pressed(KeyCode::Digit1) { *mode = RenderMode::Plain; }
    else if keys.just_pressed(KeyCode::Digit2) { *mode = RenderMode::MultiSpot; }
    else if keys.just_pressed(KeyCode::Digit3) { *mode = RenderMode::Native; }
    else if keys.just_pressed(KeyCode::Digit4) { *mode = RenderMode::SideBySide; }
    else if keys.just_pressed(KeyCode::Digit5) { *mode = RenderMode::CubemapCookie; }

    if keys.just_pressed(KeyCode::KeyB) { vis.bollards = !vis.bollards; }
    if keys.just_pressed(KeyCode::KeyG) { vis.facades = !vis.facades; }
    if keys.just_pressed(KeyCode::KeyP) { vis.persons = !vis.persons; }
    if keys.just_pressed(KeyCode::KeyH) { vis.heatmap = !vis.heatmap; }
}

/// Rebuild everything on mode or vis change.
fn rebuild_scene(
    mut commands: Commands,
    mode: Res<RenderMode>,
    vis: Res<VisHelpers>,
    old: Query<Entity, With<SceneEntity>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut text_query: Query<&mut Text, With<InfoText>>,
) {
    for e in &old { commands.entity(e).despawn(); }

    let profile = asset_server.load("photometric/acme_road.ldt");
    let warm = Color::srgb(1.0, 0.72, 0.42);

    let vis_line = format!(
        "B:bollards[{}] G:facades[{}] P:persons[{}] H:heatmap[{}]",
        if vis.bollards { "ON" } else { "off" },
        if vis.facades { "ON" } else { "off" },
        if vis.persons { "ON" } else { "off" },
        if vis.heatmap { "ON" } else { "off" },
    );

    match *mode {
        RenderMode::SideBySide => {
            let gap = 18.0;
            let (_, l1) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, -gap * 1.5, RenderMode::Plain, warm, &profile, "Plain", &vis);
            let (_, l2) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, -gap * 0.5, RenderMode::CubemapCookie, warm, &profile, "Cookie", &vis);
            let (_, l3) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, gap * 0.5, RenderMode::MultiSpot, warm, &profile, "Multi-spot", &vis);
            let (_, l4) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, gap * 1.5, RenderMode::Native, warm, &profile, "Native", &vis);
            for mut t in &mut text_query {
                *t = Text::new(format!(
                    "SIDE-BY-SIDE: Plain({l1}) | Unity/UE({l2}) | Multi-spot({l3}) | Native({l4}) lights\n\
                     1-5:mode WASD:pan Arrows:orbit R/F:zoom Space:auto-orbit\n\
                     {vis_line}"
                ));
            }
        }
        _ => {
            let label = match *mode {
                RenderMode::Plain => "PLAIN",
                RenderMode::MultiSpot => "MULTI-SPOT (eulumdat-bevy)",
                RenderMode::Native => "NATIVE",
                RenderMode::CubemapCookie => "UNITY/UNREAL (single spot from beam angle)",
                _ => unreachable!(),
            };
            let (pi, tl) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, 0.0, *mode, warm, &profile, label, &vis);
            for mut t in &mut text_query {
                *t = Text::new(format!(
                    "{label}: {pi} luminaires, {tl} lights\n\
                     1-4:mode WASD:pan Arrows:orbit R/F:zoom Space:auto-orbit\n\
                     {vis_line}"
                ));
            }
        }
    }
}

/// Spawn a complete road strip with all visualization helpers.
fn spawn_road_strip(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    x_offset: f32,
    mode: RenderMode,
    warm: Color,
    profile: &Handle<bevy::light::PhotometricProfile>,
    _label: &str,
    vis: &VisHelpers,
) -> (u32, u32) {
    let rw = road_width();

    // --- Materials ---
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
    let white_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.9, 0.9),
        perceptual_roughness: 0.5, ..default()
    });
    let facade_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.72, 0.68),
        perceptual_roughness: 0.85, ..default()
    });
    let skin_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.65, 0.5),
        perceptual_roughness: 0.7, ..default()
    });
    let clothing_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.25, 0.4),
        perceptual_roughness: 0.6, ..default()
    });

    // --- Road surface ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(rw, ROAD_LENGTH))),
        MeshMaterial3d(road_mat),
        Transform::from_xyz(x_offset, 0.0, 0.0),
        SceneEntity,
    ));

    // --- Sidewalks ---
    for side in [-1.0f32, 1.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(SIDEWALK_WIDTH, 0.15, ROAD_LENGTH))),
            MeshMaterial3d(sidewalk_mat.clone()),
            Transform::from_xyz(x_offset + side * (rw / 2.0 + SIDEWALK_WIDTH / 2.0), 0.075, 0.0),
            SceneEntity,
        ));
    }

    // --- Center dashes ---
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

    // --- Edge lines ---
    for side in [-1.0f32, 1.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.12, 0.015, ROAD_LENGTH - 2.0))),
            MeshMaterial3d(marking_mat.clone()),
            Transform::from_xyz(x_offset + side * (rw / 2.0 - 0.15), 0.008, 0.0),
            SceneEntity,
        ));
    }

    // --- Building facades (G toggle) ---
    if vis.facades {
        // One side of the road — a long building wall
        let facade_x = x_offset + rw / 2.0 + SIDEWALK_WIDTH + 0.1;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.2, 6.0, ROAD_LENGTH * 0.8))),
            MeshMaterial3d(facade_mat.clone()),
            Transform::from_xyz(facade_x, 3.0, 0.0),
            SceneEntity,
        ));
        // Window-like indentations (pillars every 4m)
        let mut wz = -ROAD_LENGTH * 0.35;
        while wz < ROAD_LENGTH * 0.35 {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.3, 6.0, 0.3))),
                MeshMaterial3d(facade_mat.clone()),
                Transform::from_xyz(facade_x + 0.15, 3.0, wz),
                SceneEntity,
            ));
            wz += 4.0;
        }
    }

    // --- Bollards (B toggle) ---
    if vis.bollards {
        // White bollards along both edges of the road at regular intervals
        let bollard_spacing = 5.0;
        let mut bz = -ROAD_LENGTH / 2.0 + 2.0;
        while bz < ROAD_LENGTH / 2.0 - 2.0 {
            for side in [-1.0f32, 1.0] {
                let bx = x_offset + side * (rw / 2.0 - 0.3);
                // Bollard post
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.05, 1.0))),
                    MeshMaterial3d(white_mat.clone()),
                    Transform::from_xyz(bx, 0.5, bz),
                    SceneEntity,
                ));
                // Reflective top
                commands.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.07))),
                    MeshMaterial3d(white_mat.clone()),
                    Transform::from_xyz(bx, 1.0, bz),
                    SceneEntity,
                ));
            }
            bz += bollard_spacing;
        }
    }

    // --- Person figures (P toggle) ---
    if vis.persons {
        // Simple person = capsule body + sphere head, standing at key positions
        let person_positions = [
            Vec3::new(x_offset - rw / 4.0, 0.0, -5.0),   // on road, left lane
            Vec3::new(x_offset + rw / 4.0, 0.0, 5.0),    // on road, right lane
            Vec3::new(x_offset - rw / 2.0 - 1.0, 0.15, 0.0), // on sidewalk
            Vec3::new(x_offset + rw / 2.0 + 1.0, 0.15, 10.0), // on sidewalk
        ];
        for pp in &person_positions {
            // Body (capsule approximated as cylinder)
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.2, 1.2))),
                MeshMaterial3d(clothing_mat.clone()),
                Transform::from_xyz(pp.x, pp.y + 0.7, pp.z),
                SceneEntity,
            ));
            // Head
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(0.12))),
                MeshMaterial3d(skin_mat.clone()),
                Transform::from_xyz(pp.x, pp.y + 1.45, pp.z),
                SceneEntity,
            ));
            // Legs (two thin cylinders)
            for leg_side in [-0.08f32, 0.08] {
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.06, 0.8))),
                    MeshMaterial3d(clothing_mat.clone()),
                    Transform::from_xyz(pp.x + leg_side, pp.y + 0.0 + 0.1, pp.z),
                    SceneEntity,
                ));
            }
        }
    }

    // --- Poles + lights ---
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

        // === Lights ===
        match mode {
            RenderMode::Plain | RenderMode::SideBySide => {
                commands.spawn((
                    PointLight { color: warm, intensity: 200_000.0, range: 20.0, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(pos), SceneEntity,
                ));
                total_lights += 1;
            }
            RenderMode::CubemapCookie => {
                // Unity/Unreal approach: single SpotLight with beam angle from IES.
                // Collapses the full 2D angular distribution to a single cone angle.
                // Loses all asymmetry — a road luminaire looks like a symmetric downlight.
                commands.spawn((
                    SpotLight {
                        color: warm,
                        intensity: 300_000.0,
                        range: 20.0,
                        // Beam angle ~70° (typical IES beam angle for a road luminaire)
                        // but the real distribution is highly asymmetric — this loses that
                        outer_angle: 1.22, // ~70 degrees
                        inner_angle: 0.52, // ~30 degrees
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(pos)
                        .looking_at(pos - Vec3::Y * 5.0, Vec3::Z),
                    SceneEntity,
                ));
                total_lights += 1;
            }
            RenderMode::MultiSpot => {
                commands.spawn((PointLight { color: warm, intensity: 80_000.0, range: 20.0, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(pos), SceneEntity));
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
                    Transform::from_translation(pos), SceneEntity,
                ));
                total_lights += 1;
            }
        }

        pz += POLE_SPACING;
        pi += 1;
    }

    // --- Ground heatmap (H toggle) ---
    if vis.heatmap {
        // Parse LDT for native mode sampling
        let ldt_bytes = include_bytes!("../../../assets/photometric/acme_road.ldt");
        let ldt_text = String::from_utf8_lossy(ldt_bytes);
        let ldt_data = parse_ldt(&ldt_text).ok();

        // Collect luminaire positions
        let mut light_positions: Vec<(Vec3, f32)> = Vec::new(); // (pos, side)
        let mut lpz = -ROAD_LENGTH / 2.0 + POLE_SPACING / 2.0;
        let mut lpi = 0u32;
        while lpz < ROAD_LENGTH / 2.0 {
            let lside: f32 = if lpi % 2 == 0 { -1.0 } else { 1.0 };
            let lpx = x_offset + lside * (rw / 2.0 + 0.5);
            let ltc = -lside;
            let larm = 1.5;
            light_positions.push((Vec3::new(lpx + ltc * larm, MOUNTING_HEIGHT, lpz), lside));
            lpz += POLE_SPACING;
            lpi += 1;
        }

        let grid_x = 30usize;
        let grid_z = 60usize;
        let total_w = rw + 2.0 * SIDEWALK_WIDTH;
        let cell_w = total_w / grid_x as f32;
        let cell_h = ROAD_LENGTH / grid_z as f32;

        let mut lux_grid = vec![0.0f32; grid_x * grid_z];
        for zi in 0..grid_z {
            for xi in 0..grid_x {
                let gx = x_offset - total_w / 2.0 + (xi as f32 + 0.5) * cell_w;
                let gz = -ROAD_LENGTH / 2.0 + (zi as f32 + 0.5) * cell_h;
                let ground_pt = Vec3::new(gx, 0.0, gz);

                let mut total_lux = 0.0f32;
                for (lp, _side) in &light_positions {
                    let to_light = *lp - ground_pt;
                    let dist = to_light.length();
                    if dist < 0.1 { continue; }
                    let dist_sq = dist * dist;
                    let cos_incidence = to_light.y / dist;
                    if cos_incidence <= 0.0 { continue; }

                    let intensity = match mode {
                        RenderMode::Native => {
                            // Sample actual LDT angular distribution
                            if let Some(ref ldt) = ldt_data {
                                let dir = -to_light / dist; // light-to-ground direction
                                // gamma = angle from nadir (downward = -Y)
                                let gamma = (-dir.y).acos().to_degrees();
                                // C = azimuthal angle
                                let c = dir.x.atan2(dir.z).to_degrees();
                                let c = if c < 0.0 { c + 360.0 } else { c };
                                sample_ldt(ldt, c, gamma) as f32 * 100.0
                            } else {
                                10000.0 // fallback
                            }
                        }
                        RenderMode::MultiSpot => {
                            // Approximate 5-spot contribution
                            let dir = -to_light / dist;
                            let down_dot = -dir.y; // alignment with straight down
                            let along_road = dir.z.abs();
                            // Crude multi-spot envelope
                            let spot_main = (down_dot * 2.0).clamp(0.0, 1.0).powi(3);
                            let spot_throw = (along_road * 1.5).clamp(0.0, 1.0).powi(2);
                            (spot_main * 5000.0 + spot_throw * 8000.0)
                        }
                        RenderMode::CubemapCookie => {
                            // Single spot cone approximation (Unity/Unreal style)
                            let dir = -to_light / dist;
                            let down_dot = -dir.y;
                            // Spot cone: ~70° outer, ~30° inner
                            let cone = ((down_dot - 0.34) / (0.87 - 0.34)).clamp(0.0, 1.0);
                            cone * cone * 15000.0
                        }
                        RenderMode::Plain | RenderMode::SideBySide => {
                            // Uniform point light
                            10000.0
                        }
                    };

                    // E = I * cos(incidence) / d²
                    total_lux += intensity * cos_incidence / dist_sq;
                }
                lux_grid[zi * grid_x + xi] = total_lux;
            }
        }

        let max_lux = lux_grid.iter().cloned().fold(0.0f32, f32::max).max(0.001);

        for zi in 0..grid_z {
            for xi in 0..grid_x {
                let val = (lux_grid[zi * grid_x + xi] / max_lux).clamp(0.0, 1.0);
                if val < 0.01 { continue; }

                let (r, g, b) = heatmap_color(val);
                let gx = x_offset - total_w / 2.0 + (xi as f32 + 0.5) * cell_w;
                let gz = -ROAD_LENGTH / 2.0 + (zi as f32 + 0.5) * cell_h;

                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgba(r, g, b, 0.6),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                });
                commands.spawn((
                    Mesh3d(meshes.add(Plane3d::default().mesh().size(cell_w * 0.95, cell_h * 0.95))),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(gx, 0.02, gz),
                    SceneEntity,
                ));
            }
        }
    }

    (pi, total_lights)
}

fn update_fps(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<&mut Text, With<FpsText>>,
) {
    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(avg) = fps.smoothed() {
            for mut text in &mut query {
                *text = Text::new(format!("FPS: {avg:.0}"));
            }
        }
    }
}

/// Map a 0..1 value to a heatmap color (blue → cyan → green → yellow → red).
fn heatmap_color(val: f32) -> (f32, f32, f32) {
    let v = val.clamp(0.0, 1.0);
    if v < 0.25 {
        let t = v / 0.25;
        (0.0, t, 1.0) // blue → cyan
    } else if v < 0.5 {
        let t = (v - 0.25) / 0.25;
        (0.0, 1.0, 1.0 - t) // cyan → green
    } else if v < 0.75 {
        let t = (v - 0.5) / 0.25;
        (t, 1.0, 0.0) // green → yellow
    } else {
        let t = (v - 0.75) / 0.25;
        (1.0, 1.0 - t, 0.0) // yellow → red
    }
}

fn orbit_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<(&mut Transform, &mut OrbitCamera)>,
) {
    let dt = time.delta_secs();

    for (mut t, mut o) in &mut q {
        if keys.just_pressed(KeyCode::Space) { o.auto_orbit = !o.auto_orbit; }

        let orbit_speed = 1.5 * dt;
        if keys.pressed(KeyCode::ArrowLeft) { o.yaw += orbit_speed; }
        if keys.pressed(KeyCode::ArrowRight) { o.yaw -= orbit_speed; }
        if keys.pressed(KeyCode::ArrowUp) { o.pitch = (o.pitch + orbit_speed * 0.5).clamp(0.05, 1.4); }
        if keys.pressed(KeyCode::ArrowDown) { o.pitch = (o.pitch - orbit_speed * 0.5).clamp(0.05, 1.4); }

        if keys.pressed(KeyCode::KeyR) { o.radius = (o.radius - 15.0 * dt).clamp(5.0, 80.0); }
        if keys.pressed(KeyCode::KeyF) { o.radius = (o.radius + 15.0 * dt).clamp(5.0, 80.0); }

        let forward = Vec3::new(-o.yaw.sin(), 0.0, -o.yaw.cos());
        let right = Vec3::new(o.yaw.cos(), 0.0, -o.yaw.sin());
        let speed = 10.0 * dt;
        if keys.pressed(KeyCode::KeyW) { o.focus += forward * speed; }
        if keys.pressed(KeyCode::KeyS) { o.focus -= forward * speed; }
        if keys.pressed(KeyCode::KeyA) { o.focus -= right * speed; }
        if keys.pressed(KeyCode::KeyD) { o.focus += right * speed; }
        if keys.pressed(KeyCode::KeyQ) { o.focus.y -= speed; }
        if keys.pressed(KeyCode::KeyE) { o.focus.y += speed; }

        if o.auto_orbit { o.yaw += dt * 0.05; }

        let x = o.focus.x + o.radius * o.pitch.cos() * o.yaw.cos();
        let y = o.focus.y + o.radius * o.pitch.sin();
        let z = o.focus.z + o.radius * o.pitch.cos() * o.yaw.sin();
        t.translation = Vec3::new(x, y, z);
        t.look_at(o.focus, Vec3::Y);
    }
}
