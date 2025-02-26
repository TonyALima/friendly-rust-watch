#![no_std]
use bitflags::bitflags;
use cortex_m::asm;
use embedded_hal::blocking::i2c;

bitflags! {
    struct StatusFlags: u8 {
        const BUSY = 0x80;
        const CALIBRATED = 0x08;
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Command {
    Initialization = 0xE1,
    TriggerMeasurement = 0xAC,
    SoftReset = 0xBA,
    CalibrationEnable = 0x08,
    HumidityAndTemperature = 0x33,
    Nop = 0x00,
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum Address {
    Default = 0x38,
    Alternative = 0x39,
}

/// Humidity reading from AHT10.
pub struct Humidity {
    h: u32,
}
impl Humidity {
    /// Humidity conveted to relative humidity.
    pub fn rh(&self) -> f32 {
        (self.h as f32) / ((1 << 20) as f32) * 100.0
    }
    /// Raw humidity reading.
    pub fn raw(&self) -> u32 {
        self.h
    }
    pub fn from_raw(h: u32) -> Self {
        Humidity { h }
    }
}

/// Temperature reading from AHT10.
pub struct Temperature {
    t: u32,
}
impl Temperature {
    /// Temperature converted to celsius.
    pub fn celsius(&self) -> f32 {
        (self.t as f32) / ((1 << 20) as f32) * 200.0 - 50.0
    }
    /// Raw temperature reading.
    pub fn raw(&self) -> u32 {
        self.t
    }
    pub fn from_raw(t: u32) -> Self {
        Temperature { t }
    }
}

pub struct Aht10<I>
where
    I: i2c::Write + i2c::Read,
{
    addr: Address,
    i2c_dev: I,
    sys_clock: u32,
}

impl<I, E> Aht10<I>
where
    I: i2c::Write<Error = E> + i2c::Read<Error = E>,
{
    pub fn new(addr: Address, i2c_dev: I, sys_clock: u32) -> Result<Self, E> {
        let mut dev = Aht10 {
            addr,
            i2c_dev,
            sys_clock,
        };
        // 20 ms to power up
        asm::delay(dev.sys_clock * (20 / 1000));

        dev.i2c_dev
            .write(dev.addr as u8, &[Command::SoftReset as u8])?;

        asm::delay(dev.sys_clock * (20 / 1000));

        while dev.status()?.contains(StatusFlags::BUSY) {
            asm::delay(dev.sys_clock * (10 / 1000));
        }

        let cmds = [
            Command::Initialization as u8,
            Command::CalibrationEnable as u8,
            Command::Nop as u8,
        ];
        dev.i2c_dev.write(dev.addr as u8, &cmds)?;

        while dev.status()?.contains(StatusFlags::BUSY) {
            asm::delay(dev.sys_clock * (10 / 1000));
        }

        Ok(dev)
    }

    // to do (feito por copilot tem q ver isso ai)
    fn status(&mut self) -> Result<StatusFlags, E> {
        let mut data = [0];
        self.i2c_dev.read(self.addr as u8, &mut data)?;
        Ok(StatusFlags::from_bits_truncate(data[0]))
    }

    pub fn read(&mut self) -> Result<(Humidity, Temperature), E> {
        let cmds = [
            Command::TriggerMeasurement as u8,
            Command::HumidityAndTemperature as u8,
            Command::Nop as u8,
        ];

        self.i2c_dev.write(self.addr as u8, &cmds)?;

        while self.status()?.contains(StatusFlags::BUSY) {
            asm::delay(self.sys_clock * (10 / 1000));
        }

        let mut data: [u8; 6] = [0; 6];
        self.i2c_dev.read(self.addr as u8, &mut data)?;

        // separete 20 bits data
        let h = ((data[1] as u32) << 12) | ((data[2] as u32) << 4) | ((data[3] as u32) >> 4);
        let t = (((data[3] as u32) & 0x0F) << 16) | ((data[4] as u32) << 8) | (data[5] as u32);

        Ok((Humidity { h }, Temperature { t }))
    }
}
