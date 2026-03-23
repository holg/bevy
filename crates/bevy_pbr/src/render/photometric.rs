//! GPU-side photometric profile support.
//!
//! This module manages the photometric descriptor buffer and texture bindings
//! for per-fragment angular intensity lookup in the PBR lighting shader.

use core::{num::NonZero, ops::Deref};

use bevy_app::{App, Plugin};
use bevy_asset::AssetId;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    entity::EntityHashMap,
    resource::Resource,
    system::{Res, ResMut},
};
use bevy_image::Image;
use bevy_math::{Mat4, Vec4, Vec4Swizzles};
use bevy_platform::collections::HashMap;
use bevy_ecs::schedule::IntoScheduleConfigs as _;
use bevy_render::{
    render_asset::RenderAssets,
    render_resource::{
        binding_types, BindGroupLayoutEntryBuilder, Buffer, BufferUsages, RawBufferVec, Sampler,
        SamplerBindingType, TextureSampleType, TextureView,
    },
    renderer::{RenderAdapter, RenderDevice, RenderQueue},
    settings::WgpuFeatures,
    sync_world::MainEntity,
    texture::{FallbackImage, GpuImage},
    GpuResourceAppExt, Render, RenderApp, RenderSystems,
};

use tracing::info;

use crate::binding_arrays_are_usable;

/// GPU-side descriptor for a photometric profile, packed as a Mat4.
///
/// Layout:
/// - col0.xyz = inverse rotation column 0, col0.w = texture_index (as f32)
/// - col1.xyz = inverse rotation column 1, col1.w = peak_candela
/// - col2.xyz = inverse rotation column 2, col2.w = 0.0
/// - col3 = unused (0.0)
///
/// Using Mat4 avoids custom struct names that trigger naga_oil
/// composable module identifier substitution rules.
pub type GpuPhotometricDescriptor = Mat4;

/// Render-world resource that collects photometric profile data for all
/// lights in the scene.
#[derive(Resource, Default)]
pub struct RenderPhotometricProfiles {
    /// Maps binding array index → texture asset ID.
    pub binding_index_to_textures: Vec<AssetId<Image>>,
    /// Maps texture asset ID → binding array index.
    texture_to_binding_index: HashMap<AssetId<Image>, u32>,
    /// Descriptors for each photometric light, indexed by the value packed
    /// into `GpuClusteredLight::flags`.
    pub descriptors: Vec<GpuPhotometricDescriptor>,
    /// Maps main-world entity → descriptor index.
    pub entity_to_descriptor_index: EntityHashMap<u32>,
}

impl RenderPhotometricProfiles {
    pub fn clear(&mut self) {
        self.binding_index_to_textures.clear();
        self.texture_to_binding_index.clear();
        self.descriptors.clear();
        self.entity_to_descriptor_index.clear();
    }

    /// Returns the binding array index for the given image, inserting if needed.
    fn get_or_insert_image(&mut self, image_id: AssetId<Image>) -> u32 {
        *self
            .texture_to_binding_index
            .entry(image_id)
            .or_insert_with(|| {
                let index = self.binding_index_to_textures.len() as u32;
                self.binding_index_to_textures.push(image_id);
                index
            })
    }

    /// Adds a photometric descriptor for a light entity.
    pub fn insert(
        &mut self,
        main_entity: MainEntity,
        image_id: AssetId<Image>,
        inverse_rotation: Mat4,
        peak_candela: f32,
    ) {
        let index = self.descriptors.len() as u32;
        self.insert_raw(image_id, inverse_rotation, peak_candela);
        self.entity_to_descriptor_index
            .insert((*main_entity).into(), index);
    }

    /// Adds a photometric descriptor without entity tracking.
    /// Returns the descriptor index.
    pub fn insert_raw(
        &mut self,
        image_id: AssetId<Image>,
        inverse_rotation: Mat4,
        peak_candela: f32,
    ) -> u32 {
        let texture_index = self.get_or_insert_image(image_id);

        // Pack into Mat4:
        // col0.xyz = inv_rot col0, col0.w = texture_index
        // col1.xyz = inv_rot col1, col1.w = peak_candela
        // col2.xyz = inv_rot col2, col2.w = 0
        // col3 = 0
        let descriptor = Mat4::from_cols(
            inverse_rotation.col(0).xyz().extend(texture_index as f32),
            inverse_rotation.col(1).xyz().extend(peak_candela),
            inverse_rotation.col(2).xyz().extend(0.0),
            Vec4::ZERO,
        );

        let index = self.descriptors.len() as u32;
        self.descriptors.push(descriptor);
        index
    }
}

/// GPU buffer holding photometric descriptors (packed as Mat4).
#[derive(Resource, Deref, DerefMut)]
pub struct PhotometricDescriptorsBuffer(RawBufferVec<Mat4>);

impl Default for PhotometricDescriptorsBuffer {
    fn default() -> Self {
        PhotometricDescriptorsBuffer(RawBufferVec::new(BufferUsages::STORAGE))
    }
}

