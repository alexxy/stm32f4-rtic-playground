#![no_main]
#![no_std]

use stm32f4_rtic_playground as _; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f4xx_hal::pac, peripherals = true, dispatchers = [SDIO])]
mod usart_shell {
    use core::fmt::Write;
    use dwt_systick_monotonic::DwtSystick;
    use stm32f4xx_hal::{
        gpio::{gpioa::PA0, gpioc::PC13, Edge, Input, Output, PushPull},
        pac::USART1,
        prelude::*,
        serial::{config::Config, Event::RxNotEmpty, Serial},
    };

    use ushell::{
        autocomplete::StaticAutocomplete, history::LRUHistory, Input as ushell_input,
        ShellError as ushell_error, UShell,
    };

    type LedType = PC13<Output<PushPull>>;
    type ButtonType = PA0<Input>;
    type ShellType = UShell<Serial<USART1>, StaticAutocomplete<5>, LRUHistory<32, 4>, 32>;

    const SHELL_PROMPT: &str = "#> ";
    const CR: &str = "\r\n";
    const HELP: &str = "\r\n\
        help: !
        ";
    const SYSFREQ: u32 = 100_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type Mono = DwtSystick<SYSFREQ>;
    // Shared resources go here
    #[shared]
    struct Shared {
        led_enabled: bool,
    }

    // Local resources go here
    #[local]
    struct Local {
        button: ButtonType,
        led: LedType,
        shell: ShellType,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("init");
        // syscfg
        let mut syscfg = ctx.device.SYSCFG.constrain();
        // clocks
        let rcc = ctx.device.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(SYSFREQ.Hz()).use_hse(25.MHz()).freeze();
        // monotonic timer
        let mono = DwtSystick::new(&mut ctx.core.DCB, ctx.core.DWT, ctx.core.SYST, SYSFREQ);
        // gpio ports A and C
        let gpioa = ctx.device.GPIOA.split();
        let gpioc = ctx.device.GPIOC.split();
        // button
        let mut button = gpioa.pa0.into_pull_up_input();
        button.make_interrupt_source(&mut syscfg);
        button.enable_interrupt(&mut ctx.device.EXTI);
        button.trigger_on_edge(&mut ctx.device.EXTI, Edge::Falling);
        // led
        let led = gpioc.pc13.into_push_pull_output();
        // serial
        let pins = (gpioa.pa9, gpioa.pa10);
        let mut serial = Serial::new(
            ctx.device.USART1,
            pins,
            Config::default().baudrate(115_200.bps()).wordlength_8(),
            &clocks,
        )
        .unwrap()
        .with_u8_data();
        serial.listen(RxNotEmpty);
        // ushell
        let autocomplete = StaticAutocomplete(["clear", "help", "off", "on", "status"]);
        let history = LRUHistory::default();
        let shell = UShell::new(serial, autocomplete, history);

        (
            Shared {
                // Initialization of shared resources go here
                led_enabled: true,
            },
            Local {
                // Initialization of local resources go here
                button,
                led,
                shell,
            },
            init::Monotonics(mono),
        )
    }

    // Optional idle, can be removed if not needed.
    #[idle]
    fn idle(_: idle::Context) -> ! {
        defmt::info!("idle");
        loop {
            continue;
        }
    }

    #[task(local = [led], shared = [led_enabled])]
    fn setled(ctx: setled::Context) {
        defmt::info!("Led!");
        let setled::LocalResources { led } = ctx.local;
        let setled::SharedResources { mut led_enabled } = ctx.shared;
        let led_on = led_enabled.lock(|e| *e);
        if led_on {
            led.set_low();
        } else {
            led.set_high();
        }
    }

    #[task(binds = EXTI0, local = [button], shared = [led_enabled])]
    fn button_click(mut ctx: button_click::Context) {
        defmt::info!("button");
        ctx.local.button.clear_interrupt_pending_bit();
        let led_on = ctx.shared.led_enabled.lock(|e| *e);
        if led_on {
            ctx.shared.led_enabled.lock(|e| *e = false);
            defmt::info!("Led set off!");
        } else {
            ctx.shared.led_enabled.lock(|e| *e = true);
            defmt::info!("Led set on!");
        }
        setled::spawn().unwrap();
        defmt::info!("Led change spawned!");
    }

    #[task(binds = USART1, priority = 1, shared = [led_enabled], local = [shell])]
    fn usartshell(ctx: usartshell::Context) {
        defmt::info!("usart");
        let usartshell::LocalResources { shell } = ctx.local;
        let usartshell::SharedResources { mut led_enabled } = ctx.shared;
        loop {
            match shell.poll() {
                Ok(Some(ushell_input::Command((cmd, _args)))) => {
                    match cmd {
                        "help" => {
                            shell.write_str(HELP).ok();
                        }
                        "clear" => {
                            shell.clear().ok();
                        }
                        "on" => {
                            led_enabled.lock(|e| *e = true);
                            defmt::info!("Led set on!");
                            setled::spawn().unwrap();
                            defmt::info!("led change spawned");
                            shell.write_str(CR).ok();
                        }
                        "off" => {
                            led_enabled.lock(|e| *e = false);
                            defmt::info!("Led set off!");
                            setled::spawn().unwrap();
                            defmt::info!("led change spawned");
                            shell.write_str(CR).ok();
                        }
                        "status" => {
                            let on = led_enabled.lock(|e| *e);
                            let status = if on { "On" } else { "Off" };
                            write!(shell, "{0:}LED: {1:}{0:}", CR, status).ok();
                        }
                        "" => {
                            shell.write_str(CR).ok();
                        }
                        _ => {
                            write!(shell, "{0:}unsupported command{0:}", CR).ok();
                        }
                    }
                    shell.write_str(SHELL_PROMPT).ok();
                }
                Err(ushell_error::WouldBlock) => break,
                _ => {}
            }
        }
    }
}
