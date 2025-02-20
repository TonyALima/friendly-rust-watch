#![no_std]
#![no_main]

#[cfg(test)]
#[embedded_test::tests]
mod tests {
    use stm32f1xx_hal::{
        gpio,
        i2c::{BlockingI2c, DutyCycle, I2c, Mode},
        pac,
        prelude::*,
    };
    use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
    type I2c1DefaultPinsDev = BlockingI2c<
        pac::I2C1,
        (
            gpio::Pin<'B', 6, gpio::Alternate<gpio::OpenDrain>>,
            gpio::Pin<'B', 7, gpio::Alternate<gpio::OpenDrain>>,
        ),
    >;

    // An optional init function which is called before every test
    // Asyncness is optional, so is the return value
    #[init]
    fn init() -> I2c1DefaultPinsDev {
        let dp = pac::Peripherals::take().unwrap();

        let mut flash = dp.FLASH.constrain();
        let rcc = dp.RCC.constrain();
        let mut afio = dp.AFIO.constrain();

        let clocks = rcc
            .cfgr
            .use_hse(8.MHz())
            .sysclk(48.MHz())
            .pclk1(24.MHz())
            .freeze(&mut flash.acr);
        let mut gpiob = dp.GPIOB.split();

        let scl = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
        let sda = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

        I2c::i2c1(
            dp.I2C1,
            (scl, sda),
            &mut afio.mapr,
            Mode::Fast {
                frequency: 400.kHz(),
                duty_cycle: DutyCycle::Ratio16to9,
            },
            clocks,
        )
        .blocking_default(clocks)
    }

    // Tests can be async (needs feature `embassy`)
    // Tests can take the state returned by the init function (optional)
    #[test]
    fn test_oled_screen(i2c_dev: I2c1DefaultPinsDev) {
        let interface = I2CDisplayInterface::new(i2c_dev);
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
        display.init().unwrap();
        display.flush().unwrap();
    }
}
