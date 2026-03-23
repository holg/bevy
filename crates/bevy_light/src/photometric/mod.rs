//! Photometric light profile support for Bevy.
//!
//! This module provides asset types and loaders for photometric data files
//! (IES and EULUMDAT/LDT formats). These files describe the angular intensity
//! distribution of real-world luminaires, measured in standardized coordinate
//! systems.
//!
//! The loaded data is stored as an [`Image`] texture in equirectangular
//! projection (C-plane × gamma angle), suitable for GPU sampling.

mod component;
mod ies;
mod ldt;
mod profile;

pub use component::PhotometricLight;
pub use ies::IesLoader;
pub use ldt::{LdtData, LdtLoader, parse_ldt, sample_ldt};
pub use profile::PhotometricProfile;

use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;

/// Plugin that registers photometric profile asset types and loaders.
pub struct PhotometricPlugin;

impl Plugin for PhotometricPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<PhotometricProfile>()
            .init_asset_loader::<IesLoader>()
            .init_asset_loader::<LdtLoader>();
    }
}
