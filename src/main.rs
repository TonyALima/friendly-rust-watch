#![no_std]
#![no_main]

use core::cmp::{max, min};

use aht10::*;
use cortex_m::asm;
use cortex_m_rt::entry;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder},
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
    pac::{self, interrupt, Interrupt, NVIC},
    prelude::*,
    timer::{Counter, Event, FTimer},
    rtc::Rtc,
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

const TEXT_STYLE: embedded_graphics::mono_font::MonoTextStyle<'_, BinaryColor> =
    MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

static mut TIMER2: Option<Counter<pac::TIM2, 1_000>> = None;
static mut RTC: Option<Rtc> = None;

struct Statistic {
    max: Weather,
    min: Weather,
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
    let mut pwr = dp.PWR;
    let mut backup_domain = rcc.bkp.constrain(dp.BKP, &mut pwr);

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .freeze(&mut flash.acr);

    let mut rtc = Rtc::new(dp.RTC, &mut backup_domain);
    rtc.set_time(0);
    rtc.set_alarm(86400);
    rtc.listen_alarm();
    unsafe {
        RTC = Some(rtc);
    }

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
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate90)
        .into_buffered_graphics_mode();
    display.init().unwrap();

    display.clear(BinaryColor::Off).unwrap();

    display.flush().unwrap();

    
    let mut tim2: Counter<pac::TIM2, 1_000> = FTimer::new(dp.TIM2, &clocks).counter();
    tim2.start(5.secs()).unwrap();
    tim2.listen(Event::Update);
    unsafe {
        TIMER2 = Some(tim2);
    }
    
    unsafe {
        // Enable the TIM2 interrupt
        NVIC::unmask(Interrupt::TIM2);
        NVIC::unmask(Interrupt::RTC);
    }

    let mut aht10_dev = Aht10::new(
        aht10::Address::Default,
        bus.acquire_i2c(),
        clocks.sysclk().raw(),
    )
    .unwrap();

    let mut stat  = Statistic {
        max: Weather {
            humidity: Humidity::from_raw(0),
            temperature: Temperature::from_raw(0),
        },
        min: Weather {
            humidity: Humidity::from_raw(0xFFFF_FFFF),
            temperature: Temperature::from_raw(0xFFFF_FFFF),
        },
    };

    let mut count: u32 = 0;

    loop {
        let weather_now = aht10_dev.read().unwrap();
        stat.max.temperature = max(stat.max.temperature, weather_now.temperature);
        stat.max.humidity = max(stat.max.humidity, weather_now.humidity);
        stat.min.temperature = min(stat.min.temperature, weather_now.temperature);
        stat.min.humidity = min(stat.min.humidity, weather_now.humidity);
        debug!("t: {}", weather_now.temperature.raw());
        debug!("h: {}", weather_now.temperature.raw());

        display.clear(BinaryColor::Off).unwrap();
        match count {
            0..2 => draw_h_t(&weather_now, "NOW", &mut display).unwrap(),
            2 => draw_h_t(&stat.max, "MAX", &mut display).unwrap(),
            3 => {
                draw_h_t(&stat.min, "MIN", &mut display).unwrap();
                count = 0;
            },
            _ => count = 0,
        }
        
        display.flush().unwrap();
        count += 1;
        asm::wfi();
    }
}

#[allow(static_mut_refs)]
#[interrupt]
fn RTC() {
    let rtc = unsafe { RTC.as_mut().unwrap() };
    rtc.clear_alarm_flag();
    rtc.set_time(0);
}

#[allow(static_mut_refs)]
#[interrupt]
fn TIM2() {
    let tim2 = unsafe { TIMER2.as_mut().unwrap() };
    tim2.clear_interrupt(Event::Update);
}

fn draw_h_t<D>(
    w: &Weather,
    text: &str,
    display: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor>,
{
    let mut temperature_text: String<6> = String::new();
    let mut humidity_text: String<6> = String::new();
    let t = float_to_string(w.temperature.celsius());
    let h = float_to_string(w.humidity.rh());
    Text::with_baseline(
        text,
        Point::new((64-30)/2, 10),
        TEXT_STYLE,
        Baseline::Top,
    )
    .draw(display)?;

    temperature_text.push_str(&t).unwrap();
    temperature_text.push_str(" C").unwrap();
    Text::with_baseline(
        &temperature_text,
        Point::new(0, 47),
        TEXT_STYLE,
        Baseline::Top,
    )
    .draw(display)?;

    humidity_text.push_str(&h).unwrap();
    humidity_text.push_str(" %").unwrap();
    Text::with_baseline(&humidity_text,
        Point::new(0, 84),
        TEXT_STYLE, Baseline::Top,
    )
    .draw(display)?;
    Ok(())
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
