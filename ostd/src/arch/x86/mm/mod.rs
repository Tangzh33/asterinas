// SPDX-License-Identifier: MPL-2.0

#![expect(dead_code)]

use alloc::fmt;
use core::ops::Range;

use cfg_if::cfg_if;
pub(crate) use util::{
    __atomic_cmpxchg_fallible, __atomic_load_fallible, __memcpy_fallible, __memset_fallible,
};
use x86::msr::{IA32_PAT, rdmsr, wrmsr};
use x86_64::{VirtAddr, instructions::tlb, structures::paging::PhysFrame};

use crate::{
    Pod,
    mm::{
        PAGE_SIZE, Paddr, PagingConstsTrait, PagingLevel, PodOnce, Vaddr,
        page_prop::{CachePolicy, PageFlags, PageProperty, PrivilegedPageFlags as PrivFlags},
        page_table::PageTableEntryTrait,
    },
};

mod util;

pub(crate) const NR_ENTRIES_PER_PAGE: usize = 512;

#[derive(Clone, Debug, Default)]
pub struct PagingConsts {}

impl PagingConstsTrait for PagingConsts {
    const BASE_PAGE_SIZE: usize = 4096;
    const NR_LEVELS: PagingLevel = 4;
    const ADDRESS_WIDTH: usize = 48;
    const VA_SIGN_EXT: bool = true;
    const HIGHEST_TRANSLATION_LEVEL: PagingLevel = 2;
    const PTE_SIZE: usize = core::mem::size_of::<PageTableEntry>();
}

bitflags::bitflags! {
    #[derive(Pod)]
    #[repr(C)]
    /// Possible flags for a page table entry.
    pub struct PageTableFlags: usize {
        /// Specifies whether the mapped frame or page table is loaded in memory.
        const PRESENT =         1 << 0;
        /// Controls whether writes to the mapped frames are allowed.
        const WRITABLE =        1 << 1;
        /// Controls whether accesses from userspace (i.e. ring 3) are permitted.
        const USER =            1 << 2;
        /// If this bit is set, a “write-through” policy is used for the cache, else a “write-back”
        /// policy is used.
        const WRITE_THROUGH =   1 << 3;
        /// Disables caching for the pointed entry is cacheable.
        const NO_CACHE =        1 << 4;
        /// Whether this entry has been used for linear-address translation.
        const ACCESSED =        1 << 5;
        /// Whether the memory area represented by this entry is modified.
        const DIRTY =           1 << 6;
        /// In level 2 or 3 it indicates that it map to a huge page.
        /// In level 1, this is the PAT (page attribute table) bit used for cache control.
        /// We no longer use this bit for validity checking (now using VALID_PAGE bit 61).
        const HUGE =            1 << 7;
        /// Indicates that the mapping is present in all address spaces, so it isn't flushed from
        /// the TLB on an address space switch.
        const GLOBAL =          1 << 8;
        /// TDX shared bit.
        #[cfg(feature = "cvm_guest")]
        const SHARED =          1 << 51;

        /// Ignored by the hardware. Free to use.
        const HIGH_IGN1 =       1 << 52;
        /// Ignored by the hardware. Free to use.
        const HIGH_IGN2 =       1 << 53;

        /// Valid page marker. Used to indicate a valid leaf page entry.
        /// This bit replaces the previous use of HUGE bit for validity checking.
        const VALID_PAGE =      1 << 61;

        /// Forbid execute codes on the page. The NXE bits in EFER msr must be set.
        const NO_EXECUTE =      1 << 63;
    }
}

/// Flush any TLB entry that contains the map of the given virtual address.
///
/// This flush performs regardless of the global-page bit. So it can flush both global
/// and non-global entries.
pub(crate) fn tlb_flush_addr(vaddr: Vaddr) {
    tlb::flush(VirtAddr::new(vaddr as u64));
}

/// Flush any TLB entry that intersects with the given address range.
pub(crate) fn tlb_flush_addr_range(range: &Range<Vaddr>) {
    for vaddr in range.clone().step_by(PAGE_SIZE) {
        tlb_flush_addr(vaddr);
    }
}

/// Flush all TLB entries except for the global-page entries.
pub(crate) fn tlb_flush_all_excluding_global() {
    tlb::flush_all();
}

