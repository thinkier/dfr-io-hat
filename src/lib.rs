use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io::Error as IoError;
use i2c_linux::I2c;

pub struct DfrIoHat {
    dev: I2c<File>,
}

pub enum Channel {
    Ch0 = 0x00,
    Ch1 = 0x01,
    Ch2 = 0x02,
    Ch3 = 0x03,
}

#[allow(dead_code)]
enum Register {
    SlaveAddr = 0x00,
    PID = 0x01,
    VID = 0x02,
    PwmCtrl = 0x03,
    PwmFreq = 0x04,
    PwmDuty0 = 0x06,
    PwmDuty1 = 0x08,
    PwmDuty2 = 0x0A,
    PwmDuty3 = 0x0C,
    AdcCtrl = 0x0E,
    AdcCh0 = 0x0F,
    AdcCh1 = 0x11,
    AdcCh2 = 0x13,
    AdcCh3 = 0x15,

    DefPID = 0xDF,
    DefVID = 0x10,
}

enum BoardError {
    DeviceNotDetected,
    SoftVersion,
}

impl DfrIoHat {
    /// Open on the factory-default I2C address (0x10) on the given bus.
    pub fn open_default(bus: u8) -> Result<DfrIoHat, Box<dyn Error>> {
        Self::open(bus, 0x10)
    }

    /// Open on the given I2C bus and address.
    pub fn open(bus: u8, addr: u8) -> Result<DfrIoHat, Box<dyn Error>> {
        let mut dev = I2c::from_path(format!("/dev/i2c-{}", bus))?;
        dev.smbus_set_slave_address(addr as u16, false)?;

        let mut hat = DfrIoHat {
            dev,
        };
        hat.begin()?;

        Ok(hat)
    }

    fn read_byte(&mut self, reg: Register) -> Result<u8, IoError> {
        self.dev.smbus_read_byte_data(reg as u8)
    }

    fn read_bytes(&mut self, reg: Register, count: u8) -> Result<Vec<u8>, IoError> {
        let mut buf = Vec::with_capacity(count as usize);

        self.dev.smbus_read_block_data(reg as u8, &mut buf)?;

        Ok(buf)
    }

    fn write_bytes(&mut self, reg: Register, bytes: &[u8]) -> Result<(), IoError> {
        self.dev.smbus_write_block_data(reg as u8, bytes)?;

        Ok(())
    }

    /// Instantiate the IO Expansion Board
    fn begin(&mut self) -> Result<(), Box<dyn Error>> {
        let pid = self.read_byte(Register::PID)?;
        let vid = self.read_byte(Register::VID)?;

        if pid != Register::DefPID as u8 {
            return Err(Box::new(BoardError::DeviceNotDetected));
        }

        if vid != Register::DefVID as u8 {
            return Err(Box::new(BoardError::SoftVersion));
        }

        self.reset()?;

        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), IoError> {
        self.enable_pwm(false)?;
        for ch in Channel::all() {
            self.set_pwm_duty(ch, 0.0)?;
        }
        self.enable_adc(false)?;

        Ok(())
    }

    /// Activate the PWM subsystem
    pub fn enable_pwm(&mut self, enable: bool) -> Result<(), IoError> {
        if enable {
            self.write_bytes(Register::PwmCtrl, &[0x01])?;
        } else {
            self.write_bytes(Register::PwmCtrl, &[0x00])?;
        }

        Ok(())
    }

    /// Activate the ADC subsystem
    pub fn enable_adc(&mut self, enable: bool) -> Result<(), IoError> {
        if enable {
            self.write_bytes(Register::AdcCtrl, &[0x01])?;
        } else {
            self.write_bytes(Register::AdcCtrl, &[0x00])?;
        }

        Ok(())
    }

    /// Set the PWM duty cycle.
    /// Valid [`duty`] values are between ``0.000` and `1.000`.
    pub fn set_pwm_duty(&mut self, channel: Channel, duty: f32) -> Result<(), IoError> {
        assert!(duty >= 0f32);
        assert!(duty <= 1f32);
        let duty = (duty * 1e2) as u16;
        let bytes = [duty as u8, ((duty * 10) % 10) as u8]; // This is from the reference library and I'm not gonna question it

        match channel {
            Channel::Ch0 => self.write_bytes(Register::PwmDuty0, &bytes)?,
            Channel::Ch1 => self.write_bytes(Register::PwmDuty1, &bytes)?,
            Channel::Ch2 => self.write_bytes(Register::PwmDuty2, &bytes)?,
            Channel::Ch3 => self.write_bytes(Register::PwmDuty3, &bytes)?,
        }

        Ok(())
    }

    /// Set the PWM frequency for the entire board.
    /// Valid [`freq`] values are between `1` and `1000`.
    pub fn set_pwm_freq(&mut self, freq: u16) -> Result<(), IoError> {
        assert!(freq >= 1);
        assert!(freq <= 1000);
        let bytes = freq.to_be_bytes();

        self.write_bytes(Register::PwmFreq, &bytes)?;

        Ok(())
    }

    /// Get the value of the specified ADC pin, it will return a value between `0` and `1023`.
    pub fn get_adc_value(&mut self, channel: Channel) -> Result<u16, IoError> {
        let bytes = match channel {
            Channel::Ch0 => self.read_bytes(Register::AdcCh0, 2)?,
            Channel::Ch1 => self.read_bytes(Register::AdcCh1, 2)?,
            Channel::Ch2 => self.read_bytes(Register::AdcCh2, 2)?,
            Channel::Ch3 => self.read_bytes(Register::AdcCh3, 2)?,
        };

        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }
}

impl Drop for DfrIoHat {
    fn drop(&mut self) {
        let _ = self.reset();
    }
}

impl Debug for BoardError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "BoardStatus::{}", match self {
            BoardError::DeviceNotDetected => "DeviceNotDetected",
            BoardError::SoftVersion => "SoftVersion",
        })
    }
}

impl Display for BoardError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            BoardError::DeviceNotDetected => "Device not detected.",
            BoardError::SoftVersion => "Firmware/software version mismatch.",
        })
    }
}

impl Error for BoardError {}

impl Channel {
    /// Return an iterator over all the channels
    pub fn all() -> [Channel; 4] {
        [
            Channel::Ch0,
            Channel::Ch1,
            Channel::Ch2,
            Channel::Ch3,
        ]
    }
}
