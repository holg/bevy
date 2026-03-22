use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    window::{PresentMode, WindowResolution},
};
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Bistro WASM".into(),
                    resolution: WindowResolution::new(1280, 720),
                    present_mode: PresentMode::AutoNoVsync,
                    fit_canvas_to_parent: true,
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

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(SceneRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("BistroExterior_web.glb")),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(10.0, 5.0, 10.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
        BenchCamera,
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 15000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.4, 0.0)),
    ));
}

fn rotate_camera(time: Res<Time>, mut query: Query<&mut Transform, With<BenchCamera>>) {
    for mut transform in &mut query {
        let elapsed = time.elapsed_secs();
        let radius = 12.0 + 4.0 * (elapsed * 0.3).sin();
        let angle = elapsed * 0.5;
        let height = 4.0 + 2.0 * (elapsed * 0.2).sin();
        transform.translation = Vec3::new(radius * angle.cos(), height, radius * angle.sin());
        transform.look_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y);
    }
}