/// Flush all TLB entries, including global-page entries.
pub(crate) fn tlb_flush_all_including_global() {
    // SAFETY: updates to CR4 here only change the global-page bit, the side effect
    // is only to invalidate the TLB, which doesn't affect the memory safety.
    unsafe {
        // To invalidate all entries, including global-page
        // entries, disable global-page extensions (CR4.PGE=0).
        x86_64::registers::control::Cr4::update(|cr4| {
            *cr4 -= x86_64::registers::control::Cr4Flags::PAGE_GLOBAL;
        });
        x86_64::registers::control::Cr4::update(|cr4| {
            *cr4 |= x86_64::registers::control::Cr4Flags::PAGE_GLOBAL;
        });
    }
}

#[derive(Clone, Copy, Pod, Default)]
#[repr(C)]
pub struct PageTableEntry(usize);

/// Activates the given level 4 page table.
/// The cache policy of the root page table node is controlled by `root_pt_cache`.
///
/// # Safety
///
/// Changing the level 4 page table is unsafe, because it's possible to violate memory safety by
/// changing the page mapping.
pub unsafe fn activate_page_table(root_paddr: Paddr, root_pt_cache: CachePolicy) {
    let addr = PhysFrame::from_start_address(x86_64::PhysAddr::new(root_paddr as u64)).unwrap();
    let flags = match root_pt_cache {
        CachePolicy::Writeback => x86_64::registers::control::Cr3Flags::empty(),
        CachePolicy::Writethrough => x86_64::registers::control::Cr3Flags::PAGE_LEVEL_WRITETHROUGH,
        CachePolicy::Uncacheable => x86_64::registers::control::Cr3Flags::PAGE_LEVEL_CACHE_DISABLE,
        // WriteCombining and WriteProtected are not supported for root page table (CR3)
        // as CR3 only supports WB, WT, and UC via PCD/PWT bits
        _ => panic!("unsupported cache policy for the root page table (only WB, WT, UC allowed)"),
    };

    // SAFETY: The safety is upheld by the caller.
    unsafe { x86_64::registers::control::Cr3::write(addr, flags) };
}

pub fn current_page_table_paddr() -> Paddr {
    x86_64::registers::control::Cr3::read_raw()
        .0
        .start_address()
        .as_u64() as Paddr
}

impl PageTableEntry {
    cfg_if! {
        if #[cfg(feature = "cvm_guest")] {
            const PHYS_ADDR_MASK: usize = 0x7_FFFF_FFFF_F000;
        } else {
            const PHYS_ADDR_MASK: usize = 0xF_FFFF_FFFF_F000;
        }
    }
    const PROP_MASK: usize =
        !Self::PHYS_ADDR_MASK & !PageTableFlags::HUGE.bits() & !PageTableFlags::VALID_PAGE.bits();
}

/// Parse a bit-flag bits `val` in the representation of `from` to `to` in bits.
macro_rules! parse_flags {
    ($val:expr, $from:expr, $to:expr) => {
        ($val as usize & $from.bits() as usize) >> $from.bits().ilog2() << $to.bits().ilog2()
    };
}

impl PodOnce for PageTableEntry {}

impl PageTableEntryTrait for PageTableEntry {
    fn is_present(&self) -> bool {
        // For PT child, `PRESENT` should be set; for huge page, `HUGE` should
        // be set; for the leaf child page, `VALID_PAGE` bit should be set.
        self.0 & PageTableFlags::PRESENT.bits() != 0
            || self.0 & PageTableFlags::HUGE.bits() != 0
            || self.0 & PageTableFlags::VALID_PAGE.bits() != 0
    }

    fn new_page(paddr: Paddr, level: PagingLevel, prop: PageProperty) -> Self {
        // For x86_64:
        // - Level 1 (4KB pages): set VALID_PAGE bit (bit 61), bit 7 can be PAT
        // - Level 2/3 (huge pages): set HUGE bit (bit 7)
        let flags = if level == 1 {
            PageTableFlags::VALID_PAGE.bits()
        } else {
            PageTableFlags::HUGE.bits()
        };
        let mut pte = Self(paddr & Self::PHYS_ADDR_MASK | flags);
        pte.set_prop(prop);
        pte
    }

    fn new_pt(paddr: Paddr) -> Self {
        // In x86 if it's an intermediate PTE, it's better to have the same permissions
        // as the most permissive child (to reduce hardware page walk accesses). But we
        // don't have a mechanism to keep it generic across architectures, thus just
        // setting it to be the most permissive.
        let flags = PageTableFlags::PRESENT.bits()
            | PageTableFlags::WRITABLE.bits()
            | PageTableFlags::USER.bits();
        Self(paddr & Self::PHYS_ADDR_MASK | flags)
    }

    fn paddr(&self) -> Paddr {
        self.0 & Self::PHYS_ADDR_MASK
    }

