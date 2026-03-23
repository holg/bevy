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
    camera::primitives::CubemapLayout,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    image::Image,
    light::{ColorTemperature, PhotometricLight, PhotometricPlugin, PointLightTexture,
            photometric::{LdtData, parse_ldt, sample_ldt}},
    prelude::*,
    asset::RenderAssetUsages,
};
// gldf-rs available for future L3D model loading (native only)
#[cfg(not(target_arch = "wasm32"))]
#[allow(unused_imports)]
use gldf_rs::{GldfProduct, get_first_l3d_with_ldt};

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
impl Default for RenderMode { fn default() -> Self { RenderMode::Native } }

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

/// Available LDT profiles with labels.
#[derive(Resource)]
struct LdtLibrary {
    profiles: Vec<(String, String)>, // (label, asset_path)
    current: usize,
}

impl Default for LdtLibrary {
    fn default() -> Self {
        Self {
            profiles: vec![
                ("ACME Road Runner".into(), "photometric/acme_road.ldt".into()),
                ("BGP307 DM10 (LED84)".into(), "photometric/BGP307-LED84-4S_830-PSA-DM10.ldt".into()),
                ("BGP307 DRN2 (LED84)".into(), "photometric/BGP307-LED84-4S_830-PSA-DRN2.ldt".into()),
                ("BGP307 DX70 (LED84)".into(), "photometric/BGP307-LED84-4S_830-PSA-DX70.ldt".into()),
                ("BGP307 DM10 (LED99)".into(), "photometric/BGP307-LED99-4S_830-PSA-DM10.ldt".into()),
                ("BGP307 DRN2 (LED99)".into(), "photometric/BGP307-LED99-4S_830-PSA-DRN2.ldt".into()),
                ("BGP307 DX70 (LED99)".into(), "photometric/BGP307-LED99-4S_830-PSA-DX70.ldt".into()),
            ],
            current: 0,
        }
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhotometricPlugin, FrameTimeDiagnosticsPlugin::default()))
        .init_resource::<RenderMode>()
        .init_resource::<VisHelpers>()
        .init_resource::<LdtLibrary>()
        .add_systems(Startup, setup_camera)
        .add_systems(Update, (handle_input, orbit_camera, update_fps))
        .add_systems(Update, rebuild_scene.run_if(
            resource_changed::<RenderMode>
                .or_else(resource_changed::<VisHelpers>)
                .or_else(resource_changed::<LdtLibrary>)
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
    mut ldt_lib: ResMut<LdtLibrary>,
) {
    if keys.just_pressed(KeyCode::Digit1) { *mode = RenderMode::Plain; }
    else if keys.just_pressed(KeyCode::Digit2) { *mode = RenderMode::MultiSpot; }
    else if keys.just_pressed(KeyCode::Digit3) { *mode = RenderMode::Native; }
    else if keys.just_pressed(KeyCode::Digit4) { *mode = RenderMode::CubemapCookie; }
    else if keys.just_pressed(KeyCode::Digit5) { *mode = RenderMode::SideBySide; }

    if keys.just_pressed(KeyCode::KeyB) { vis.bollards = !vis.bollards; }
    if keys.just_pressed(KeyCode::KeyG) { vis.facades = !vis.facades; }
    if keys.just_pressed(KeyCode::KeyP) { vis.persons = !vis.persons; }
    if keys.just_pressed(KeyCode::KeyH) { vis.heatmap = !vis.heatmap; }

    // Cycle LDT profiles with [ and ]
    if keys.just_pressed(KeyCode::BracketLeft) {
        let len = ldt_lib.profiles.len();
        ldt_lib.current = (ldt_lib.current + len - 1) % len;
        // Force rebuild by toggling vis (hack — ideally we'd have a separate changed detection)
        vis.set_changed();
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        ldt_lib.current = (ldt_lib.current + 1) % ldt_lib.profiles.len();
        vis.set_changed();
    }
}

/// Rebuild everything on mode or vis change.
fn rebuild_scene(
    mut commands: Commands,
    mode: Res<RenderMode>,
    vis: Res<VisHelpers>,
    ldt_lib: Res<LdtLibrary>,
    old: Query<Entity, With<SceneEntity>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut text_query: Query<&mut Text, With<InfoText>>,
) {
    for e in &old { commands.entity(e).despawn(); }

    let ldt_label = ldt_lib.profiles[ldt_lib.current].0.clone();
    let ldt_path = ldt_lib.profiles[ldt_lib.current].1.clone();
    let profile = asset_server.load::<bevy::light::PhotometricProfile>(ldt_path.clone());
    let warm = Color::srgb(1.0, 0.72, 0.42);

    // Load current LDT for cubemap cookie generation
    let cookie_image = if *mode == RenderMode::CubemapCookie || *mode == RenderMode::SideBySide {
        let full_path = format!("assets/{ldt_path}");
        if let Ok(ldt_bytes) = std::fs::read(&full_path) {
            let ldt_text = String::from_utf8_lossy(&ldt_bytes);
            if let Ok(ldt) = parse_ldt(&ldt_text) {
                Some(images.add(generate_cubemap_cookie(&ldt, 128)))
            } else { None }
        } else { None }
    } else { None };

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
            let (_, l1) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, &mut images, -gap * 1.5, RenderMode::Plain, warm, &profile, None, &ldt_path, "Plain", &vis);
            let (_, l2) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, &mut images, -gap * 0.5, RenderMode::CubemapCookie, warm, &profile, cookie_image.clone(), &ldt_path, "Cookie", &vis);
            let (_, l3) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, &mut images, gap * 0.5, RenderMode::MultiSpot, warm, &profile, None, &ldt_path, "Multi-spot", &vis);
            let (_, l4) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, &mut images, gap * 1.5, RenderMode::Native, warm, &profile, None, &ldt_path, "Native", &vis);
            for mut t in &mut text_query {
                *t = Text::new(format!(
                    "SIDE-BY-SIDE: Plain({l1}) | Unity/UE({l2}) | Multi-spot({l3}) | Native({l4}) lights\n\
                     LDT: {ldt_label} [{}/{}] ([/] to switch)\n\
                     1:Plain 2:Multi-spot 3:Native 4:Unity/UE 5:Side-by-side | WASD Arrows R/F Space\n\
                     {vis_line}",
                    ldt_lib.current + 1, ldt_lib.profiles.len()
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
            let (pi, tl) = spawn_road_strip(&mut commands, &mut meshes, &mut materials, &mut images, 0.0, *mode, warm, &profile, cookie_image.clone(), &ldt_path, label, &vis);
            for mut t in &mut text_query {
                *t = Text::new(format!(
                    "{label}: {pi} luminaires, {tl} lights\n\
                     LDT: {ldt_label} [{}/{}] ([/] to switch)\n\
                     1:Plain 2:Multi-spot 3:Native 4:Unity/UE 5:Side-by-side | WASD Arrows R/F Space\n\
                     {vis_line}",
                    ldt_lib.current + 1, ldt_lib.profiles.len()
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
    images: &mut ResMut<Assets<Image>>,
    x_offset: f32,
    mode: RenderMode,
    warm: Color,
    profile: &Handle<bevy::light::PhotometricProfile>,
    cookie_image: Option<Handle<Image>>,
    ldt_path: &str,
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
    let housing_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.3, 0.3),
        emissive: LinearRgba::new(8.0, 6.0, 3.5, 1.0), // warm glow
        ..default()
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
        // Housing body (dark)
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.6, 0.06, 0.3))),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_xyz(pos.x, pos.y + 0.02, pos.z),
            SceneEntity,
        ));
        // Luminous opening (bottom face — the part that emits light)
        commands.spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(0.55, 0.25))),
            MeshMaterial3d(housing_mat.clone()),
            Transform::from_xyz(pos.x, pos.y - 0.01, pos.z)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
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
                // Unity/Unreal approach: PointLight + cubemap cookie texture.
                // The IES/LDT distribution is baked into a cubemap and projected
                // from the light's perspective. Preserves some angular variation
                // but is fundamentally a projective approach, not per-fragment angular.
                let mut light_cmd = commands.spawn((
                    PointLight {
                        color: warm,
                        intensity: 300_000.0,
                        range: 20.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(pos),
                    SceneEntity,
                ));
                if let Some(ref cookie) = cookie_image {
                    light_cmd.insert(PointLightTexture {
                        image: cookie.clone(),
                        cubemap_layout: CubemapLayout::SequenceVertical,
                    });
                }
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
        // Parse current LDT for heatmap sampling
        let full_ldt_path = format!("assets/{ldt_path}");
        let ldt_data = match std::fs::read(&full_ldt_path) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                match parse_ldt(&text) {
                    Ok(data) => {
                        info!("Heatmap: loaded '{}', {} C-planes, {} gamma angles",
                            ldt_path, data.c_angles.len(), data.gamma_angles.len());
                        Some(data)
                    }
                    Err(e) => {
                        warn!("Heatmap: failed to parse '{}': {}", ldt_path, e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("Heatmap: failed to read '{}': {}", full_ldt_path, e);
                None
            }
        };

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

                    // Compute (C, gamma) for this ground point relative to luminaire
                    let dir = -to_light / dist; // light-to-ground direction
                    let gamma_deg = (-dir.y).acos().to_degrees();
                    let c_deg = dir.x.atan2(dir.z).to_degrees();
                    let c_deg = if c_deg < 0.0 { c_deg + 360.0 } else { c_deg };

                    let intensity = match mode {
                        RenderMode::Native => {
                            // Per-fragment angular lookup — the correct approach
                            if let Some(ref ldt) = ldt_data {
                                sample_ldt(ldt, c_deg, gamma_deg) as f32 * 100.0
                            } else {
                                10000.0
                            }
                        }
                        RenderMode::CubemapCookie => {
                            // Cubemap cookie projection (Unity/Unreal approach).
                            // Sample the LDT via the cubemap UV mapping, which
                            // introduces projective distortion at grazing angles.
                            if let Some(ref ldt) = ldt_data {
                                // Reconstruct the cubemap lookup:
                                // 1. Find which cube face this direction hits
                                // 2. Compute face UV (projective division)
                                // 3. Convert UV back to angular coordinates
                                // The projective division (dividing by the dominant axis)
                                // is where the distortion comes from.
                                let abs_dir = Vec3::new(dir.x.abs(), dir.y.abs(), dir.z.abs());
                                let max_axis = abs_dir.x.max(abs_dir.y).max(abs_dir.z);
                                // Projective UV on the dominant face
                                let (face_u, face_v) = if max_axis == abs_dir.y {
                                    // Y face (most common for downlights)
                                    (dir.x / abs_dir.y, dir.z / abs_dir.y)
                                } else if max_axis == abs_dir.x {
                                    (dir.z / abs_dir.x, dir.y / abs_dir.x)
                                } else {
                                    (dir.x / abs_dir.z, dir.y / abs_dir.z)
                                };
                                // Convert projected UV back to direction and then to angles
                                // This is where the distortion lives — the projected UV
                                // maps to a different (C, gamma) than the original direction
                                let proj_dir = Vec3::new(
                                    face_u * max_axis,
                                    dir.y,
                                    face_v * max_axis,
                                ).normalize();
                                let proj_gamma = (-proj_dir.y).acos().to_degrees();
                                let proj_c = proj_dir.x.atan2(proj_dir.z).to_degrees();
                                let proj_c = if proj_c < 0.0 { proj_c + 360.0 } else { proj_c };
                                sample_ldt(ldt, proj_c, proj_gamma) as f32 * 100.0
                            } else {
                                10000.0
                            }
                        }
                        RenderMode::MultiSpot => {
                            // Simulate the 5-spot workaround as actually spawned:
                            // Each spot has a direction and cone. Compute contribution
                            // from each using spot attenuation (Filament formula).
                            if let Some(ref ldt) = ldt_data {
                                // Spot definitions: (direction, outer_angle, inner_angle, intensity_frac)
                                let spots: [(Vec3, f32, f32, f32); 5] = [
                                    (Vec3::NEG_Y, 1.1, 0.4, 0.25),                          // downward
                                    (Vec3::new(0.0, -5.0, 8.0).normalize(), 1.2, 0.3, 0.30), // road throw
                                    (Vec3::new(0.0, -5.0, -5.0).normalize(), 1.0, 0.3, 0.15),// opposite
                                    (Vec3::new(-_side * 5.0, -6.0, 0.0).normalize(), 0.9, 0.3, 0.20), // cross
                                    (Vec3::NEG_Y, std::f32::consts::PI, 0.0, 0.10),         // ambient fill
                                ];
                                let mut total = 0.0f32;
                                for (spot_dir, outer, inner, frac) in &spots {
                                    let cd = dir.dot(*spot_dir);
                                    let cos_outer = outer.cos();
                                    let cos_inner = inner.cos();
                                    let spot_scale = 1.0 / (cos_inner - cos_outer).max(1e-4);
                                    let spot_offset = -cos_outer * spot_scale;
                                    let atten = (cd * spot_scale + spot_offset).clamp(0.0, 1.0);
                                    let atten = atten * atten;
                                    // Sample LDT at the spot's central direction for base intensity
                                    let spot_gamma = (-spot_dir.y).acos().to_degrees();
                                    let spot_c = spot_dir.x.atan2(spot_dir.z).to_degrees();
                                    let spot_c = if spot_c < 0.0 { spot_c + 360.0 } else { spot_c };
                                    let base_i = sample_ldt(ldt, spot_c, spot_gamma) as f32;
                                    total += atten * base_i * frac;
                                }
                                total * 100.0
                            } else {
                                10000.0
                            }
                        }
                        RenderMode::Plain | RenderMode::SideBySide => {
                            // Uniform point light — no angular variation
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

/// Generate a cubemap cookie texture from LDT data.
/// This is what Unity/Unreal do: bake the photometric distribution into a
/// cubemap and use it as a light cookie (projective texture).
///
/// Returns a packed vertical sequence image (1 column × 6 faces).
fn generate_cubemap_cookie(ldt: &LdtData, face_size: u32) -> Image {
    let width = face_size;
    let height = face_size * 6; // 6 faces stacked vertically
    let mut pixels = vec![0u8; (width * height * 4) as usize]; // RGBA8

    // Face order for SequenceVertical: +X, -X, +Y, -Y, -Z, +Z
    let face_dirs: [(Vec3, Vec3, Vec3); 6] = [
        // (right, up, forward) for each face — matching the shader's cubemap_uv
        (Vec3::Z, -Vec3::Y, Vec3::X),      // +X: face_uv = (z, -y) / x
        (-Vec3::Z, -Vec3::Y, -Vec3::X),     // -X: face_uv = (-z, -y) / -x
        (Vec3::X, -Vec3::Z, Vec3::Y),       // +Y: face_uv = (x, -z) / y
        (Vec3::X, Vec3::Z, -Vec3::Y),       // -Y: face_uv = (x, z) / -y
        (Vec3::X, Vec3::Y, Vec3::Z),        // +Z: face_uv = (x, y) / z  (note: -Z face in cubemap)
        (Vec3::X, -Vec3::Y, -Vec3::Z),      // -Z: face_uv = (x, -y) / -z (note: +Z face in cubemap)
    ];

    // Find max intensity for normalization
    let mut max_val = 0.0f32;
    for face in 0..6u32 {
        let (right, up, forward) = face_dirs[face as usize];
        for py in 0..face_size {
            for px in 0..face_size {
                // Map pixel to [-1, 1] range
                let u = (px as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;
                let v = (py as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;
                let dir = (forward + right * u + up * v).normalize();

                // Convert direction to (C, gamma) angles
                let gamma = (-dir.y).acos().to_degrees(); // 0=nadir, 90=horizontal
                let c = dir.x.atan2(dir.z).to_degrees();
                let c = if c < 0.0 { c + 360.0 } else { c };

                let intensity = sample_ldt(ldt, c, gamma);
                if intensity > max_val { max_val = intensity; }
            }
        }
    }
    if max_val < 0.001 { max_val = 1.0; }

    // Render faces
    for face in 0..6u32 {
        let (right, up, forward) = face_dirs[face as usize];
        let y_offset = face * face_size;

        for py in 0..face_size {
            for px in 0..face_size {
                let u = (px as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;
                let v = (py as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;
                let dir = (forward + right * u + up * v).normalize();

                let gamma = (-dir.y).acos().to_degrees();
                let c = dir.x.atan2(dir.z).to_degrees();
                let c = if c < 0.0 { c + 360.0 } else { c };

                let intensity = sample_ldt(ldt, c, gamma);
                let val = (intensity / max_val * 255.0).clamp(0.0, 255.0) as u8;

                let idx = ((y_offset + py) * width + px) as usize * 4;
                pixels[idx] = val;     // R (only R channel is read)
                pixels[idx + 1] = val; // G
                pixels[idx + 2] = val; // B
                pixels[idx + 3] = 255; // A
            }
        }
    }

    Image::new(
        bevy::render::render_resource::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        pixels,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
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
