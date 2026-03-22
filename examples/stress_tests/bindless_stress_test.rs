use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::{PresentMode, WindowResolution},
};
use std::time::Duration;

const GRID_SIZE: u32 = 50;  // 50x50 = 2500 unique materials
const SPHERE_SUBDIVISIONS: u32 = 64; // high-poly spheres

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Bindless Stress Test".into(),
                    resolution: WindowResolution::new(1920, 1080)
                        .with_scale_factor_override(1.0),
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            }),
            FrameTimeDiagnosticsPlugin::default(),
            LogDiagnosticsPlugin {
                wait_duration: Duration::from_secs(3),
                ..default()
            },
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, rotate_camera)
        .run();
}

#[derive(Component)]
struct BenchCamera;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mesh = meshes.add(Sphere::new(0.4).mesh().ico(SPHERE_SUBDIVISIONS.try_into().unwrap()).unwrap());

    // 2500 entities, each with a unique material + unique texture
    for x in 0..GRID_SIZE {
        for z in 0..GRID_SIZE {
            // Unique procedural texture per material
            let texture = images.add(create_texture(x, z));

            let material = materials.add(StandardMaterial {
                base_color: Color::hsl(
                    (x as f32 / GRID_SIZE as f32) * 360.0,
                    0.7 + 0.3 * (z as f32 / GRID_SIZE as f32),
                    0.5,
                ),
                base_color_texture: Some(texture),
                metallic: x as f32 / GRID_SIZE as f32,
                perceptual_roughness: z as f32 / GRID_SIZE as f32,
                ..default()
            });

            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(x as f32 - GRID_SIZE as f32 / 2.0, 0.0, z as f32 - GRID_SIZE as f32 / 2.0),
            ));
        }
    }

    // Many point lights
    for i in 0..32 {
        let angle = i as f32 * std::f32::consts::TAU / 32.0;
        let r = GRID_SIZE as f32 * 0.3;
        commands.spawn((
            PointLight {
                intensity: 500000.0,
                range: 50.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(r * angle.cos(), 8.0, r * angle.sin()),
        ));
    }

    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 20000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.4, 0.0)),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 30.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
        BenchCamera,
    ));
}

fn create_texture(x: u32, z: u32) -> Image {
    let size = 64;
    let mut data = vec![0u8; size * size * 4];
    for py in 0..size {
        for px in 0..size {
            let i = (py * size + px) * 4;
            // Procedural pattern unique per material
            let checker = ((px / 8 + py / 8 + x as usize) % 2 == 0) as u8 * 200;
            let stripe = ((py + z as usize * 3) % 16 < 8) as u8 * 55;
            data[i] = checker.wrapping_add((x as u8).wrapping_mul(7));
            data[i + 1] = stripe.wrapping_add((z as u8).wrapping_mul(11));
            data[i + 2] = ((px as u8).wrapping_mul(x as u8 + 1)).wrapping_add(checker / 2);
            data[i + 3] = 255;
        }
    }
    Image::new(
        Extent3d {
            width: size as u32,
            height: size as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    )
}

fn rotate_camera(time: Res<Time>, mut query: Query<&mut Transform, With<BenchCamera>>) {
    for mut transform in &mut query {
        let elapsed = time.elapsed_secs();
        let radius = 25.0 + 10.0 * (elapsed * 0.2).sin();
        let angle = elapsed * 0.3;
        let height = 15.0 + 10.0 * (elapsed * 0.15).sin();
        transform.translation = Vec3::new(radius * angle.cos(), height, radius * angle.sin());
        transform.look_at(Vec3::ZERO, Vec3::Y);
    }
}
