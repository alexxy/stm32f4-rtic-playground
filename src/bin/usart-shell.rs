#![no_main]
#![no_std]

use f411_rtic_playground as _; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f4xx_hal::pac, peripherals = true)]
mod usart_shell {
    use core::task::Context;
    use core::fmt::Write;
    use dwt_systick_monotonic::{DwtSystick, ExtU32};
    use stm32f4xx_hal::{
        gpio::{gpioa::PA0, gpioa::PA9, gpioa::PA10, gpioc::PC13, Alternate, Edge, Input, Output, PullUp, PushPull},
        serial::{
            config::Config,
            Event::Rxne,
            Serial
        },
        pac::USART1,
        prelude::*,
    };

    use ushell::{
        autocomplete::StaticAutocomplete,
        control as ushell_control,
        history::LRUHistory,
        Input as ushell_input,
        ShellError as ushell_error,
        UShell,
    };

    type LedType = PC13<Output<PushPull>>;
    type ButtonType = PA0<Input<PullUp>>;
    type SerialType = Serial<USART1, (PA9<Alternate<PushPull, 7>>, PA10<Alternate<PushPull, 7>>)>;
    type ShellType = UShell<SerialType, StaticAutocomplete<6>, LRUHistory<32, 4>, 32>;

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
        let clocks = rcc.cfgr.sysclk(SYSFREQ.hz()).use_hse(25.mhz()).freeze();
        // monotonic timer
        let mono = DwtSystick::new(
            &mut ctx.core.DCB,
            ctx.core.DWT,
            ctx.core.SYST,
            clocks.hclk().0,
        );
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
        let pins = (
            gpioa.pa9.into_alternate(),
            gpioa.pa10.into_alternate(),
            );
        let mut serial = Serial::new(ctx.device.USART1, pins, Config::default().baudrate(115_200.bps()), &clocks).unwrap().with_u8_data();
        serial.listen(Rxne);
        // yshell
        let autocomplete = StaticAutocomplete(["clear", "help", "off", "on", "set ", "status"]);
        let history = LRUHistory::default();
        let shell = UShell::new(serial, autocomplete, history);

        (
            Shared {
               // Initialization of shared resources go here
               led_enabled: false,
            },
            Local {
                // Initialization of local resources go here
                button,
                led,
                shell
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
    
    #[task(binds = EXTI0, local = [button, led])]
    fn button_click(ctx: button_click::Context) {
        ctx.local.button.clear_interrupt_pending_bit();
        ctx.local.led.toggle();
    }

    #[task(binds = USART1, priority = 1, shared = [led_enabled], local = [shell])]
    fn usart1(ctx: usart1::Context) {
        let usart1::LocalResources { shell } = ctx.local;
        loop {
            match shell.poll() {
                Ok(Some(ushell_input::Command((cmd, args)))) => {
                    match cmd {
                        "help" => {
                            shell.write_str(HELP).ok();
                        }
                        "clear" => {
                            shell.clear().ok();
                        }
/*                        "on" => {
                            blink_enabled.lock(|e| *e = true);
                            shell.write_str(CR).ok();
                        }
                        "off" => {
                            blink_enabled.lock(|e| *e = false);
                            shell.write_str(CR).ok();
                        }
                        "status" => {
                            let on = blink_enabled.lock(|e| *e);
                            let status = if on { "On" } else { "Off" };
                            write!(
                                shell,
                                "{0:}Animation: {1:}{0:}Frequency: {2:}Hz{0:}",
                                CR, status, blink_freq
                            )
                            .ok();
                        }
                        "set" => match btoi::btoi(args.as_bytes()) {
                            Ok(freq) if freq > 0 && freq <= 100 => {
                                *blink_freq = freq;
                                blink_timer.lock(|t| {
                                    t.start((freq as u32 * 2).hz());
                                });
                                shell.write_str(CR).ok();
                            }
                            _ => {
                                write!(shell, "{0:}unsupported frequency{0:}", CR).ok();
                            }
                        },*/
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
