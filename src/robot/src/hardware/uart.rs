mod crc;
pub mod regs;

use std::{ops::RangeTo, path::Path, time::Duration};

use log::trace;
use rppal::uart::Parity;

use regs::{ChopConf, DrvStatus, GConf, GStat, IholdIrun, NodeConf, PwmConf};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

const WRITE_BIT: u8 = 1 << 7;
const SYNC_BYTE: u8 = 0b_1010_0000_u8.reverse_bits();
const MASTER_ADDRESS: u8 = 0xff;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UartId {
    Uart0,
    Uart4,
}

impl UartId {
    fn file_path(self) -> &'static Path {
        match self {
            UartId::Uart0 => Path::new("/dev/ttyAMA0"),
            UartId::Uart4 => Path::new("/dev/ttyAMA4"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum NodeAddress {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
}

/// One UART bus, possibly with multiple motors.
#[derive(Debug)]
pub struct UartBus {
    inner: rppal::uart::Uart,
}

impl UartBus {
    /// The baud rate of the connection.
    ///
    /// The TMC2209 automatically detects the baud rate, but can only accept baud rates between
    /// 9600 and 500,000 (datasheet pg. 6). Additionally, the hardware on the Pi can only produce certain baud
    /// rates; see [`rppal::uart::Uart::set_baud_rate`]. We set the baud rate
    /// at this level to avoid needing to wait between uart operations.
    const BAUD_RATE: u32 = 230_400;

    pub fn new(id: UartId) -> Self {
        Self::with_path(id.file_path())
    }

    pub fn with_path(path: &Path) -> Self {
        trace!(target: "uart", "Initializing uart: path={path:?}");

        // For the parity & data bits settings, see datasheet pg. 21.
        // For the stop bits setting, see datasheet pg. 18.
        let mut uart = rppal::uart::Uart::with_path(path, Self::BAUD_RATE, Parity::None, 8, 1)
            // No error handling yet.
            .unwrap();

        // See logic in `Self::recv` for why the read buffer size is 4.
        // Additionally, all read and writes are blocking as we don't have any non-blocking
        // logic implemented yet.
        uart.set_read_mode(4, Duration::ZERO).unwrap();
        uart.set_write_mode(true).unwrap();

        trace!(target: "uart", "Initialized uart");

        Self { inner: uart }
    }

    /// See datasheet pg. 19 for the packet format.
    fn send_read(&mut self, address: NodeAddress, register: u8) {
        assert!(
            register & WRITE_BIT == 0,
            "Register address must have MSB set to 0, was {register}"
        );

        let packet = crc::with_crc([SYNC_BYTE, address as u8, register, 0]);

        self.inner.write(&packet).unwrap();

        trace!(
            target: "uart",
            "Sent read packet: address={} register={register}, {packet:?}", address as u8
        );
    }

    /// See datasheet pg. 18 for the packet format.
    fn send_write(&mut self, address: NodeAddress, register: u8, val: u32) {
        assert!(
            register & WRITE_BIT == 0,
            "Register address must have MSB set to 0, was {register}"
        );

        let val_bytes = val.to_be_bytes();
        let packet = crc::with_crc([
            SYNC_BYTE,
            address as u8,
            register | WRITE_BIT,
            val_bytes[0],
            val_bytes[1],
            val_bytes[2],
            val_bytes[3],
            0,
        ]);

        self.inner.write(&packet).unwrap();

        trace!(
            target: "uart",
            "Sent write packet: address={} register={register} val=0x{val:08x}, {packet:?}", address as u8
        );
    }

    fn recv(&mut self) -> (u8, u8, Option<u32>) {
        let mut buf = [0; 8];
        let ([buf1, buf2], []) = buf.as_chunks_mut::<4>() else {
            unreachable!()
        };

        self.inner.read(buf1).unwrap();

        let _sync_byte = buf1[0];
        assert_eq!(_sync_byte, SYNC_BYTE); // TODO: we should do something better here, right?
        let address = buf1[1];
        let register = buf1[2];

        let has_data = register & WRITE_BIT > 0 || address == MASTER_ADDRESS;
        let (val, packet) = if has_data {
            self.inner.read(buf2).unwrap();

            let val = u32::from_be_bytes(buf[3..7].try_into().unwrap());
            (Some(val), &buf[..])
        } else {
            (None, &buf1[..])
        };

        if let Some(val) = val {
            trace!(
                "Recieved packet: address={address} register={register} val=0x{val:08x}, {packet:?}"
            );
        } else {
            trace!("Recieved packet: address={address} register={register}, {packet:?}");
        }

        let _real_crc = *packet.last().unwrap();
        let _expected_crc = crc::calc_crc(packet);
        assert_eq!(_real_crc, _expected_crc); // TODO: we should do something better here, right?

        (address, register, val)
    }
}

#[derive(Debug)]
pub struct UartNode<'a> {
    bus: &'a mut UartBus,
    address: NodeAddress,
}

impl UartBus {
    pub fn node(&mut self, address: NodeAddress) -> UartNode<'_> {
        UartNode { bus: self, address }
    }
}

impl UartNode<'_> {
    pub const ADDRESS_RANGE: RangeTo<u8> = ..4;

    fn send_read(&mut self, register: u8) {
        self.bus.send_read(self.address, register);
    }

    fn send_write(&mut self, register: u8, value: u32) {
        self.bus.send_write(self.address, register, value);
    }
    
    pub fn read(&mut self, register: u8) -> u32 {
        trace!(
            "Reading from register {register} (address={})",
            self.address as u8
        );

        self.send_read(register);

        loop {
            if let (MASTER_ADDRESS, register2, Some(value)) = self.bus.recv()
                && register2 == register
            {
                return value;
            }
        }
    }

    /// Write to a register without doing any IFCNT-bookkeeping (or any other
    /// reads).
    pub fn write_raw(&mut self, register: u8, value: u32) {
        self.send_write(register, value);
    }

    pub fn write(&mut self, register: u8, value: u32) {
        trace!(
            "Writing value 0x{value:08x} ({value}) to register {register} (address={})",
            self.address as u8
        );

        let ifcnt = self.ifcnt();

        loop {
            self.send_write(register, value);

            if self.ifcnt() == ifcnt.wrapping_add(1) {
                break;
            }
        }
    }

    pub fn ifcnt(&mut self) -> u8 {
        self.read(regs::IFCNT_ADDRESS) as u8
    }

    pub fn tpowerdown(&mut self) -> u8 {
        self.read(regs::TPOWERDOWN_ADDRESS) as u8
    }

    pub fn set_tpowerdown(&mut self, value: u8) {
        self.write(regs::TPOWERDOWN_ADDRESS, value as u32)
    }
}

macro_rules! regs {
    ($($X:ty: $(get $x:ident)? $(set $set_x:ident)? $(clear $clear_x:ident)?;)*) => {$(
        $(pub fn $x(&mut self) -> $X {
            <$X>::from_bits_retain(self.read(<$X>::ADDRESS))
        })?

        $(pub fn $set_x(&mut self, value: $X) {
            self.write(<$X>::ADDRESS, value.bits())
        })?

        $(pub fn $clear_x(&mut self, value: $X) {
            self.write(<$X>::ADDRESS, value.bits())
        })?
    )*};
}

impl UartNode<'_> {
    regs!(
        GConf: get gconf set set_gconf;
        GStat: get gstat clear clear_gstat;
        NodeConf: set set_nodeconf;
        ChopConf: get chopconf set set_chopconf;
        PwmConf: get pwmconf set set_pwmconf;
        IholdIrun: set set_iholdirun;
        DrvStatus: get drvstatus;
    );
}
