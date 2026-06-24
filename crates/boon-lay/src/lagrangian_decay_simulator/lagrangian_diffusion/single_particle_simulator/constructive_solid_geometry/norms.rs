#[inline]
pub fn l2_norms_sq_3d_f64(vs: &[[f64; 3]], out: &mut [f64]) {
    assert_eq!(vs.len(), out.len());
    for (v, o) in vs.iter().zip(out.iter_mut()) {
        let x = v[0];
        let y = v[1];
        let z = v[2];
        *o = x.mul_add(x, y.mul_add(y, z * z));
    }
}

#[inline]
pub fn l2_norms_sqrt_3d_f64(vs: &[[f64;3]], out: &mut [f64]) {

    // first compute the square norm
    l2_norms_sq_3d_f64(vs, out);

    // then sqrt all in the vector 
    for norm_sq_ptr in out {
        // perform sqrt
        let sqrt_num: f64 = norm_sq_ptr.sqrt();
        // return sqrt
        *norm_sq_ptr = sqrt_num;

    }

}
