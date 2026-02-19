use std::cmp::Ordering;

use crate::hardware::motors::{
    Dir,
    motor::{MotorAction, MotorCommand},
};

// type Profile = impl Fn(total_steps, v_max, a_max) -> MotorAction
// (can't actually write that)
// Profiles are expected to have `CW` as forwards and `CCW` as backwards, except for after `specify_dir`.

/// Flip the directions of the motor commands to match `dir`.
pub fn specify_dir(
    dir: Dir,
    profile: impl Fn(u32, f64, f64) -> MotorAction,
) -> impl Fn(u32, f64, f64) -> MotorAction {
    move |total_steps, v_max, a_max| {
        let mut commands = profile(total_steps, v_max, a_max);

        if dir == Dir::CCW {
            for command in &mut commands.0 {
                command.1 = command.1.flip_dir();
            }
        }

        commands
    }
}

pub const fn mk_steps_from_inv(
    inv: fn(u32, u32, f64, f64) -> f64,
) -> impl Fn(u32, f64, f64) -> MotorAction {
    move |total_steps, v_max, a_max| {
        let mut out = Vec::new();

        for i in 1..=total_steps {
            let t = inv(i, total_steps, v_max, a_max);
            out.push((t, MotorCommand::StepCW));
        }

        MotorAction(out)
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

// pub fn trapezoid_profile(compensation: i32) -> MotorAction {
//     |total_steps, v_max, a_max| {
//         let (new_ics, accel) = accel_stage(
//             InitialConditions {
//                 position: 0,
//                 velocity: 0.,
//             },
//             InitialConditions {
//                 position: total_steps / 2,
//                 velocity: v_max,
//             },
//             a_max,
//         );
//     }
// }

#[derive(Clone, Copy, Debug)]
struct InitialConditions {
    position: i32,
    velocity: f64,
}

/// Create a section with constant velocity that stops once it is at the given `position`.
///
/// # Panics
///
/// Panics if the the initial velocity and (`target_pos` - `ics.position`) are not the same sign.
pub fn vel_stage(ics: InitialConditions, target_pos: i32) -> (InitialConditions, MotorAction) {
    let amt_to_move = ics.position - target_pos;

    let command = match amt_to_move.cmp(&0) {
        Ordering::Less => MotorCommand::StepCCW,
        Ordering::Equal => {
            return (ics, MotorAction(Vec::new()));
        }
        Ordering::Greater => MotorCommand::StepCW,
    };

    if amt_to_move.signum() != ics.velocity.signum() as i32 {
        panic!("Cannot reach the target position with the given velocity");
    }

    let mut out = Vec::new();

    let spacing = ics.velocity.recip().abs();
    let mut now = 0.;

    for _ in 0..amt_to_move.unsigned_abs() {
        now += spacing;
        out.push((now, command));
    }

    (
        InitialConditions {
            position: target_pos,
            velocity: ics.velocity,
        },
        MotorAction(out),
    )
}

/// Create an acceleration ramp with the given acceleration that stops once either the `position` or `velocity` in `target_ics` are satisfied. Returns the true initial conditions achieved.
///
/// # Panics
///
/// Panics if neither the given velocity nor target position can be reached.
pub fn accel_stage(
    mut ics: InitialConditions,
    target_ics: InitialConditions,
    accel: f64,
) -> (InitialConditions, MotorAction) {
    if accel.signum() != (ics.velocity - target_ics.velocity).signum() {
        // We cannot reach the target velocity with the given acceleration
        return (ics, MotorAction(Vec::new()));
    }

    let target_vel_reachable = accel.signum() == (ics.velocity - target_ics.velocity).signum();

    let mut out = Vec::new();
    let mut now = 0.;

    while ics.position != target_ics.position {
        let target_pos_possibly_reachable =
            (ics.position - target_ics.position).signum() == ics.velocity.signum() as i32;
        if !target_vel_reachable && !target_pos_possibly_reachable {
            panic!(
                "Cannot reach either the target position or the given velocity with the given acceleration and ICs"
            );
        }

        // Formula found by integrating velocity & using quadratic formula
        let time_to_next_step = (-ics.velocity + (ics.velocity.powi(2) + 2. * accel)) / accel;

        let vel_after = ics.velocity + accel * time_to_next_step;

        if vel_after.signum() != ics.velocity.signum() {
            // We would need to accelerate past zero velocity; lets add enough time such that the target position after exactly equals the current velocity

            now += (ics.velocity / accel).abs() * 2.;
            ics.velocity *= -1.;

            continue;
        }

        out.push((
            now,
            match ics.velocity.total_cmp(&0.) {
                Ordering::Less => MotorCommand::StepCCW,
                Ordering::Equal => {
                    continue;
                }
                Ordering::Greater => MotorCommand::StepCW,
            },
        ));

        ics.velocity = vel_after;
        now += time_to_next_step;
    }

    (ics, MotorAction(out))
}
