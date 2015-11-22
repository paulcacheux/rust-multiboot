//! Multiboot v1 library
//!
//! # Additional documentation
//!   * https://www.gnu.org/software/grub/manual/multiboot/multiboot.html
//!   * http://git.savannah.gnu.org/cgit/grub.git/tree/doc/multiboot.texi?h=multiboot
//!

#![feature(core_slice_ext, no_std, raw)]
#![no_std]

#![crate_name = "multiboot"]
#![crate_type = "lib"]

#[cfg(test)]
extern crate std;

use core::mem::{size_of, transmute};
use core::raw;
use core::str;
use core::slice;
use core::fmt;

/// Value found in %rax after multiboot jumps to our entry point.
pub const SIGNATURE_RAX: u64 = 0x2BADB002;

pub type PAddr = u64;

/// Multiboot struct clients mainly interact with
/// To create this use Multiboot::new()
pub struct Multiboot<'a> {
    header: &'a MultibootInfo,
    paddr_to_slice: unsafe fn(PAddr, usize) -> Option<&'a [u8]>,
}

/// Representation of Multiboot header according to specification.
////
///<rawtext>
///         +-------------------+
/// 0       | flags             |    (required)
///         +-------------------+
/// 4       | mem_lower         |    (present if flags[0] is set)
/// 8       | mem_upper         |    (present if flags[0] is set)
///         +-------------------+
/// 12      | boot_device       |    (present if flags[1] is set)
///         +-------------------+
/// 16      | cmdline           |    (present if flags[2] is set)
///         +-------------------+
/// 20      | mods_count        |    (present if flags[3] is set)
/// 24      | mods_addr         |    (present if flags[3] is set)
///         +-------------------+
/// 28 - 40 | syms              |    (present if flags[4] or
///         |                   |                flags[5] is set)
///         +-------------------+
/// 44      | mmap_length       |    (present if flags[6] is set)
/// 48      | mmap_addr         |    (present if flags[6] is set)
///         +-------------------+
/// 52      | drives_length     |    (present if flags[7] is set)
/// 56      | drives_addr       |    (present if flags[7] is set)
///         +-------------------+
/// 60      | config_table      |    (present if flags[8] is set)
///         +-------------------+
/// 64      | boot_loader_name  |    (present if flags[9] is set)
///         +-------------------+
/// 68      | apm_table         |    (present if flags[10] is set)
///         +-------------------+
/// 72      | vbe_control_info  |    (present if flags[11] is set)
/// 76      | vbe_mode_info     |
/// 80      | vbe_mode          |
/// 82      | vbe_interface_seg |
/// 84      | vbe_interface_off |
/// 86      | vbe_interface_len |
///         +-------------------+
///</rawtext>
///
#[derive(Debug)]
#[repr(C, packed)]
struct MultibootInfo {
    flags: u32,

    mem_lower: u32,
    mem_upper: u32,

    boot_device: BootDevice,

    /// The command line is a normal C-style zero-terminated string.
    cmdline: u32,

    mods_count: u32,
    mods_addr: u32,

    elf_symbols: ElfSymbols,

    mmap_length: u32,
    mmap_addr: u32,

    drives_length: u32,
    drives_addr: u32,

    config_table: u32,

    boot_loader_name: u32,

    apm_table: u32,

    vbe_control_info: u32,
    vbe_mode_info: u32,
    vbe_mode: u16,
    vbe_interface_off: u16,
    vbe_interface_len: u16
}

/// Convert a C string into a [u8 slice and from there into a &'static str.
/// This unsafe block builds on assumption that multiboot strings are sane.
fn convert_safe_c_string(cstring: *const u8) -> &'static str {
    unsafe {
        let mut iter = cstring;
        while *iter != 0 {
            iter = iter.offset(1);
        }

        let slice = raw::Slice { data: cstring, len: iter as usize - cstring as usize };
        let byte_array: &'static [u8] = transmute(slice);
        str::from_utf8_unchecked(byte_array)
    }
}

macro_rules! check_flag {
    ($doc:meta, $fun:ident, $bit:expr) => (
        #[$doc]
        fn $fun(&self) -> bool {
            //assert!($bit <= 31);
            (self.header.flags & (1 << $bit)) > 0
        }
    );

    // syms field is valid if bit 4 or 5 is set, wtf?
    ($doc:meta, $fun:ident, $bit1:expr, $bit2:expr) => (
        #[$doc]
        fn $fun(&self) -> bool {
            //assert!($bit1 <= 31);
            //assert!($bit2 <= 31);
            (self.header.flags & (1 << $bit1)) > 0 || (self.header.flags & (1 << $bit2)) > 0
        }
    );
}

/// Multiboot structure.
impl<'a> Multiboot<'a> {

