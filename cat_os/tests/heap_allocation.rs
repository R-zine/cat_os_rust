#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(cat_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    use cat_os::allocator;
    use cat_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    cat_os::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");
    cat_os::enable_hardware_interrupts();

    test_main();
    cat_os::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    cat_os::test_panic_handler(info)
}

use alloc::boxed::Box;

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}

use alloc::vec::Vec;

#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

use cat_os::allocator::HEAP_SIZE;

#[test_case]
fn many_boxes() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1);
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1);
}

use alloc::sync::Arc;
use cat_os::task::{
    Task,
    executor::{Executor, MAX_TASKS, SpawnError},
};
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::task::{Context, Poll};

struct WakeStorm {
    poll_count: Arc<AtomicUsize>,
}

impl Future for WakeStorm {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.poll_count.fetch_add(1, Ordering::Relaxed) == 0 {
            for _ in 0..1_000 {
                cx.waker().wake_by_ref();
            }
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

#[test_case]
fn executor_coalesces_duplicate_wakes() {
    let poll_count = Arc::new(AtomicUsize::new(0));
    let mut executor = Executor::new();
    executor
        .spawn(Task::new(WakeStorm {
            poll_count: poll_count.clone(),
        }))
        .expect("task should fit in the executor");

    executor.run_until_stalled();

    assert_eq!(poll_count.load(Ordering::Relaxed), 2);
}

#[test_case]
fn executor_reports_task_limit() {
    let mut executor = Executor::new();
    for _ in 0..MAX_TASKS {
        assert_eq!(executor.spawn(Task::new(core::future::pending())), Ok(()));
    }

    assert_eq!(
        executor.spawn(Task::new(core::future::pending())),
        Err(SpawnError::TaskLimitReached)
    );
}
