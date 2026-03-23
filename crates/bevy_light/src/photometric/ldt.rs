use super::PhotometricProfile;
use bevy_asset::{io::Reader, AssetLoader, LoadContext, RenderAssetUsages};
use serde::{Deserialize, Serialize};
use bevy_image::Image;
use bevy_math::Vec3;
use bevy_reflect::TypePath;
use thiserror::Error;
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

/// Asset loader for EULUMDAT (LDT) photometric files.
///
/// EULUMDAT is the European standard photometric data format, used by the
/// majority of European luminaire manufacturers. Unlike IES, EULUMDAT uses
/// Type C coordinates natively (C0 along luminaire length), so no C-plane
/// rotation is needed at load time.
#[derive(Default, TypePath)]
pub struct LdtLoader;

/// Settings for the LDT loader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdtLoaderSettings {
    /// Resolution of the output texture in C-plane steps.
    pub c_steps: u32,
    /// Resolution of the output texture in gamma steps.
    pub g_steps: u32,
}

impl Default for LdtLoaderSettings {
    fn default() -> Self {
        Self {
            c_steps: 361,
            g_steps: 181,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum LdtLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid LDT file: {0}")]
    Parse(String),
}

impl AssetLoader for LdtLoader {
    type Asset = PhotometricProfile;
    type Settings = LdtLoaderSettings;
    type Error = LdtLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        // EULUMDAT files may use Windows-1252 encoding
        let text = String::from_utf8_lossy(&bytes);
        let parsed = parse_ldt(&text)?;
        let profile = build_profile(&parsed, settings, load_context)?;
        Ok(profile)
    }

    fn extensions(&self) -> &[&str] {
        &["ldt"]
    }
}

/// Parsed EULUMDAT file data.
/// Parsed EULUMDAT file data, available for CPU-side sampling.
#[allow(dead_code)]
pub struct LdtData {
    /// Symmetry indicator (Isym):
    /// 0 = no symmetry (full 0-360)
    /// 1 = full rotational symmetry (single C-plane)
    /// 2 = bilateral symmetry about C0-C180 (data for C0-C180)
    /// 3 = bilateral symmetry about C90-C270 (data for C0-C180)
    /// 4 = quadrant symmetry (data for C0-C90)
    isym: u32,
    /// Number of C-planes in the data
    num_c_planes: usize,
    /// Delta C angle in degrees
    delta_c: f32,
    /// Number of gamma angles per C-plane
    num_gamma: usize,
    /// Delta gamma angle in degrees
    delta_gamma: f32,
    /// C-plane angles in degrees
    c_angles: Vec<f32>,
    /// Gamma angles in degrees
    gamma_angles: Vec<f32>,
    /// Intensity values in cd/klm: c_planes × gamma_angles
    intensities: Vec<Vec<f32>>,
    /// Total luminous flux in lumens
    total_flux: f32,
    /// Luminaire dimensions in mm
    length_mm: f32,
    width_mm: f32,
    height_mm: f32,
}

/// Parse an EULUMDAT file.
///
/// The EULUMDAT format is a fixed-field text format where each value
/// occupies one line. The format is publicly documented.
/// Parse an EULUMDAT file from text. Public for CPU-side sampling.
pub fn parse_ldt(text: &str) -> Result<LdtData, LdtLoaderError> {
    let lines: Vec<&str> = text.lines().collect();

    if lines.len() < 42 {
        return Err(LdtLoaderError::Parse("File too short for EULUMDAT format".into()));
    }

    // EULUMDAT line assignments (0-indexed):
    // 0: Company identification
    // 1: Type indicator (Ityp)
    // 2: Symmetry indicator (Isym)
    // 3: Number of C-planes (Mc)
    // 4: Distance between C-planes (Dc)
    // 5: Number of luminous intensities per C-plane (Ng)
    // 6: Distance between gamma angles (Dg)
    // 7: Measurement report number
    // 8: Luminaire name
    // 9: Luminaire number
    // 10: File name
    // 11: Date/user
    // 12: Length/diameter of luminaire (mm)
    // 13: Width of luminaire b (mm), 0 for circular
    // 14: Height of luminaire (mm)
    // 15: Length of luminous area (mm)
    // 16: Width of luminous area b1 (mm), 0 for circular
    // 17: Height of luminous area C0 (mm)
    // 18: Height of luminous area C90 (mm)
    // 19: Height of luminous area C180 (mm)
    // 20: Height of luminous area C270 (mm)
    // 21: Downward flux fraction (DFF) %
    // 22: Light output ratio (LORL) %
    // 23: Conversion factor for luminous intensities
    // 24: Tilt of luminaire during measurement
    // 25: Number of standard sets of lamps (n)
    // 26 + (n-1)*6 lines of lamp data follow

    let isym = parse_ldt_int(lines[2])?;
    let num_c_planes = parse_ldt_int(lines[3])? as usize;
    let delta_c = parse_ldt_float(lines[4])?;
    let num_gamma = parse_ldt_int(lines[5])? as usize;
    let delta_gamma = parse_ldt_float(lines[6])?;

    let length_mm = parse_ldt_float(lines[12])?;
    let width_mm = parse_ldt_float(lines[13])?;
    let height_mm = parse_ldt_float(lines[14])?;

    let conversion_factor = parse_ldt_float(lines[23])?;
    let num_lamp_sets = parse_ldt_int(lines[25])? as usize;

    // Each lamp set has 6 lines: num_lamps, type, total_flux, color_appearance,
    // color_rendering_group, wattage
    let lamp_data_start = 26;
    let lamp_data_end = lamp_data_start + num_lamp_sets * 6;

    // Read total flux from the first lamp set (line 28 for first set)
    let total_flux = if num_lamp_sets > 0 && lamp_data_start + 2 < lines.len() {
        parse_ldt_float(lines[lamp_data_start + 2])?
    } else {
        0.0
    };

    if lamp_data_end >= lines.len() {
        return Err(LdtLoaderError::Parse("File too short for lamp data".into()));
    }

    // After lamp data come:
    // - 10 lines of direct ratios (DR)
    let dr_start = lamp_data_end;
    let dr_end = dr_start + 10;

    if dr_end >= lines.len() {
        return Err(LdtLoaderError::Parse("File too short for direct ratios".into()));
    }

    // Then C-plane angles (num_c_planes values, one per line)
    let c_angles_start = dr_end;
    let c_angles_end = c_angles_start + num_c_planes;

    if c_angles_end > lines.len() {
        return Err(LdtLoaderError::Parse("File too short for C-plane angles".into()));
    }

    let c_angles: Vec<f32> = (c_angles_start..c_angles_end)
        .map(|i| parse_ldt_float(lines[i]))
        .collect::<Result<_, _>>()?;

    // Then gamma angles (num_gamma values, one per line)
    let gamma_start = c_angles_end;
    let gamma_end = gamma_start + num_gamma;

    if gamma_end > lines.len() {
        return Err(LdtLoaderError::Parse("File too short for gamma angles".into()));
    }

    let gamma_angles: Vec<f32> = (gamma_start..gamma_end)
        .map(|i| parse_ldt_float(lines[i]))
        .collect::<Result<_, _>>()?;

    // Then intensity data. The number of data planes depends on symmetry:
    // Isym=0: all num_c_planes have data
    // Isym=1: only 1 plane (full rotational symmetry)
    // Isym=2,3: data for C0-C180 only (remaining mirrored)
    // Isym=4: data for C0-C90 only (remaining mirrored)
    let num_data_planes = match isym as u32 {
        1 => 1,
        2 | 3 => c_angles.iter().filter(|&&a| a <= 180.0).count(),
        4 => c_angles.iter().filter(|&&a| a <= 90.0).count(),
        _ => num_c_planes, // Isym=0 or unknown
    };

    let intensity_start = gamma_end;
    let total_intensities = num_data_planes * num_gamma;

    if intensity_start + total_intensities > lines.len() {
        return Err(LdtLoaderError::Parse(format!(
            "File too short for intensity data: need {} lines from offset {}, file has {} lines (isym={}, data_planes={}, gamma={})",
            total_intensities, intensity_start, lines.len(), isym, num_data_planes, num_gamma
        )));
    }

    let mut intensities = Vec::with_capacity(num_data_planes);
    for c in 0..num_data_planes {
        let block_start = intensity_start + c * num_gamma;
        let row: Vec<f32> = (block_start..block_start + num_gamma)
            .map(|i| {
                let val = parse_ldt_float(lines[i])?;
                Ok(val * conversion_factor)
            })
            .collect::<Result<_, LdtLoaderError>>()?;
        intensities.push(row);
    }

    // Store only the C-plane angles that have data
    let data_c_angles = c_angles[..num_data_planes].to_vec();

    Ok(LdtData {
        isym: isym as u32,
        num_c_planes,
        delta_c,
        num_gamma,
        delta_gamma,
        c_angles: data_c_angles,
        gamma_angles,
        intensities,
        total_flux,
        length_mm,
        width_mm,
        height_mm,
    })
}

/// Build a PhotometricProfile from parsed LDT data.
fn build_profile(
    data: &LdtData,
    settings: &LdtLoaderSettings,
    load_context: &mut LoadContext<'_>,
) -> Result<PhotometricProfile, LdtLoaderError> {
    let c_steps = settings.c_steps as usize;
    let g_steps = settings.g_steps as usize;

    let mut grid = vec![0.0f32; c_steps * g_steps];
    let mut peak = 0.0f32;

    for c_idx in 0..c_steps {
        let c_angle_deg = (c_idx as f32 / (c_steps - 1) as f32) * 360.0;

        for g_idx in 0..g_steps {
            let g_angle_deg = (g_idx as f32 / (g_steps - 1) as f32) * 180.0;

            let intensity = sample_ldt(data, c_angle_deg, g_angle_deg);
            grid[c_idx * g_steps + g_idx] = intensity;
            if intensity > peak {
                peak = intensity;
            }
        }
    }

    // Normalize by peak
    if peak > 0.0 {
        for v in &mut grid {
            *v /= peak;
        }
    }

    // Create R16Float texture
    let half_data: Vec<u8> = grid
        .iter()
        .flat_map(|&v| half::f16::from_f32(v).to_le_bytes())
        .collect();

    let image = Image::new(
        Extent3d {
            width: c_steps as u32,
            height: g_steps as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        half_data,
        TextureFormat::R16Float,
        RenderAssetUsages::default(),
    );

    let image_handle = load_context.add_labeled_asset("intensity_map".to_string(), image);

    // Dimensions in meters
    let dimensions = if data.length_mm != 0.0 || data.width_mm != 0.0 || data.height_mm != 0.0 {
        Some(Vec3::new(
            data.width_mm / 1000.0,
            data.length_mm / 1000.0,
            data.height_mm / 1000.0,
        ))
    } else {
        None
    };

    Ok(PhotometricProfile {
        image: image_handle,
        peak_candela: peak,
        total_lumens: if data.total_flux > 0.0 {
            Some(data.total_flux)
        } else {
            None
        },
        dimensions_m: dimensions,
    })
}

/// Sample intensity at (C, gamma) from LDT data with symmetry and interpolation.
/// Public for CPU-side illuminance computation (e.g., heatmaps).
pub fn sample_ldt(data: &LdtData, c_deg: f32, gamma_deg: f32) -> f32 {
    // Clamp gamma to measured range. Beyond max_gamma, use the edge value
    // (will be 0 if the last measured value is 0, which is typical for downlights).
    let gamma_clamped = gamma_deg.clamp(0.0, 180.0);

    // If gamma is beyond the last measured angle, return 0
    // (typical for downlights that only measure 0-90°)
    if !data.gamma_angles.is_empty() {
        let max_gamma = *data.gamma_angles.last().unwrap();
        if gamma_clamped > max_gamma {
            return 0.0;
        }
    }

    // Resolve symmetry to get effective C-plane angle.
    // For Isym=3, this can return negative values; take absolute value
    // since the distribution is symmetric about the reference plane.
    let c_effective = resolve_ldt_symmetry(data.isym, c_deg).abs();

    // Find interpolation parameters
    let c_interp = find_interp(&data.c_angles, c_effective);
    let g_interp = find_interp(&data.gamma_angles, gamma_deg);

    // Bilinear interpolation
    let get = |c_idx: usize, g_idx: usize| -> f32 {
        let c_idx = c_idx.min(data.intensities.len().saturating_sub(1));
        let row = &data.intensities[c_idx];
        let g_idx = g_idx.min(row.len().saturating_sub(1));
        row[g_idx]
    };

    let v00 = get(c_interp.0, g_interp.0);
    let v01 = get(c_interp.0, g_interp.1);
    let v10 = get(c_interp.1, g_interp.0);
    let v11 = get(c_interp.1, g_interp.1);

    let top = v00 + (v01 - v00) * g_interp.2;
    let bottom = v10 + (v11 - v10) * g_interp.2;
    top + (bottom - top) * c_interp.2
}

/// Resolve EULUMDAT symmetry types.
///
/// - Isym=0: no symmetry, full 0-360 data
/// - Isym=1: full rotational symmetry, all C-planes identical
/// - Isym=2: symmetry about C0-C180 plane (data 0-180, mirror for 180-360)
/// - Isym=3: symmetry about C90-C270 plane (data 0-180, mirror for 180-360 shifted)
/// - Isym=4: quadrant symmetry (data 0-90, mirror for other quadrants)
/// Resolve EULUMDAT symmetry types.
///
/// Returns the effective C-plane angle within the stored data range.
/// Matches eulumdat-rs SymmetryHandler::get_intensity_at logic exactly.
fn resolve_ldt_symmetry(isym: u32, c_deg: f32) -> f32 {
    let c = c_deg.rem_euclid(360.0);

    match isym {
        1 => 0.0, // VerticalAxis: all C-planes identical
        2 => {
            // PlaneC0C180: data for 0..180, mirror 180..360
            if c <= 180.0 { c } else { 360.0 - c }
        }
        3 => {
            // PlaneC90C270: data for 0..180 (representing C270..C90 via the plane)
            // eulumdat-rs: shifted = (c+90) % 360
            //   if shifted <= 180: effective = shifted - 90
            //   else: effective = 270 - shifted
            let shifted = (c + 90.0).rem_euclid(360.0);
            if shifted <= 180.0 {
                shifted - 90.0
            } else {
                270.0 - shifted
            }
        }
        4 => {
            // BothPlanes: fold into 0..90
            let c_half = if c <= 180.0 { c } else { 360.0 - c };
            if c_half <= 90.0 { c_half } else { 180.0 - c_half }
        }
        _ => c, // Isym=0: full data
    }
}

/// Find interpolation bracket (lo_idx, hi_idx, fraction) in a sorted array.
fn find_interp(angles: &[f32], target: f32) -> (usize, usize, f32) {
    if angles.is_empty() {
        return (0, 0, 0.0);
    }
    if angles.len() == 1 {
        return (0, 0, 0.0);
    }
    if target <= angles[0] {
        return (0, 0, 0.0);
    }
    let last = angles.len() - 1;
    if target >= angles[last] {
        return (last, last, 0.0);
    }

    let mut lo = 0;
    let mut hi = last;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if angles[mid] <= target {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let range = angles[hi] - angles[lo];
    let frac = if range > 0.0 {
        (target - angles[lo]) / range
    } else {
        0.0
    };

    (lo, hi, frac)
}

/// Parse a float from an EULUMDAT line (may use comma as decimal separator).
fn parse_ldt_float(line: &str) -> Result<f32, LdtLoaderError> {
    let s = line.trim().replace(',', ".");
    s.parse::<f32>()
        .map_err(|_| LdtLoaderError::Parse(format!("Invalid float: '{}'", line.trim())))
}

/// Parse an integer from an EULUMDAT line.
fn parse_ldt_int(line: &str) -> Result<i32, LdtLoaderError> {
    let s = line.trim();
    if let Ok(v) = s.parse::<i32>() {
        return Ok(v);
    }
    // Try parsing as float (some files have "2.0" for integer fields)
    if let Ok(v) = s.replace(',', ".").parse::<f32>() {
        return Ok(v as i32);
    }
    Err(LdtLoaderError::Parse(format!("Invalid integer: '{s}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symmetry_resolution() {
        // Isym=1: full rotational symmetry
        assert_eq!(resolve_ldt_symmetry(1, 45.0), 0.0);
        assert_eq!(resolve_ldt_symmetry(1, 270.0), 0.0);

        // Isym=2: bilateral about C0-C180
        assert!((resolve_ldt_symmetry(2, 270.0) - 90.0).abs() < 0.01);
        assert!((resolve_ldt_symmetry(2, 350.0) - 10.0).abs() < 0.01);

        // Isym=4: quadrant
        assert!((resolve_ldt_symmetry(4, 135.0) - 45.0).abs() < 0.01);
        assert!((resolve_ldt_symmetry(4, 225.0) - 45.0).abs() < 0.01);
        assert!((resolve_ldt_symmetry(4, 315.0) - 45.0).abs() < 0.01);
    }

    #[test]
    fn test_ldt_float_parsing() {
        assert!((parse_ldt_float("  1234.5  ").unwrap() - 1234.5).abs() < 0.01);
        assert!((parse_ldt_float("1234,5").unwrap() - 1234.5).abs() < 0.01);
        assert!((parse_ldt_float("0").unwrap()).abs() < 0.01);
    }
}