    /// Initializes the multiboot structure.
    ///
    /// # Arguments
    ///
    ///  * `mboot_ptr` - The physical address of the multiboot header. On qemu for example
    ///                  this is typically at 0x9500.
    ///  * `paddr_to_slice` - Translation of the physical addresses into kernel addresses.
    ///
    ///  `paddr_to_slice` translates physical addr + size into a kernel accessible slice.
    ///  The simplest paddr_to_slice function would for example be just the identity
    ///  function. But this may vary depending on how your page table layout looks like.
    ///
    /// # Safety
    /// The user must ensure that mboot_ptr holds the physical address of a valid
    /// Multiboot1 structure and that paddr_to_slice provides correct translations.
    pub unsafe fn new(mboot_ptr: PAddr,
                      paddr_to_slice: unsafe fn(paddr: PAddr, sz: usize) -> Option<&'a [u8]>) -> Option<Multiboot<'a>> {
        paddr_to_slice(mboot_ptr, size_of::<MultibootInfo>()).map(|inner| {
            let info = transmute(inner.as_ptr());
            Multiboot { header: info, paddr_to_slice: paddr_to_slice }
        })
    }

    unsafe fn cast<T>(&self, addr: PAddr) -> Option<&T> {
        (self.paddr_to_slice)(addr, size_of::<T>()).map(|inner| {
            transmute(inner.as_ptr())
        })
    }

    check_flag!(doc = "If true, then the `mem_upper` and `mem_lower` fields are valid.",
               has_memory_bounds, 0);
    check_flag!(doc = "If true, then the `boot_device` field is valid.",
               has_boot_device, 1);
    check_flag!(doc = "If true, then the `cmdline` field is valid.",
               has_cmdline, 2);
    check_flag!(doc = "If true, then the `mods_addr` and `mods_count` fields are valid.",
               has_modules, 3);
    check_flag!(doc = "If true, then the `syms` field is valid.",
               has_symbols, 4, 5);
    check_flag!(doc = "If true, then the `mmap_addr` and `mmap_length` fields are valid.",
               has_memory_map, 6);
    check_flag!(doc = "If true, then the `drives_addr` and `drives_length` fields are valid.",
               has_drives, 7);
    check_flag!(doc = "If true, then the `config_table` field is valid.",
               has_config_table, 8);
    check_flag!(doc = "If true, then the `boot_loader_name` field is valid.",
               has_boot_loader_name, 9);
    check_flag!(doc = "If true, then the `apm_table` field is valid.",
               has_apm_table, 10);
    check_flag!(doc = "If true, then the `vbe_*` fields are valid.",
               has_vbe, 11);

    /// Indicate the amount of lower memory in kilobytes.
    ///
    /// Lower memory starts at address 0. The maximum possible value for
    /// lower memory is 640 kilobytes.
    pub fn lower_memory_bound(&self) -> Option<u32> {
        match self.has_memory_bounds() {
            true => Some(self.header.mem_lower),
            false => None
        }
    }

    /// Indicate the amount of upper memory in kilobytes.
    ///
    /// Upper memory starts at address 1 megabyte.
    /// The value returned for upper memory is maximally the address of
    /// the first upper memory hole minus 1 megabyte. It is not guaranteed
    /// to be this value.
    pub fn upper_memory_bound(&self) -> Option<u32> {
        match self.has_memory_bounds() {
            true => Some(self.header.mem_upper),
            false => None
        }
    }

    /// Indicates which bios disk device the boot loader loaded the OS image from.
    ///
    /// If the OS image was not loaded from a bios disk, then this
    /// returns None.
    /// The operating system may use this field as a hint for determining its
    /// own root device, but is not required to.
    pub fn boot_device(&self) -> Option<BootDevice> {
        match self.has_boot_device() {
            true => Some(self.header.boot_device.clone()),
            false => None
        }
    }

    /// Command line to be passed to the kernel.
    pub fn command_line(&self) -> Option<&'static str> {
        if self.has_cmdline() {
            unsafe {
                self.cast(self.header.cmdline as PAddr)
                    .map(|cstring| convert_safe_c_string(cstring))
            }
        } else {
            None
        }
    }

    /// Discover all additional modules in multiboot.
    pub fn modules(&'a self) -> Option<ModuleIter> {
        if self.has_modules() {
            unsafe {
                (self.paddr_to_slice)(self.header.mods_addr as PAddr,
                                      self.header.mods_count as usize *
                                      size_of::<MBModule>()).map(|slice| {
                                          let ptr = transmute(slice.as_ptr());
                                          let mods = slice::from_raw_parts(ptr,
                                                                           self.header.mods_count as usize);
                                          ModuleIter { mb: &self, mods: mods}
                                      })
            }
        } else {
            None
        }
    }

    /// Discover all memory regions in the multiboot memory map.
    pub fn memory_regions(&'a self) -> Option<MemoryMapIter> {
        match self.has_memory_map() {
            true => {
                let start = self.header.mmap_addr;
                let end = self.header.mmap_addr + self.header.mmap_length;
                Some(MemoryMapIter { current: start, end: end, mb: self })
            }
            false => None
        }
    }
}