    fn prop(&self) -> PageProperty {
        let flags = (parse_flags!(self.0, PageTableFlags::PRESENT, PageFlags::R))
            | (parse_flags!(self.0, PageTableFlags::WRITABLE, PageFlags::W))
            | (parse_flags!(!self.0, PageTableFlags::NO_EXECUTE, PageFlags::X))
            | (parse_flags!(self.0, PageTableFlags::ACCESSED, PageFlags::ACCESSED))
            | (parse_flags!(self.0, PageTableFlags::DIRTY, PageFlags::DIRTY))
            | (parse_flags!(self.0, PageTableFlags::HIGH_IGN2, PageFlags::AVAIL2));
        let priv_flags = (parse_flags!(self.0, PageTableFlags::USER, PrivFlags::USER))
            | (parse_flags!(self.0, PageTableFlags::GLOBAL, PrivFlags::GLOBAL))
            | (parse_flags!(self.0, PageTableFlags::HIGH_IGN1, PrivFlags::AVAIL1));
        #[cfg(feature = "cvm_guest")]
        let priv_flags =
            priv_flags | (parse_flags!(self.0, PageTableFlags::SHARED, PrivFlags::SHARED));

        // Determine cache policy from PAT, PCD, PWT bits
        // Note: For level 1 (4KB pages), bit 7 is PAT bit, but we don't have level info here.
        // We check VALID_PAGE bit to distinguish: if VALID_PAGE is set, it's a leaf page
        // and bit 7 should be treated as PAT. If HUGE is set without VALID_PAGE, it's a huge page.
        let is_leaf_page = (self.0 & PageTableFlags::VALID_PAGE.bits()) != 0;
        let pat = is_leaf_page && (self.0 & PageTableFlags::HUGE.bits()) != 0;
        let pcd = (self.0 & PageTableFlags::NO_CACHE.bits()) != 0;
        let pwt = (self.0 & PageTableFlags::WRITE_THROUGH.bits()) != 0;

        // PAT index encoding programmed by `configure_pat`:
        // PAT PCD PWT -> Memory Type
        //  0   0   0  -> WB (Writeback)
        //  0   0   1  -> WT (Writethrough)
        //  0   1   0  -> UC- (Uncacheable minus)
        //  0   1   1  -> UC (Uncacheable)
        //  1   0   0  -> WC (Write Combining)
        //  1   0   1  -> WP (Write Protected)
        //  1   1   0  -> UC (Uncacheable)
        //  1   1   1  -> UC (Uncacheable)
        let cache = match (pat, pcd, pwt) {
            (false, false, false) => CachePolicy::Writeback,
            (false, false, true) => CachePolicy::Writethrough,
            (false, true, _) => CachePolicy::Uncacheable,
            (true, false, false) => CachePolicy::WriteCombining,
            (true, false, true) => CachePolicy::WriteProtected,
            (true, true, _) => CachePolicy::Uncacheable,
        };

        PageProperty {
            flags: PageFlags::from_bits(flags as u8).unwrap(),
            cache,
            priv_flags: PrivFlags::from_bits(priv_flags as u8).unwrap(),
        }
    }

