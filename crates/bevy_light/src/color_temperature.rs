use bevy_color::color_temperature::kelvin_to_linear_rgb;
use bevy_color::LinearRgba;
use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;

/// A component that tints a light's color based on correlated color temperature (CCT)
/// in Kelvin.
///
/// When attached to an entity with a [`PointLight`], [`SpotLight`], or [`DirectionalLight`],
/// the light's effective color is multiplied by the color corresponding to the given
/// color temperature.
///
/// Common color temperatures:
/// - **1800K** — Candle flame
/// - **2700K** — Warm white (incandescent bulb)
/// - **3000K** — Soft white
/// - **4000K** — Neutral white (cool fluorescent)
/// - **5000K** — Horizon daylight
/// - **5500K** — Mid-morning / mid-afternoon daylight
/// - **6500K** — Overcast daylight (D65 reference white)
/// - **7500K** — North sky daylight
/// - **10000K** — Blue sky
///
/// [`PointLight`]: crate::PointLight
/// [`SpotLight`]: crate::SpotLight
/// [`DirectionalLight`]: crate::DirectionalLight
///
/// # Example
///
/// ```ignore
/// commands.spawn((
///     PointLight {
///         intensity: 800.0,
///         ..default()
///     },
///     ColorTemperature { kelvin: 2700.0 },
/// ));
/// ```
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component, Default, Debug)]
pub struct ColorTemperature {
    /// The color temperature in Kelvin.
    ///
    /// Values are clamped to the range 1000K–40000K.
    pub kelvin: f32,
}

impl Default for ColorTemperature {
    fn default() -> Self {
        Self { kelvin: 6500.0 }
    }
}

impl ColorTemperature {
    /// Creates a new `ColorTemperature` with the given Kelvin value.
    pub fn new(kelvin: f32) -> Self {
        Self { kelvin }
    }

    /// Returns the linear RGB color corresponding to this color temperature.
    pub fn to_linear_rgba(self) -> LinearRgba {
        kelvin_to_linear_rgb(self.kelvin)
    }
}
