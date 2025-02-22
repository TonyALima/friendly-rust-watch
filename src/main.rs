#![deny(unsafe_code)]
#![no_std]
#![no_main]

use aht10;
use cortex_m_rt::entry;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use heapless::String;
use panic_reset as _;
use shared_bus::BusManagerSimple;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use stm32f1xx_hal::{
    i2c::{DutyCycle, I2c, Mode},
    pac,
    prelude::*,
    timer::FTimer,
};

macro_rules! debug {
    ($($x:tt)*) => {
        {
            #[cfg(debug_assertions)]
            {
                cortex_m_semihosting::hprintln!($($x)*)
            }
            #[cfg(not(debug_assertions))]
            {

            }
        }
    }
}

#[entry]
fn main() -> ! {
    // Get access to the core peripherals from the cortex-m crate
    // let cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let mut afio = dp.AFIO.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(48.MHz())
        .pclk1(24.MHz())
        .freeze(&mut flash.acr);

    // Acquire the GPIOC peripheral
    let mut gpiob = dp.GPIOB.split();

    let scl = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
    let sda = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

    let i2c_dev = I2c::i2c1(
        dp.I2C1,
        (scl, sda),
        &mut afio.mapr,
        Mode::Fast {
            frequency: 400.kHz(),
            duty_cycle: DutyCycle::Ratio16to9,
        },
        clocks,
    )
    .blocking_default(clocks);

    let bus = BusManagerSimple::new(i2c_dev);

    let interface = I2CDisplayInterface::new(bus.acquire_i2c());
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap();

    display.clear(BinaryColor::Off).unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    display.flush().unwrap();

    let tim2: FTimer<pac::TIM2, 1_000> = FTimer::new(dp.TIM2, &clocks);
    let mut tim2_delay = tim2.delay();

    let mut aht10_dev =
        aht10::Aht10::new(aht10::Address::Default, bus.acquire_i2c(), &mut tim2_delay).unwrap();

    loop {
        display.clear(BinaryColor::Off).unwrap();
        let (h, t) = aht10_dev.read(&mut tim2_delay).unwrap();
        debug!("t: {}", t.raw());
        debug!("h: {}", h.raw());
        let t = float_to_string(t.celsius());
        let h = float_to_string(h.rh());
        let mut temperature_text: String<20> = String::new();
        temperature_text.push_str("Temperature: ").unwrap();
        temperature_text.push_str(&t).unwrap();
        temperature_text.push_str("C").unwrap();
        Text::with_baseline(&temperature_text, Point::zero(), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        let mut humidity_text: String<16> = String::new();
        humidity_text.push_str("Humidity: ").unwrap();
        humidity_text.push_str(&h).unwrap();
        humidity_text.push_str("%").unwrap();
        Text::with_baseline(&humidity_text, Point::new(0, 16), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        display.flush().unwrap();

        tim2_delay.delay_ms(5_000u16);
    }
}

fn float_to_string(f: f32) -> String<4> {
    let mut s: String<4> = String::new();
    s.push((((f / 10.0) as u8) % 10 + 48) as char).unwrap();
    s.push((((f / 1.0) as u8) % 10 + 48) as char).unwrap();
    s.push('.').unwrap();
    s.push((((((f / 0.1) as u16) % 10) as u8) + 48) as char)
        .unwrap();
    s
}
