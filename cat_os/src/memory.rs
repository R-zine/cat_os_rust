use x86_64::structures::paging::OffsetPageTable;
use x86_64::{VirtAddr, structures::paging::PageTable};

use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageSize, PhysFrame, Size4KiB},
};

/// Initialize a new OffsetPageTable.
///
/// # Safety
///
/// The caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

/// Creates an example mapping for the given page to VGA frame `0xb8000`.
///
/// # Safety
///
/// The caller must ensure that `page` is unused and that aliasing the VGA
/// frame cannot violate Rust's reference aliasing rules. This helper is only
/// intended for controlled paging tests.
pub unsafe fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe { mapper.map_to(page, frame, flags, frame_allocator) };
    map_to_result.expect("map_to failed").flush();
}

pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

use bootloader::bootinfo::MemoryMap;

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    region_index: usize,
    next_frame_number: u64,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            region_index: 0,
            next_frame_number: 0,
        }
    }
}

use bootloader::bootinfo::MemoryRegionType;

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        while let Some(region) = self.memory_map.get(self.region_index) {
            if region.region_type != MemoryRegionType::Usable {
                self.region_index += 1;
                self.next_frame_number = 0;
                continue;
            }

            let frame_number = self.next_frame_number.max(region.range.start_frame_number);
            if frame_number < region.range.end_frame_number {
                self.next_frame_number = frame_number + 1;
                let address = PhysAddr::new(frame_number * Size4KiB::SIZE);
                return Some(PhysFrame::containing_address(address));
            }

            self.region_index += 1;
            self.next_frame_number = 0;
        }

        None
    }
}
