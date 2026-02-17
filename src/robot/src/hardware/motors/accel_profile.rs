// (step, total_steps, v_max, a_max) -> time
type Profile = fn(u32);

// computes position -> time
pub fn trapezoid_profile_inv(step: u32, total_steps: u32, v_max: f64, a_max: f64) -> f64 {
    let step = step as f64;
    let total_steps = total_steps as f64;
    let thresh = v_max * v_max / a_max;
    if total_steps > thresh {
        let t1 = v_max / a_max;
        let t2 = total_steps / v_max;

        if step <= 0.5 * thresh {
            (step * 2.0 / a_max).sqrt()
        } else if total_steps - 0.5 * thresh <= step {
            (t1 + t2) - ((total_steps - step) * 2.0 / a_max).sqrt()
        } else {
            (step + 0.5 * thresh) / v_max
        }
    } else {
        let t1 = (total_steps / a_max).sqrt();

        if step <= total_steps / 2.0 {
            (step * 2.0 / a_max).sqrt()
        } else {
            2.0 * t1 - ((total_steps - step) * 2.0 / a_max).sqrt()
        }
    }
}