/// The ‘boot_device’ field.
///
/// Partition numbers always start from zero. Unused partition
/// bytes must be set to 0xFF. For example, if the disk is partitioned
/// using a simple one-level DOS partitioning scheme, then
/// ‘part’ contains the DOS partition number, and ‘part2’ and ‘part3’
/// are both 0xFF. As another example, if a disk is partitioned first into
/// DOS partitions, and then one of those DOS partitions is subdivided
/// into several BSD partitions using BSD's disklabel strategy, then ‘part1’
/// contains the DOS partition number, ‘part2’ contains the BSD sub-partition
/// within that DOS partition, and ‘part3’ is 0xFF.
///
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct BootDevice {
    /// Contains the bios drive number as understood by
    /// the bios INT 0x13 low-level disk interface: e.g. 0x00 for the
    /// first floppy disk or 0x80 for the first hard disk.
    pub drive: u8,
    /// Specifies the top-level partition number.
    pub partition1: u8,
    /// Specifies a sub-partition in the top-level partition
    pub partition2: u8,
    /// Specifies a sub-partition in the 2nd-level partition
    pub partition3: u8
}

impl BootDevice {

    /// Is partition1 a valid partition?
    pub fn partition1_is_valid(&self) -> bool {
        self.partition1 != 0xff
    }

    /// Is partition2 a valid partition?
    pub fn partition2_is_valid(&self) -> bool {
        self.partition2 != 0xff
    }

    /// Is partition3 a valid partition?
    pub fn partition3_is_valid(&self) -> bool {
        self.partition3 != 0xff
    }
}

/// Types that define if the memory is usable or not.
#[derive(Debug, PartialEq, Eq)]
pub enum MemoryType {
    RAM = 1,
    Unusable = 2,
}

/// Multiboot format of the MMAP buffer.
///
/// Note that size is defined to be at -4 bytes in multiboot.
#[derive(Debug)]
#[repr(C, packed)]
pub struct MemoryEntry {
    size: u32,
    base_addr: u64,
    length: u64,
    mtype: u32
}

impl MemoryEntry {

    /// Get base of memory region.
    pub fn base_address(&self) -> PAddr {
        self.base_addr as PAddr
    }

    /// Get size of the memory region.
    pub fn length(&self) -> u64 {
        self.length
    }

    /// Is the region type valid RAM?
    pub fn memory_type(&self) -> MemoryType {
        match self.mtype {
            1 => MemoryType::RAM,
            _ => MemoryType::Unusable
        }
    }
}

/// Used to iterate over all memory regions provided by multiboot.
pub struct MemoryMapIter<'a> {
    mb: &'a Multiboot<'a>,
    current: u32,
    end: u32,
}

impl<'a> Iterator for MemoryMapIter<'a> {
    type Item = &'a MemoryEntry;

    #[inline]
    fn next(&mut self) -> Option<&'a MemoryEntry> {
        if self.current < self.end {
            unsafe {
                self.mb.cast(self.current as PAddr).map(|region: &'a MemoryEntry| {
                    self.current += region.size + 4;
                    region
                })
            }
        } else {
            None
        }
    }
}

/// Multiboot format to information about module
#[derive(Debug)]
#[repr(C, packed)]
struct MBModule {
    /// Start address of module in memory.
    start: u32,

    /// End address of module in memory.
    end: u32,

    /// The `string` field provides an arbitrary string to be associated
    /// with that particular boot module.
    ///
    /// It is a zero-terminated ASCII string, just like the kernel command line.
    /// The `string` field may be 0 if there is no string associated with the module.
    /// Typically the string might be a command line (e.g. if the operating system
    /// treats boot modules as executable programs), or a pathname
    /// (e.g. if the operating system treats boot modules as files in a file system),
    /// but its exact use is specific to the operating system.
    string: u32,

    /// Must be zero.
    reserved: u32
}

/// Information about a module in multiboot.
pub struct Module {
    /// Start address of module in physical memory.
    pub start: PAddr,
    /// End address of module in physic memory.
    pub end: PAddr,
    /// Name of the module.
    pub string: &'static str
}

impl Module {
    fn new(start: PAddr, end: PAddr, name: &'static str) -> Module {
        Module{start: start, end: end, string: name}
    }
}

impl fmt::Debug for Module {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Module {}: {:x} - {:x}", self.string, self.start, self.end)
    }
}

/// Used to iterate over all modules in multiboot.
pub struct ModuleIter<'a> {
    mb: &'a Multiboot<'a>,
    mods: &'a [MBModule],
}

impl<'a> Iterator for ModuleIter<'a> {
    type Item = Module;

    #[inline]
    fn next(&mut self) -> Option<Module> {
        self.mods.split_first().and_then(|(first, rest)| {
            self.mods = rest;
            unsafe {
                self.mb.cast(first.string as PAddr).map(|cstring| {
                    Module::new(first.start as PAddr,
                                first.end as PAddr,
                                convert_safe_c_string(cstring))
                })
            }
        })
    }
}

/// Multiboot format for ELF Symbols
#[derive(Debug)]
#[repr(C, packed)]
struct ElfSymbols {
    num: u32,
    size: u32,
    addr: u32,
    shndx: u32,
}
