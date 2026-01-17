use std::{fmt::Debug, ops::Index, str::FromStr};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use super::uart::{NodeAddress, UartId};

/// Global robot configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotConfig {
    pub motors: Motors,
    pub revolutions_per_second: f64,
    pub max_acceleration: f64,
    pub microstep_resolution: Microsteps,
    pub priority: Priority,
    pub wait_between_moves: f64,
    pub compensation: u32,
    pub float: bool,
    pub stealthchop: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorConfig {
    pub step_pin: u8,
    pub dir_pin: u8,
    pub uart_bus: UartId,
    pub uart_address: NodeAddress,

    pub pos_compensation: Option<u32>,
    pub neg_compensation: Option<u32>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum Face {
    R,
    L,
    U,
    D,
    F,
    B,
}

impl FromStr for Face {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "R" => Ok(Face::R),
            "L" => Ok(Face::L),
            "U" => Ok(Face::U),
            "D" => Ok(Face::D),
            "F" => Ok(Face::F),
            "B" => Ok(Face::B),
            _ => Err(()),
        }
    }
}

impl Face {
    pub const ALL: [Self; 6] = {
        use Face::*;
        let v = [R, L, U, D, F, B];

        let mut i = 0;
        while i < v.len() {
            assert!(i == v[i] as usize);
            i += 1;
        }

        v
    };
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "MotorsRepr", into = "MotorsRepr")]
pub struct Motors([MotorConfig; 6]);

impl Index<Face> for Motors {
    type Output = MotorConfig;

    fn index(&self, index: Face) -> &Self::Output {
        &self.0[index as usize]
    }
}

impl Debug for Motors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        MotorsRepr::from(self.clone()).fmt(f)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct MotorsRepr {
    R: MotorConfig,
    U: MotorConfig,
    F: MotorConfig,
    L: MotorConfig,
    D: MotorConfig,
    B: MotorConfig,
}

impl From<MotorsRepr> for Motors {
    fn from(value: MotorsRepr) -> Self {
        let mut out = [const { None }; 6];
        out[Face::R as usize] = Some(value.R);
        out[Face::U as usize] = Some(value.U);
        out[Face::F as usize] = Some(value.F);
        out[Face::L as usize] = Some(value.L);
        out[Face::D as usize] = Some(value.D);
        out[Face::B as usize] = Some(value.B);
        Motors(out.map(Option::unwrap))
    }
}

impl From<Motors> for MotorsRepr {
    fn from(value: Motors) -> Self {
        let mut value = value.0.map(Some);
        MotorsRepr {
            R: value[Face::R as usize].take().unwrap(),
            U: value[Face::U as usize].take().unwrap(),
            F: value[Face::F as usize].take().unwrap(),
            L: value[Face::L as usize].take().unwrap(),
            D: value[Face::D as usize].take().unwrap(),
            B: value[Face::B as usize].take().unwrap(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(from = "MicrostepsRepr", into = "MicrostepsRepr")]
pub enum Microsteps {
    Fullstep = 8,
    Two = 7,
    Four = 6,
    Eight = 5,
    Sixteen = 4,
    ThirtyTwo = 3,
    SixtyFour = 2,
    OneTwentyEight = 1,
    TwoFiftySix = 0,
}

#[derive(Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum MicrostepsRepr {
    Fullstep = 1,
    Two = 2,
    Four = 4,
    Eight = 8,
    Sixteen = 16,
    ThirtyTwo = 32,
    SixtyFour = 64,
    OneTwentyEight = 128,
    TwoFiftySix = 256,
}

macro_rules! enum_conv {
    ($a:ty, $b:ty; $($variant:ident),* $(,)?) => {
        impl From<$a> for $b {
            fn from(value: $a) -> Self {
                match value { $(<$a>::$variant => <$b>::$variant),* }
            }
        }

        impl From<$b> for $a {
            fn from(value: $b) -> Self {
                match value { $(<$b>::$variant => <$a>::$variant),* }
            }
        }
    };
}

enum_conv!(
    Microsteps, MicrostepsRepr;
    Fullstep, Two, Four, Eight, Sixteen, ThirtyTwo, SixtyFour, OneTwentyEight, TwoFiftySix
);

impl Microsteps {
    pub fn mres_value(self) -> u8 {
        self as u8
    }

    pub fn value(self) -> u32 {
        MicrostepsRepr::from(self) as u32
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum)]
pub enum Priority {
    /// Leave the priority as whatever the OS decides it to be
    Default,
    /// Set the priority to the maximum non-real-time priority
    MaxNonRT,
    /// Set the priority to the maximum real-time priority that is also lower than any kernel priority
    RealTime,
}
