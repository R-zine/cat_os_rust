#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(cat_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use cat_os::println;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    cat_os::init();

    #[cfg(test)]
    test_main();

    println!("It did not crash!");

    cat_os::hlt_loop();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    cat_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    cat_os::test_panic_handler(info)
}
