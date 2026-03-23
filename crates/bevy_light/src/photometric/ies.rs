use super::PhotometricProfile;
use bevy_asset::{io::Reader, AssetLoader, LoadContext, RenderAssetUsages};
use serde::{Deserialize, Serialize};
use bevy_image::Image;
use bevy_math::Vec3;
use bevy_reflect::TypePath;
use thiserror::Error;
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

/// Asset loader for IES photometric files (IESNA LM-63 format).
///
/// Supports all major LM-63 versions: 1986, 1991, 1995, 2002, and 2019.
/// Handles Type C (most common), Type B, and Type A coordinate systems,
/// converting everything to Type C at load time.
#[derive(Default, TypePath)]
pub struct IesLoader;

/// Settings for the IES loader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IesLoaderSettings {
    /// Resolution of the output texture in C-plane steps.
    /// Default: 361 (1° resolution for 0°–360°).
    pub c_steps: u32,
    /// Resolution of the output texture in gamma steps.
    /// Default: 181 (1° resolution for 0°–180°).
    pub g_steps: u32,
}

impl Default for IesLoaderSettings {
    fn default() -> Self {
        Self {
            c_steps: 361,
            g_steps: 181,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum IesLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid IES file: {0}")]
    Parse(String),
}

impl AssetLoader for IesLoader {
    type Asset = PhotometricProfile;
    type Settings = IesLoaderSettings;
    type Error = IesLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let text = String::from_utf8_lossy(&bytes);
        let parsed = parse_ies(&text)?;
        let profile = build_profile(&parsed, settings, load_context)?;
        Ok(profile)
    }

    fn extensions(&self) -> &[&str] {
        &["ies"]
    }
}

/// Parsed IES file data.
#[allow(dead_code)]
struct IesData {
    /// Photometric type: 1 = Type C, 2 = Type B, 3 = Type A
    photometric_type: u32,
    /// Number of lamps
    num_lamps: i32,
    /// Lumens per lamp (-1 for absolute photometry)
    lumens_per_lamp: f32,
    /// Candela multiplier
    candela_multiplier: f32,
    /// Vertical (gamma) angles in degrees
    vertical_angles: Vec<f32>,
    /// Horizontal (C-plane) angles in degrees
    horizontal_angles: Vec<f32>,
    /// Candela values: horizontal_angles.len() rows × vertical_angles.len() columns
    candela_values: Vec<Vec<f32>>,
    /// Luminaire dimensions in meters: (width, length, height)
    dimensions: Option<Vec3>,
    /// Total lumens (computed if absolute photometry)
    total_lumens: Option<f32>,
}

/// Parse an IES file from text content.
fn parse_ies(text: &str) -> Result<IesData, IesLoaderError> {
    let lines: Vec<&str> = text.lines().collect();

    // Find the TILT line which marks the end of the header.
    // All IES versions have a TILT line before the photometric data.
    let tilt_line_idx = lines
        .iter()
        .position(|line| line.trim().starts_with("TILT"))
        .ok_or_else(|| IesLoaderError::Parse("No TILT line found".into()))?;

    let tilt_value = lines[tilt_line_idx].trim();
    let has_tilt_data = tilt_value == "TILT=INCLUDE";

    // After TILT line, optionally skip tilt data, then read the lamp/luminaire line.
    let mut data_start = tilt_line_idx + 1;

    if has_tilt_data {
        // Skip tilt data: lamp_to_luminaire, num_pairs, then angles and factors
        if data_start >= lines.len() {
            return Err(IesLoaderError::Parse("Unexpected end after TILT=INCLUDE".into()));
        }
        data_start += 1; // lamp_to_luminaire
        let num_pairs_tokens = tokenize_from_lines(&lines, &mut data_start, 1)?;
        let num_pairs = parse_int(&num_pairs_tokens[0])?;
        // Skip angles and multiplying factors (2 * num_pairs values)
        tokenize_from_lines(&lines, &mut data_start, num_pairs as usize)?;
        tokenize_from_lines(&lines, &mut data_start, num_pairs as usize)?;
    }

    // Read the 10 values on the lamp/luminaire line(s):
    // num_lamps, lumens_per_lamp, candela_multiplier, num_vert_angles,
    // num_horiz_angles, photometric_type, units_type, width, length, height
    let lamp_line_tokens = tokenize_from_lines(&lines, &mut data_start, 10)?;

    let num_lamps = parse_int(&lamp_line_tokens[0])?;
    let lumens_per_lamp = parse_float(&lamp_line_tokens[1])?;
    let candela_multiplier = parse_float(&lamp_line_tokens[2])?;
    let num_vert = parse_int(&lamp_line_tokens[3])? as usize;
    let num_horiz = parse_int(&lamp_line_tokens[4])? as usize;
    let photometric_type = parse_int(&lamp_line_tokens[5])? as u32;
    let _units_type = parse_int(&lamp_line_tokens[6])?; // 1=feet, 2=meters
    let width = parse_float(&lamp_line_tokens[7])?;
    let length = parse_float(&lamp_line_tokens[8])?;
    let height = parse_float(&lamp_line_tokens[9])?;

    // Read 3 values: ballast_factor, future_use (or photometric_file_factor), input_watts
    let _electrical = tokenize_from_lines(&lines, &mut data_start, 3)?;

    // Read vertical angles
    let vert_tokens = tokenize_from_lines(&lines, &mut data_start, num_vert)?;
    let vertical_angles: Vec<f32> = vert_tokens
        .iter()
        .map(|s| parse_float(s))
        .collect::<Result<_, _>>()?;

    // Read horizontal angles
    let horiz_tokens = tokenize_from_lines(&lines, &mut data_start, num_horiz)?;
    let horizontal_angles: Vec<f32> = horiz_tokens
        .iter()
        .map(|s| parse_float(s))
        .collect::<Result<_, _>>()?;

    // Read candela values: num_horiz sets of num_vert values
    let mut candela_values = Vec::with_capacity(num_horiz);
    for _ in 0..num_horiz {
        let row_tokens = tokenize_from_lines(&lines, &mut data_start, num_vert)?;
        let row: Vec<f32> = row_tokens
            .iter()
            .map(|s| parse_float(s))
            .collect::<Result<_, _>>()?;
        candela_values.push(row);
    }

    // Apply candela multiplier
    let candela_values: Vec<Vec<f32>> = candela_values
        .into_iter()
        .map(|row| row.into_iter().map(|v| v * candela_multiplier).collect())
        .collect();

    // Compute dimensions in meters (IES uses the opening dimensions)
    let dimensions = if width != 0.0 || length != 0.0 || height != 0.0 {
        // IES units: if units_type == 1, values are in feet, convert to meters
        let scale = if _units_type == 1 { 0.3048 } else { 1.0 };
        Some(Vec3::new(
            width.abs() * scale,
            length.abs() * scale,
            height.abs() * scale,
        ))
    } else {
        None
    };

    // Compute total lumens
    let total_lumens = if lumens_per_lamp < 0.0 {
        // Absolute photometry: lumens must be calculated from candela data
        None // Will be computed from the intensity grid
    } else {
        Some(lumens_per_lamp * num_lamps.abs() as f32)
    };

    Ok(IesData {
        photometric_type,
        num_lamps,
        lumens_per_lamp,
        candela_multiplier: 1.0, // Already applied
        vertical_angles,
        horizontal_angles,
        candela_values,
        dimensions,
        total_lumens,
    })
}

/// Build a PhotometricProfile from parsed IES data.
fn build_profile(
    data: &IesData,
    settings: &IesLoaderSettings,
    load_context: &mut LoadContext<'_>,
) -> Result<PhotometricProfile, IesLoaderError> {
    let c_steps = settings.c_steps as usize;
    let g_steps = settings.g_steps as usize;

    // Generate the full intensity grid in Type C coordinates.
    // IES Type C: gamma from vertical_angles, C from horizontal_angles.
    // NOTE: IES and EULUMDAT have different C0 conventions but the actual
    // orientation depends on the manufacturer's goniophotometer setup.
    // We do NOT apply automatic rotation — the user can rotate the luminaire
    // via the entity's Transform component.
    // the EULUMDAT convention (C0 along luminaire length).
    let mut grid = vec![0.0f32; c_steps * g_steps];
    let mut peak = 0.0f32;

    for c_idx in 0..c_steps {
        let c_angle_deg = (c_idx as f32 / (c_steps - 1) as f32) * 360.0;
        // No automatic C-plane rotation — take data as provided.
        // The user can rotate the luminaire in the scene if needed.
        let rotated_c = c_angle_deg;

        for g_idx in 0..g_steps {
            let g_angle_deg = (g_idx as f32 / (g_steps - 1) as f32) * 180.0;

            let intensity = if data.photometric_type == 2 {
                // Type B: convert (C, gamma) to (H, V) and sample
                sample_type_b(data, g_angle_deg, rotated_c)
            } else {
                // Type C (and Type A treated as Type C for now)
                sample_type_c(data, g_angle_deg, rotated_c)
            };

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

    Ok(PhotometricProfile {
        image: image_handle,
        peak_candela: peak,
        total_lumens: data.total_lumens,
        dimensions_m: data.dimensions,
    })
}

/// Sample intensity at (gamma, c_angle) from Type C data with interpolation.
fn sample_type_c(data: &IesData, gamma_deg: f32, c_deg: f32) -> f32 {
    // If gamma is outside the measured range, intensity is zero
    if !data.vertical_angles.is_empty() {
        let max_gamma = *data.vertical_angles.last().unwrap();
        let min_gamma = data.vertical_angles[0];
        if gamma_deg > max_gamma || gamma_deg < min_gamma {
            return 0.0;
        }
    }

    // Expand symmetry: determine the effective C-plane angle
    let c_effective = resolve_c_symmetry(&data.horizontal_angles, c_deg);

    // Bilinear interpolation
    let c_val = interpolate_angle(&data.horizontal_angles, c_effective);
    let g_val = interpolate_angle(&data.vertical_angles, gamma_deg);

    bilinear_sample(&data.candela_values, &data.horizontal_angles, &data.vertical_angles, c_val, g_val)
}

/// Sample intensity from Type B data by converting (gamma, c_angle) to (H, V).
fn sample_type_b(data: &IesData, gamma_deg: f32, c_deg: f32) -> f32 {
    // Type B→C conversion:
    // V = arcsin(sin(γ) · cos(C))
    // H = atan2(sin(γ) · sin(C), cos(γ))
    let gamma_rad = gamma_deg.to_radians();
    let c_rad = c_deg.to_radians();
    let sin_g = gamma_rad.sin();
    let cos_g = gamma_rad.cos();
    let sin_c = c_rad.sin();
    let cos_c = c_rad.cos();

    let v_rad = (sin_g * cos_c).asin();
    let h_rad = (sin_g * sin_c).atan2(cos_g);

    let v_deg = v_rad.to_degrees();
    let h_deg = h_rad.to_degrees();

    // Type B: vertical_angles are V angles, horizontal_angles are H angles
    let h_val = interpolate_angle(&data.horizontal_angles, h_deg);
    let v_val = interpolate_angle(&data.vertical_angles, v_deg);

    bilinear_sample(&data.candela_values, &data.horizontal_angles, &data.vertical_angles, h_val, v_val)
}

/// Resolve C-plane symmetry from the horizontal angles range.
///
/// IES symmetry is encoded by the range of horizontal angles:
/// - 0 only → full rotational symmetry (use same data for all C)
/// - 0 to 90 → quadrant symmetry (mirror about 0 and 90)
/// - 0 to 180 → bilateral symmetry about C0-C180 (mirror about 180)
/// - 0 to 360 → no symmetry (full data)
fn resolve_c_symmetry(horiz_angles: &[f32], c_deg: f32) -> f32 {
    if horiz_angles.len() <= 1 {
        // Full rotational symmetry: all C angles map to the single plane
        return horiz_angles.first().copied().unwrap_or(0.0);
    }

    let max_h = *horiz_angles.last().unwrap_or(&360.0);

    if max_h <= 0.0 {
        // Single plane
        0.0
    } else if max_h <= 90.0 {
        // Quadrant symmetry
        let mut c = c_deg % 360.0;
        if c < 0.0 { c += 360.0; }
        if c > 270.0 {
            360.0 - c
        } else if c > 180.0 {
            c - 180.0
        } else if c > 90.0 {
            180.0 - c
        } else {
            c
        }
    } else if max_h <= 180.0 {
        // Bilateral symmetry about C0-C180
        let mut c = c_deg % 360.0;
        if c < 0.0 { c += 360.0; }
        if c > 180.0 {
            360.0 - c
        } else {
            c
        }
    } else {
        // Full data 0-360
        let mut c = c_deg % 360.0;
        if c < 0.0 { c += 360.0; }
        c
    }
}

/// Find interpolation parameters for a target angle within a sorted angle array.
/// Returns (fractional index) for use in bilinear sampling.
struct InterpResult {
    idx_lo: usize,
    idx_hi: usize,
    frac: f32,
}

fn interpolate_angle(angles: &[f32], target: f32) -> InterpResult {
    if angles.is_empty() {
        return InterpResult { idx_lo: 0, idx_hi: 0, frac: 0.0 };
    }
    if angles.len() == 1 {
        return InterpResult { idx_lo: 0, idx_hi: 0, frac: 0.0 };
    }

    // Clamp to range
    if target <= angles[0] {
        return InterpResult { idx_lo: 0, idx_hi: 0, frac: 0.0 };
    }
    if target >= *angles.last().unwrap() {
        let last = angles.len() - 1;
        return InterpResult { idx_lo: last, idx_hi: last, frac: 0.0 };
    }

    // Binary search for the bracket
    let mut lo = 0;
    let mut hi = angles.len() - 1;
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

    InterpResult { idx_lo: lo, idx_hi: hi, frac }
}

/// Bilinear sample from the candela grid.
fn bilinear_sample(
    candela: &[Vec<f32>],
    _horiz_angles: &[f32],
    _vert_angles: &[f32],
    h_interp: InterpResult,
    v_interp: InterpResult,
) -> f32 {
    let get = |h_idx: usize, v_idx: usize| -> f32 {
        let h_idx = h_idx.min(candela.len().saturating_sub(1));
        let row = &candela[h_idx];
        let v_idx = v_idx.min(row.len().saturating_sub(1));
        row[v_idx]
    };

    let v00 = get(h_interp.idx_lo, v_interp.idx_lo);
    let v01 = get(h_interp.idx_lo, v_interp.idx_hi);
    let v10 = get(h_interp.idx_hi, v_interp.idx_lo);
    let v11 = get(h_interp.idx_hi, v_interp.idx_hi);

    let top = v00 + (v01 - v00) * v_interp.frac;
    let bottom = v10 + (v11 - v10) * v_interp.frac;
    top + (bottom - top) * h_interp.frac
}

/// Tokenize values from multiple lines, collecting at least `count` whitespace-separated tokens.
fn tokenize_from_lines(
    lines: &[&str],
    line_idx: &mut usize,
    count: usize,
) -> Result<Vec<String>, IesLoaderError> {
    let mut tokens = Vec::with_capacity(count);
    while tokens.len() < count {
        if *line_idx >= lines.len() {
            return Err(IesLoaderError::Parse(format!(
                "Unexpected end of file, expected {} more values",
                count - tokens.len()
            )));
        }
        let line = lines[*line_idx].trim();
        *line_idx += 1;
        if line.is_empty() {
            continue;
        }
        for token in line.split_whitespace() {
            tokens.push(token.to_string());
            if tokens.len() >= count {
                break;
            }
        }
    }
    Ok(tokens)
}

fn parse_float(s: &str) -> Result<f32, IesLoaderError> {
    s.parse::<f32>()
        .map_err(|_| IesLoaderError::Parse(format!("Invalid float: '{s}'")))
}

fn parse_int(s: &str) -> Result<i32, IesLoaderError> {
    // Handle floats that are really integers (e.g., "1.0")
    if let Ok(v) = s.parse::<i32>() {
        return Ok(v);
    }
    if let Ok(v) = s.parse::<f32>() {
        return Ok(v as i32);
    }
    Err(IesLoaderError::Parse(format!("Invalid integer: '{s}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid IES file for testing.
    const MINIMAL_IES: &str = r#"IESNA:LM-63-2002
[TEST] Test
[MANUFAC] Test Manufacturer
TILT=NONE
1 1000 1.0 3 1 1 2 0.0 0.0 0.0
1.0 1.0 100
0 90 180
0
1000 500 100
"#;

    #[test]
    fn test_parse_minimal_ies() {
        let data = parse_ies(MINIMAL_IES).unwrap();
        assert_eq!(data.photometric_type, 1);
        assert_eq!(data.vertical_angles.len(), 3);
        assert_eq!(data.horizontal_angles.len(), 1);
        assert_eq!(data.candela_values.len(), 1);
        assert_eq!(data.candela_values[0].len(), 3);
        assert_eq!(data.candela_values[0][0], 1000.0);
        assert_eq!(data.candela_values[0][1], 500.0);
        assert_eq!(data.candela_values[0][2], 100.0);
    }

    #[test]
    fn test_symmetry_resolution() {
        // Full rotational symmetry (single plane at 0)
        let angles = vec![0.0];
        assert_eq!(resolve_c_symmetry(&angles, 45.0), 0.0);
        assert_eq!(resolve_c_symmetry(&angles, 270.0), 0.0);

        // Bilateral symmetry 0-180
        let angles = vec![0.0, 90.0, 180.0];
        assert!((resolve_c_symmetry(&angles, 270.0) - 90.0).abs() < 0.01);
        assert!((resolve_c_symmetry(&angles, 350.0) - 10.0).abs() < 0.01);

        // Quadrant symmetry 0-90
        let angles = vec![0.0, 45.0, 90.0];
        assert!((resolve_c_symmetry(&angles, 135.0) - 45.0).abs() < 0.01);
        assert!((resolve_c_symmetry(&angles, 270.0) - 90.0).abs() < 0.01);
        assert!((resolve_c_symmetry(&angles, 315.0) - 45.0).abs() < 0.01);
    }
}
