/// Linear 1-D interpolation over a sorted table `(xs, ys)`.
///
/// Clamps to the endpoint values outside the table range.
/// Assumes `xs` is sorted in ascending order.
/// Maps to `Foam::interpolateXY(scalar, UList<scalar>&, UList<Type>&)`.
pub fn interpolate_xy(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len();
    assert_eq!(n, ys.len(), "xs and ys must have the same length");

    if n == 0 { return 0.0; }
    if n == 1 || x <= xs[0] { return ys[0]; }
    if x >= xs[n - 1] { return ys[n - 1]; }

    // Binary search: hi = first index where xs[hi] >= x
    let hi = xs.partition_point(|&v| v < x);
    let lo = hi - 1;

    let t = (x - xs[lo]) / (xs[hi] - xs[lo]);
    ys[lo] + t * (ys[hi] - ys[lo])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_at_knots() {
        let xs = [0.0, 1.0, 2.0, 3.0];
        let ys = [0.0, 1.0, 4.0, 9.0];
        for i in 0..4 {
            let v = interpolate_xy(xs[i], &xs, &ys);
            assert!((v - ys[i]).abs() < 1e-14, "at {}: got {}", xs[i], v);
        }
    }

    #[test]
    fn midpoints() {
        let xs = [0.0, 1.0, 2.0];
        let ys = [0.0, 2.0, 6.0];
        // 0.5 → 1.0
        let v = interpolate_xy(0.5, &xs, &ys);
        assert!((v - 1.0).abs() < 1e-14, "got {}", v);
        // 1.5 → 4.0
        let v = interpolate_xy(1.5, &xs, &ys);
        assert!((v - 4.0).abs() < 1e-14, "got {}", v);
    }

    #[test]
    fn clamp_left() {
        let xs = [1.0, 2.0, 3.0];
        let ys = [10.0, 20.0, 30.0];
        assert_eq!(interpolate_xy(-5.0, &xs, &ys), 10.0);
        assert_eq!(interpolate_xy(1.0, &xs, &ys), 10.0);
    }

    #[test]
    fn clamp_right() {
        let xs = [1.0, 2.0, 3.0];
        let ys = [10.0, 20.0, 30.0];
        assert_eq!(interpolate_xy(3.0, &xs, &ys), 30.0);
        assert_eq!(interpolate_xy(100.0, &xs, &ys), 30.0);
    }

    #[test]
    fn single_point() {
        let xs = [5.0];
        let ys = [42.0];
        assert_eq!(interpolate_xy(0.0, &xs, &ys), 42.0);
        assert_eq!(interpolate_xy(5.0, &xs, &ys), 42.0);
        assert_eq!(interpolate_xy(9.0, &xs, &ys), 42.0);
    }

    #[test]
    fn two_points() {
        let xs = [0.0, 2.0];
        let ys = [0.0, 4.0];
        let v = interpolate_xy(1.0, &xs, &ys);
        assert!((v - 2.0).abs() < 1e-14);
    }
}