/// Uploads photometric descriptors to the GPU.
pub fn upload_photometric_descriptors(
    render_profiles: Res<RenderPhotometricProfiles>,
    mut buffer: ResMut<PhotometricDescriptorsBuffer>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    buffer.clear();

    let desc_count = render_profiles.descriptors.len();
    let tex_count = render_profiles.binding_index_to_textures.len();
    if desc_count > 0 {
        info!("Photometric upload: {} descriptors, {} textures", desc_count, tex_count);
    }

    for &descriptor in &render_profiles.descriptors {
        buffer.push(descriptor);
    }

    // Ensure non-empty for binding.
    if buffer.is_empty() {
        buffer.push(Mat4::ZERO);
    }

    buffer.write_buffer(&render_device, &render_queue);
}

/// Bind group entries for photometric profiles for a single view.
pub struct RenderViewPhotometricBindGroupEntries<'a> {
    /// Storage buffer containing all photometric descriptors.
    pub descriptors: &'a Buffer,
    /// Texture views for all photometric profile textures.
    pub texture_views: Vec<&'a <TextureView as Deref>::Target>,
    /// Sampler for the photometric textures.
    pub sampler: &'a Sampler,
}

/// Returns the bind group layout entries for photometric profiles.
pub(crate) fn get_bind_group_layout_entries(
    render_device: &RenderDevice,
    render_adapter: &RenderAdapter,
) -> Option<[BindGroupLayoutEntryBuilder; 3]> {
    if !photometric_profiles_are_usable(render_device, render_adapter) {
        return None;
    }

    Some([
        // `photometric_descriptors` — packed as array<mat4x4<f32>>
        binding_types::storage_buffer_read_only::<Mat4>(false),
        // `photometric_textures`
        binding_types::texture_2d(TextureSampleType::Float { filterable: true })
            .count(NonZero::<u32>::new(max_photometric_textures(render_device)).unwrap()),
        // `photometric_sampler`
        binding_types::sampler(SamplerBindingType::Filtering),
    ])
}

impl<'a> RenderViewPhotometricBindGroupEntries<'a> {
    /// Creates bind group entries for photometric profiles.
    pub fn get(
        render_profiles: &RenderPhotometricProfiles,
        buffer: &'a PhotometricDescriptorsBuffer,
        images: &'a RenderAssets<GpuImage>,
        fallback_image: &'a FallbackImage,
        render_device: &RenderDevice,
        render_adapter: &RenderAdapter,
    ) -> Option<RenderViewPhotometricBindGroupEntries<'a>> {
        if !photometric_profiles_are_usable(render_device, render_adapter) {
            return None;
        }

        // Use the first available sampler, or fallback.
        let sampler = match render_profiles
            .binding_index_to_textures
            .iter()
            .filter_map(|image_id| images.get(*image_id))
            .next()
        {
            Some(gpu_image) => &gpu_image.sampler,
            None => &fallback_image.d2.sampler,
        };

        // Gather texture views.
        let mut texture_views = vec![];
        for image_id in &render_profiles.binding_index_to_textures {
            match images.get(*image_id) {
                None => texture_views.push(&*fallback_image.d2.texture_view),
                Some(gpu_image) => texture_views.push(&*gpu_image.texture_view),
            }
        }

        // Pad if platform doesn't support partial binding arrays.
        if !render_device
            .features()
            .contains(WgpuFeatures::PARTIALLY_BOUND_BINDING_ARRAY)
        {
            let max_textures = max_photometric_textures(render_device) as usize;
            while texture_views.len() < max_textures {
                texture_views.push(&*fallback_image.d2.texture_view);
            }
        } else if texture_views.is_empty() {
            texture_views.push(&*fallback_image.d2.texture_view);
        }

        Some(RenderViewPhotometricBindGroupEntries {
            descriptors: buffer.buffer()?,
            texture_views,
            sampler,
        })
    }
}

/// Returns true if photometric profiles are usable on the current platform.
pub fn photometric_profiles_are_usable(
    render_device: &RenderDevice,
    render_adapter: &RenderAdapter,
) -> bool {
    binding_arrays_are_usable(render_device, render_adapter)
        && cfg!(feature = "pbr_photometric_lights")
}

/// Maximum number of photometric profile textures.
fn max_photometric_textures(render_device: &RenderDevice) -> u32 {
    if render_device
        .features()
        .contains(WgpuFeatures::PARTIALLY_BOUND_BINDING_ARRAY)
    {
        256
    } else {
        8
    }
}

/// Plugin that registers GPU-side photometric profile resources and systems.
pub struct PhotometricRenderPlugin;

impl Plugin for PhotometricRenderPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(feature = "pbr_photometric_lights") {
            info!("PhotometricRenderPlugin: pbr_photometric_lights feature ENABLED");
        } else {
            info!("PhotometricRenderPlugin: pbr_photometric_lights feature DISABLED");
        }

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_gpu_resource::<PhotometricDescriptorsBuffer>()
            .init_resource::<RenderPhotometricProfiles>()
            .add_systems(
                Render,
                upload_photometric_descriptors.in_set(RenderSystems::PrepareResources),
            );
    }
}
