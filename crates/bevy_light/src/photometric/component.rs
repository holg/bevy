use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;

use super::PhotometricProfile;

/// Adds a photometric angular intensity distribution to a light.
///
/// When attached alongside a [`PointLight`] or [`SpotLight`], the light's
/// intensity at each surface point is modulated by the measured angular
/// distribution from the referenced [`PhotometricProfile`]. This replaces
/// the default uniform emission with a realistic luminaire pattern.
///
/// The profile texture is sampled per-fragment on the GPU using the direction
/// from the light to the surface point, converted to Type C photometric
/// coordinates (C-plane azimuthal angle + gamma polar angle) in the
/// luminaire's local coordinate frame.
///
/// [`PointLight`]: crate::PointLight
/// [`SpotLight`]: crate::SpotLight
///
/// # Example
///
/// ```ignore
/// commands.spawn((
///     PointLight {
///         intensity: 1000.0,
///         ..default()
///     },
///     PhotometricLight {
///         profile: asset_server.load("luminaire.ies"),
///     },
/// ));
/// ```
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component, Debug)]
pub struct PhotometricLight {
    /// Handle to the [`PhotometricProfile`] asset containing the angular
    /// intensity distribution.
    pub profile: Handle<PhotometricProfile>,
}
