use std::error::Error;
use std::thread;
use std::time::Duration;
use dfr_io_hat::{Channel, DfrIoHat};

fn main() -> Result<(), Box<dyn Error>> {
    let mut hat = DfrIoHat::open_default(1)?;

    hat.set_pwm_freq(2)?;
    for ch in Channel::all() {
        hat.set_pwm_duty(ch, 0.5)?;
    }

    thread::sleep(Duration::from_secs(30));

    Ok(())
}
