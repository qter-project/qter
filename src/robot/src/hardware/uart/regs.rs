use bitflags::bitflags;

macro_rules! bitfields {
    ($( $vis:vis $field:ident: $ty:ty = $start:literal..=$end:literal; )*) => {
        ::paste::paste! {$(
            $vis const fn $field(self) -> $ty {
                const START: u32 = $start;
                const END: u32 = $end + 1;
                const MASK: u32 = (!0 << START) & !(!0 << END);
                const _: () = {
                    assert!(<$ty>::BITS >= (END - START));
                };

                ((self.bits() & MASK) >> START) as $ty
            }

            $vis const fn [< with_ $field >](self, val: $ty) -> Self {
                const START: u32 = $start;
                const END: u32 = $end + 1;
                const MASK: u32 = (!0 << START) & !(!0 << END);

                let val = val as u32;
                assert!(val & !(MASK >> START) == 0);

                Self::from_bits_retain(self.bits() & !MASK | val << START)
            }
        )*}
    };
}

/// The IFCNT register address on the TMC2209.
///
/// See datasheet pg. 24.
pub const IFCNT_ADDRESS: u8 = 0x02;

/// The TPOWERDOWN register address on the TMC2209.
///
/// See datasheet pg. 28.
pub const TPOWERDOWN_ADDRESS: u8 = 0x11;

bitflags! {
    /// The GCONF register bitflags on the TMC2209. UART is permitted to read
    /// and write to this register.
    ///
    /// See datasheet pg. 23.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct GConf: u32 {
        /// Enable StealthChop (0) or SpreadCycle (1) mode.
        const EN_SPREADCYCLE = 1 << 2;
        /// Inverse the motor direction.
        const SHAFT = 1 << 3;
        // INDEX pin outputs overtemperature prewarning flag (otpw)
        const INDEX_OTPW = 1 << 4;
        // PDN_UART input function disabled; set to use UART
        const PDN_DISABLE = 1 << 6;
        // Microstep resolution selected by MRES register
        const MSTEP_REG_SELECT = 1 << 7;

        // Indicate which bits are used (but aren't in our flags yet)
        const _ = (1 << 10) - 1;
    }
}

impl GConf {
    pub const ADDRESS: u8 = 0x00;
}

// I can't see us needing this in the near future...
// 0x1, n = 3, R+WC
bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct GStat: u32 {
        const RESET = 1 << 0;
        const DRV_ERR = 1 << 1;
        const UV_CP = 1 << 2;
    }
}

impl GStat {
    pub const ADDRESS: u8 = 0x01;
}

bitflags! {
    /// The NODECONF register bitflags on the TMC2209. UART is only permitted
    /// to write to this register, so all reads will return 0.
    ///
    /// See datasheet pg. 24.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct NodeConf: u32 {
        /// SENDDELAY bit 0.
        const SENDDELAY0 = 1 << 8;
        /// SENDDELAY bit 1.
        const SENDDELAY1 = 1 << 9;
        /// SENDDELAY bit 2.
        const SENDDELAY2 = 1 << 10;
        /// SENDDELAY bit 3.
        const SENDDELAY3 = 1 << 11;
    }
}

impl NodeConf {
    pub const ADDRESS: u8 = 0x03;

    bitfields! {
        pub senddelay: u8 = 8..=11;
    }
}

bitflags! {
    /// The CHOPCONF register bitflags on the TMC2209. UART is permitted to read
    /// and write to this register.
    ///
    /// See datasheet pg. 33.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct ChopConf: u32 {
        /// Microstep resolution bit 0.
        const MRES0 = 1 << 24;
        /// Microstep resolution bit 1.
        const MRES1 = 1 << 25;
        /// Microstep resolution bit 2.
        const MRES2 = 1 << 26;
        /// Microstep resolution bit 3.
        const MRES3 = 1 << 27;

        // Indicate which bits are used (but aren't in our flags yet)
        const _ = 0b_11111111_00000011_10000111_11111111;
    }
}

impl ChopConf {
    pub const ADDRESS: u8 = 0x6C;

