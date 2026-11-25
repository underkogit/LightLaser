use image::DynamicImage;

pub type Segment = (f64, f64, f64, f64);

pub fn flip_image(image: DynamicImage, mirror_x: bool, mirror_y: bool) -> DynamicImage {
    let mut image = image;
    if mirror_x {
        image = image.fliph();
    }
    if mirror_y {
        image = image.flipv();
    }
    image
}

pub fn flip_segments(mut segments: Vec<Segment>, mirror_x: bool, mirror_y: bool) -> Vec<Segment> {
    if segments.is_empty() || (!mirror_x && !mirror_y) {
        return segments;
    }

    let Some((min_x, min_y, max_x, max_y)) = bounds(&segments) else {
        return segments;
    };

    let cx = (min_x + max_x) / 2.0;
    let cy = (min_y + max_y) / 2.0;

    for s in &mut segments {
        if mirror_x {
            s.0 = 2.0 * cx - s.0;
            s.2 = 2.0 * cx - s.2;
        }
        if mirror_y {
            s.1 = 2.0 * cy - s.1;
            s.3 = 2.0 * cy - s.3;
        }
    }

    segments
}

pub fn bounds(segments: &[Segment]) -> Option<Segment> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for (x0, y0, x1, y1) in segments {
        min_x = min_x.min(*x0).min(*x1);
        min_y = min_y.min(*y0).min(*y1);
        max_x = max_x.max(*x0).max(*x1);
        max_y = max_y.max(*y0).max(*y1);
    }

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Luma, Rgba};

    fn close(a: Segment, b: Segment) -> bool {
        (a.0 - b.0).abs() < 1e-9
            && (a.1 - b.1).abs() < 1e-9
            && (a.2 - b.2).abs() < 1e-9
            && (a.3 - b.3).abs() < 1e-9
    }

    fn sample() -> Vec<Segment> {
        vec![(0.0, 0.0, 2.0, 0.0), (0.0, 1.0, 1.0, 1.0)]
    }

    #[test]
    fn segments_flip_around_their_own_center() {
        let flipped = flip_segments(sample(), true, false);

        assert!(close(flipped[0], (2.0, 0.0, 0.0, 0.0)));
        assert!(close(flipped[1], (2.0, 1.0, 1.0, 1.0)));
        assert_eq!(bounds(&flipped), bounds(&sample()));
    }

    #[test]
    fn double_flip_restores_segments() {
        let once = flip_segments(sample(), true, true);
        let twice = flip_segments(once, true, true);

        for (a, b) in sample().iter().zip(twice.iter()) {
            assert!(close(*a, *b));
        }
    }

    #[test]
    fn empty_segments_stay_empty() {
        assert!(flip_segments(Vec::new(), true, true).is_empty());
    }

    #[test]
    fn image_flip_moves_the_dark_pixel_to_the_other_side() {
        let mut source = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            4,
            2,
            Rgba([255, 255, 255, 255]),
        ));
        source
            .as_mut_rgba8()
            .unwrap()
            .put_pixel(0, 0, Rgba([0, 0, 0, 255]));

        let flipped = flip_image(source, true, false);

        assert_eq!(flipped.to_luma8().get_pixel(3, 0), &Luma([0u8]));
        assert_eq!(flipped.to_luma8().get_pixel(0, 0), &Luma([255u8]));
    }
}