    fn set_prop(&mut self, prop: PageProperty) {
        if !self.is_present() {
            return;
        }
        let mut flags = PageTableFlags::empty().bits();
        flags |= (parse_flags!(prop.flags.bits(), PageFlags::R, PageTableFlags::PRESENT))
            | (parse_flags!(prop.flags.bits(), PageFlags::W, PageTableFlags::WRITABLE))
            | (parse_flags!(!prop.flags.bits(), PageFlags::X, PageTableFlags::NO_EXECUTE))
            | (parse_flags!(
                prop.flags.bits(),
                PageFlags::ACCESSED,
                PageTableFlags::ACCESSED
            ))
            | (parse_flags!(prop.flags.bits(), PageFlags::DIRTY, PageTableFlags::DIRTY))
            | (parse_flags!(
                prop.priv_flags.bits(),
                PrivFlags::AVAIL1,
                PageTableFlags::HIGH_IGN1
            ))
            | (parse_flags!(
                prop.flags.bits(),
                PageFlags::AVAIL2,
                PageTableFlags::HIGH_IGN2
            ))
            | (parse_flags!(
                prop.priv_flags.bits(),
                PrivFlags::USER,
                PageTableFlags::USER
            ))
            | (parse_flags!(
                prop.priv_flags.bits(),
                PrivFlags::GLOBAL,
                PageTableFlags::GLOBAL
            ));
        #[cfg(feature = "cvm_guest")]
        {
            flags |= parse_flags!(
                prop.priv_flags.bits(),
                PrivFlags::SHARED,
                PageTableFlags::SHARED
            );
        }

        // Set cache policy using PAT, PCD, PWT bits
        // For 4KB pages (VALID_PAGE set), bit 7 is PAT
        // For huge pages (HUGE set), bit 7 is HUGE flag, cannot use PAT
        // Standard PAT encoding:
        //   WB: PAT=0, PCD=0, PWT=0
        //   WT: PAT=0, PCD=0, PWT=1
        //   UC: PAT=0, PCD=1, PWT=x
        //   WC: PAT=1, PCD=0, PWT=0 (4KB pages only)
        //   WP: PAT=1, PCD=0, PWT=1 (4KB pages only)

        // Preserve the HUGE and VALID_PAGE bits to determine page type
        let is_4kb_page = (self.0 & PageTableFlags::VALID_PAGE.bits()) != 0;
        let is_huge_page = (self.0 & PageTableFlags::HUGE.bits()) != 0;

        // Preserve HUGE or VALID_PAGE bit in flags
        if is_4kb_page {
            flags |= PageTableFlags::VALID_PAGE.bits();
        } else if is_huge_page {
            flags |= PageTableFlags::HUGE.bits();
        }

        match prop.cache {
            CachePolicy::Writeback => {
                // PAT=0, PCD=0, PWT=0 (default, no bits set)
            }
            CachePolicy::Writethrough => {
                // PAT=0, PCD=0, PWT=1
                flags |= PageTableFlags::WRITE_THROUGH.bits();
            }
            CachePolicy::Uncacheable => {
                // PAT=0, PCD=1, PWT=0
                flags |= PageTableFlags::NO_CACHE.bits();
            }
            CachePolicy::WriteCombining => {
                // PAT=1, PCD=0, PWT=0
                // Only valid for 4KB pages; for 4KB pages, set bit 7 as PAT
                if is_4kb_page {
                    flags |= PageTableFlags::HUGE.bits(); // This is PAT bit for level 1
                } else {
                    panic!("WriteCombining only supported for 4KB pages, not huge pages");
                }
            }
            CachePolicy::WriteProtected => {
                // PAT=1, PCD=0, PWT=1
                // Only valid for 4KB pages
                if is_4kb_page {
                    flags |= PageTableFlags::HUGE.bits() | PageTableFlags::WRITE_THROUGH.bits();
                } else {
                    panic!("WriteProtected only supported for 4KB pages, not huge pages");
                }
            }
        }
        self.0 = self.0 & !Self::PROP_MASK | flags;
    }

    fn is_last(&self, _level: PagingLevel) -> bool {
        // A PTE is "last" (leaf mapping) if:
        // 1. It has HUGE bit set (huge page at level 2/3), OR
        // 2. It has VALID_PAGE bit set (4KB page at level 1)
        self.0 & PageTableFlags::HUGE.bits() != 0 || self.0 & PageTableFlags::VALID_PAGE.bits() != 0
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_struct("PageTableEntry");
        f.field("raw", &format_args!("{:#x}", self.0))
            .field("paddr", &format_args!("{:#x}", self.paddr()))
            .field("present", &self.is_present())
            .field(
                "flags",
                &PageTableFlags::from_bits_truncate(self.0 & !Self::PHYS_ADDR_MASK),
            )
            .field("prop", &self.prop())
            .finish()
    }
}

const PAT_ENTRIES: [u64; 8] = [0x06, 0x04, 0x07, 0x00, 0x01, 0x05, 0x00, 0x00];
const PROGRAMMED_PAT: u64 = PAT_ENTRIES[0]
    | (PAT_ENTRIES[1] << 8)
    | (PAT_ENTRIES[2] << 16)
    | (PAT_ENTRIES[3] << 24)
    | (PAT_ENTRIES[4] << 32)
    | (PAT_ENTRIES[5] << 40)
    | (PAT_ENTRIES[6] << 48)
    | (PAT_ENTRIES[7] << 56);

/// Programs the PAT MSR so that write-combining mappings use the correct memory type.
pub(crate) fn configure_pat() {
    // Desired PAT entries (index 0..7):
    //   0: Write-back (WB)
    //   1: Write-through (WT)
    //   2: Uncacheable minus (UC-)
    //   3: Uncacheable (UC)
    //   4: Write-combining (WC)
    //   5: Write-protected (WP)
    //   6: Uncacheable (UC)
    //   7: Uncacheable (UC)

    unsafe {
        let current = rdmsr(IA32_PAT);
        if current != PROGRAMMED_PAT {
            wrmsr(IA32_PAT, PROGRAMMED_PAT);
        }
    }
}