    bitfields! {
        pub mres: u8 = 24..=27;
    }
}

bitflags! {
    /// The PWMCONF register bitflags on the TMC2209. UART is permitted to read
    /// and write to this register.
    ///
    /// See datasheet pg. 35.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct PwmConf: u32 {
        /// Freewheel mode bit 0.
        const FREEWHEEL0 = 1 << 20;
        /// Freewheel mode bit 1.
        const FREEWHEEL1 = 1 << 21;

        // Indicate which bits are used (but aren't in our flags yet)
        const _ = 0b_11111111_00111111_11111111_11111111;
    }
}

impl PwmConf {
    pub const ADDRESS: u8 = 0x70;

    bitfields! {
        pub freewheel: u8 = 20..=21;
    }
}

bitflags! {
    /// The IHOLD_IRUN register bitflags on the TMC2209. UART is only permitted
    /// to write to this register, so all reads will return 0.
    ///
    /// See datasheet pg. 35.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct IholdIrun: u32 {
        /// IHOLD bit 0.
        const IHOLD0 = 1;
        /// IHOLD bit 1.
        const IHOLD1 = 1 << 1;
        /// IHOLD bit 2.
        const IHOLD2 = 1 << 2;
        /// IHOLD bit 3.
        const IHOLD3 = 1 << 3;
        /// IHOLD bit 4.
        const IHOLD4 = 1 << 4;
        /// IRUN bit 0.
        const IRUN0 = 1 << 8;
        /// IRUN bit 1.
        const IRUN1 = 1 << 9;
        /// IRUN bit 2.
        const IRUN2 = 1 << 10;
        /// IRUN bit 3.
        const IRUN3 = 1 << 11;
        /// IRUN bit 4.
        const IRUN4 = 1 << 12;
        /// IHOLDDELAY bit 0.
        const IHOLDDELAY0 = 1 << 16;
        /// IHOLDDELAY bit 1.
        const IHOLDDELAY1 = 1 << 17;
        /// IHOLDDELAY bit 2.
        const IHOLDDELAY2 = 1 << 18;
        /// IHOLDDELAY bit 3.
        const IHOLDDELAY3 = 1 << 19;
    }
}

impl IholdIrun {
    pub const ADDRESS: u8 = 0x10;

    bitfields! {
        pub ihold: u8 = 0..=4;
        pub irun: u8 = 8..=12;
        pub iholddelay: u8 = 16..=19;
    }
}

bitflags! {
    /// The DRV_STATUS register bitflags on the TMC2209. UART is only permitted
    /// to read from this register.
    #[derive(Debug, PartialEq, Clone, Copy)]
    pub struct DrvStatus: u32 {
        /// Overtemperature pre-warning flag
        const OTPW = 1 << 0;
        /// Overtemperature flag.
        const OT = 1 << 1;
        /// Short-to-ground on phase A.
        const S2GA = 1 << 2;
        /// Short-to-ground on phase B.
        const S2GB = 1 << 3;
        /// Low-side short on phase A.
        const S2VSA = 1 << 4;
        /// Low-side short on phase B.
        const S2VSB = 1 << 5;
        /// Open-load detected on phase A.
        const OLA = 1 << 6;
        /// Open-load detected on phase B.
        const OLB = 1 << 7;
        /// Bit 0 of the actual motor current.
        const CS_ACTUAL0 = 1 << 16;
        /// Bit 1 of the actual motor current.
        const CS_ACTUAL1 = 1 << 17;
        /// Bit 2 of the actual motor current.
        const CS_ACTUAL2 = 1 << 18;
        /// Bit 3 of the actual motor current.
        const CS_ACTUAL3 = 1 << 19;
        /// Bit 4 of the actual motor current.
        const CS_ACTUAL4 = 1 << 20;

        // Indicate which bits are used (but aren't in our flags yet)
        const _ = 0b_11000000_00011111_00001111_11111111;
    }
}

impl DrvStatus {
    pub const ADDRESS: u8 = 0x6F;

    bitfields! {
        pub cs_actual: u8 = 16..=20;
    }
}
