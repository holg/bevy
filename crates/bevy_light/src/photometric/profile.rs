use bevy_asset::{Asset, Handle};
use bevy_image::Image;
use bevy_math::Vec3;
use bevy_reflect::TypePath;

/// A photometric light profile describing the angular intensity distribution
/// of a luminaire.
///
/// The intensity data is stored as an equirectangular [`Image`] in R16Float
/// format, where:
/// - U axis = C-plane angle (azimuthal, 0°–360°)
/// - V axis = gamma angle (polar, 0°=nadir, 180°=zenith)
/// - Pixel values are normalized to [0, 1] by `peak_candela`
///
/// All format-specific coordinate transforms (IES C-plane rotation, symmetry
/// expansion, Type B→C conversion) are resolved at load time. The texture is
/// always in Type C coordinates regardless of the source format.
#[derive(Asset, TypePath, Debug, Clone)]
pub struct PhotometricProfile {
    /// The intensity distribution texture (R16Float, equirectangular).
    pub image: Handle<Image>,
    /// Peak intensity in candelas. The texture values are normalized by this.
    pub peak_candela: f32,
    /// Total luminous flux in lumens, if available from the file.
    pub total_lumens: Option<f32>,
    /// Luminaire physical dimensions in meters (width, length, height),
    /// if available from the file.
    pub dimensions_m: Option<Vec3>,
}
