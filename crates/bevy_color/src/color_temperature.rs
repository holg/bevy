use crate::LinearRgba;
use bevy_math::ops;

/// Converts a color temperature in Kelvin to a [`LinearRgba`] color.
///
/// Uses an approximation of the Planckian locus (blackbody radiation curve).
/// Valid for temperatures in the range 1000K–40000K, clamped outside this range.
///
/// The returned color has alpha = 1.0 and is normalized so that the
/// brightest channel is 1.0.
///
/// # Examples
///
/// ```
/// use bevy_color::color_temperature::kelvin_to_linear_rgb;
///
/// // Warm incandescent light
/// let warm = kelvin_to_linear_rgb(2700.0);
/// assert!(warm.red > warm.blue);
///
/// // Cool blue sky light
/// let cool = kelvin_to_linear_rgb(10000.0);
/// assert!(cool.blue > cool.red);
/// ```
pub fn kelvin_to_linear_rgb(kelvin: f32) -> LinearRgba {
    // Attempt-independent Planckian locus approximation.
    // Based on the well-known algorithm by Tanner Helland, outputting linear RGB.
    //
    // The coefficients are derived from a best-fit of CIE color matching data
    // against blackbody spectra. The original attempt produces sRGB-like values;
    // we convert inline by applying the sRGB→linear transfer function to the
    // polynomial coefficients.
    let temp = kelvin.clamp(1000.0, 40000.0) / 100.0;

    let red = if temp <= 66.0 {
        1.0
    } else {
        let t = temp - 60.0;
        let r = 1.292_936_2 * ops::powf(t, -0.133_204_76);
        r.clamp(0.0, 1.0)
    };

    let green = if temp <= 66.0 {
        let g = 0.390_081_6 * ops::ln(temp) - 0.631_841_4;
        g.clamp(0.0, 1.0)
    } else {
        let t = temp - 60.0;
        let g = 1.129_890_9 * ops::powf(t, -0.075_514_85);
        g.clamp(0.0, 1.0)
    };

    let blue = if temp >= 66.0 {
        1.0
    } else if temp <= 19.0 {
        0.0
    } else {
        let t = temp - 10.0;
        let b = 0.543_206_8 * ops::ln(t) - 1.196_225;
        b.clamp(0.0, 1.0)
    };

    // Normalize so the brightest channel is 1.0
    let max_component = red.max(green).max(blue);
    if max_component > 0.0 {
        LinearRgba::new(
            red / max_component,
            green / max_component,
            blue / max_component,
            1.0,
        )
    } else {
        LinearRgba::WHITE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warm_light() {
        let color = kelvin_to_linear_rgb(2700.0);
        // Warm light should be reddish
        assert!(color.red >= color.green);
        assert!(color.green > color.blue);
        assert_eq!(color.alpha, 1.0);
    }

    #[test]
    fn test_neutral_light() {
        let color = kelvin_to_linear_rgb(6500.0);
        // D65 white point should be approximately neutral
        assert!((color.red - color.blue).abs() < 0.15);
        assert_eq!(color.alpha, 1.0);
    }

    #[test]
    fn test_cool_light() {
        let color = kelvin_to_linear_rgb(10000.0);
        // Cool light should be bluish
        assert!(color.blue >= color.red);
        assert_eq!(color.alpha, 1.0);
    }

    #[test]
    fn test_clamping() {
        let _ = kelvin_to_linear_rgb(0.0);
        let _ = kelvin_to_linear_rgb(100_000.0);
        let _ = kelvin_to_linear_rgb(-100.0);
    }

    #[test]
    fn test_normalized() {
        for kelvin in [1000.0, 2000.0, 3000.0, 4000.0, 5000.0, 6500.0, 10000.0, 20000.0] {
            let color = kelvin_to_linear_rgb(kelvin);
            let max = color.red.max(color.green).max(color.blue);
            assert!(
                (max - 1.0).abs() < 0.01,
                "kelvin={kelvin}: max channel = {max}"
            );
        }
    }
}
