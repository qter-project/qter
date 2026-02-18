use crate::hardware::motors::{Dir, motor::{MotorAction, MotorCommand}};

// type Profile = impl Fn(total_steps, v_max, a_max) -> MotorAction
// (can't actually write that)
// Profiles are expected to have `CW` as forwards and `CCW` as backwards, except for after `specify_dir`.

/// Flip the directions of the motor commands to match `dir`.
pub fn specify_dir(dir: Dir, profile: impl Fn(u32, f64, f64) -> Vec<(f64, MotorCommand)>) -> impl Fn(u32, f64, f64) -> MotorAction {
    move |total_steps, v_max, a_max| {
        let mut commands = profile(total_steps, v_max, a_max);

        if dir == Dir::CCW {
            for command in &mut commands {
                command.1 = command.1.flip_dir();
            }
        }

        commands
    }
}

pub const fn mk_steps_from_inv(inv: fn(u32, u32, f64, f64) -> f64) -> impl Fn(u32, f64, f64) -> MotorAction {
    move |total_steps, v_max, a_max| {
        let mut out = Vec::new();

        for i in 1..=total_steps {
            let t = inv(i, total_steps, v_max, a_max);
            out.push((t, MotorCommand::StepCW));
        }

        out
    }
}

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

#[derive(Clone, Copy, Debug)]
struct InitialConditions {
    position: u32,
    velocity: f64,
}

/// Create an acceleration ramp with the given acceleration that stops once either the `position` or `velocity` in `target_ics` are satisfied. Returns the true initial conditions achieved.
// pub fn accel_stage(ics: InitialConditions, target_ics: InitialConditions, accel: f64) -> (InitialConditions, MotorAction) {
    
// }
