// SPDX-License-Identifier: MPL-2.0

//! TLB flush operations.

use alloc::vec::Vec;
use core::{
    ops::Range,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use super::{page::DynPage, Vaddr, PAGE_SIZE};
use crate::{
    cpu::{self, AtomicCpuSet, CpuId, CpuSet, PinCurrentCpu},
    cpu_local,
    smp::inter_processor_call,
    trap::{self, DisabledLocalIrqGuard},
};

/// A TLB flusher that is aware of which CPUs are needed to be flushed.
///
/// The flusher needs to stick to the current CPU.
pub struct TlbFlusher<'c> {
    target_cpus: &'c AtomicCpuSet,
    irq_guard: DisabledLocalIrqGuard,
    need_flush_all: bool,
    flush_ops_size: usize,
    flush_ops: [Option<TlbFlushOp>; FLUSH_ALL_OPS_THRESHOLD],
    defer_pages: Vec<(TlbFlushOp, DynPage)>,
}

impl<'c> TlbFlusher<'c> {
    /// Creates a new TLB flusher with the specified CPUs to be flushed.
    ///
    /// The flusher needs to stick to the current CPU. So please provide a
    /// guard that implements [`PinCurrentCpu`].
    pub fn new(target_cpus: &'c AtomicCpuSet) -> Self {
        Self {
            target_cpus,
            irq_guard: trap::disable_local(),
            need_flush_all: false,
            flush_ops_size: 0,
            flush_ops: [const { None }; FLUSH_ALL_OPS_THRESHOLD],
            defer_pages: Vec::new(),
        }
    }

    /// Issues a pending TLB flush request.
    ///
    /// On SMP systems, the notification is sent to all the relevant CPUs only
    /// when the remote buffer is full. Otherwise, this is non-blocking.
    pub fn issue_tlb_flush(&mut self, op: TlbFlushOp) {
        if self.need_flush_all {
            return;
        }

        let op = op.optimize_for_large_range();

        if op == TlbFlushOp::All || self.flush_ops_size >= FLUSH_ALL_OPS_THRESHOLD {
            self.need_flush_all = true;
        } else {
            self.flush_ops[self.flush_ops_size] = Some(op);
            self.flush_ops_size += 1;
        }
    }

    /// Issues a TLB flush request that must happen before dropping the page.
    ///
    /// If we need to remove a mapped page from the page table, we can only
    /// recycle the page after all the relevant TLB entries in all CPUs are
    /// flushed. Otherwise if the page is recycled for other purposes, the user
    /// space program can still access the page through the TLB entries. This
    /// method is designed to be used in such cases.
    pub fn issue_tlb_flush_with(&mut self, op: TlbFlushOp, drop_after_flush: DynPage) {
        self.defer_pages.push((op, drop_after_flush));
    }

    /// Synchronizes the TLB flush requests to all the relevant CPUs.
    pub fn sync_tlb_flush(&mut self) {
        let cpu_set = self.dispatch_tlb_flush();

        sync_tlb_with(&self.irq_guard, cpu_set);
    }

    fn dispatch_tlb_flush(&mut self) -> CpuSet {
        let mut target_cpus = self.target_cpus.load();
        let this_cpu = self.irq_guard.current_cpu();

        let need_self_flush = target_cpus.contains(this_cpu);

        if need_self_flush {
            target_cpus.remove(this_cpu);
        }

        let target_cpu_size = target_cpus.count();

        let need_remote_flush = target_cpu_size > 1;

        if need_self_flush {
            if self.need_flush_all {
                TlbFlushOp::All.perform_on_current();
            } else {
                for i in 0..self.flush_ops_size {
                    self.flush_ops[i].as_ref().unwrap().perform_on_current();
                }
                for (op, _) in &self.defer_pages {
                    op.perform_on_current();
                }
            }
        }

        if need_remote_flush {
            let mut inner = LATR_FLUSH_ARRAY.get_on_cpu(this_cpu).entries.write();
            inner.reserve(self.defer_pages.len().max(64));
            for (op, page) in self.defer_pages.drain(..) {
                inner.push((page, op, AtomicCpuSet::new(target_cpus.clone())));
            }
        }

        target_cpus
    }
}

impl Drop for TlbFlusher<'_> {
    fn drop(&mut self) {
        let _ = self.dispatch_tlb_flush();
    }
}

/// The operation to flush TLB entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlbFlushOp {
    /// Flush all TLB entries except for the global entries.
    All,
    /// Flush the TLB entry for the specified virtual address.
    Address(Vaddr),
    /// Flush the TLB entries for the specified virtual address range.
    Range(Range<Vaddr>),
}

impl TlbFlushOp {
    /// Performs the TLB flush operation on the current CPU.
    pub fn perform_on_current(&self) {
        use crate::arch::mm::{
            tlb_flush_addr, tlb_flush_addr_range, tlb_flush_all_excluding_global,
        };
        match self {
            TlbFlushOp::All => tlb_flush_all_excluding_global(),
            TlbFlushOp::Address(addr) => tlb_flush_addr(*addr),
            TlbFlushOp::Range(range) => tlb_flush_addr_range(range),
        }
    }

    fn optimize_for_large_range(self) -> Self {
        match self {
            TlbFlushOp::Range(range) => {
                if range.len() > FLUSH_ALL_RANGE_THRESHOLD {
                    TlbFlushOp::All
                } else {
                    TlbFlushOp::Range(range)
                }
            }
            _ => self,
        }
    }
}

/// If a TLB flushing request exceeds this threshold, we flush all.
const FLUSH_ALL_RANGE_THRESHOLD: usize = 32 * PAGE_SIZE;

/// If the number of pending requests exceeds this threshold, we flush all the
/// TLB entries instead of flushing them one by one.
const FLUSH_ALL_OPS_THRESHOLD: usize = 32;

// The queues of pending requests publicly seen on each CPU.
//
// On scheduler ticks or some timer interrupts, we will process the pending
// requests on all CPUs and recycle the pages on the current CPU.
cpu_local! {
    static LATR_FLUSH_ARRAY: LatrArray = LatrArray::new();
}

/// Recycle the local pages that is delayed to be recycled.
///
/// This function checks if all the issued TLB flush requests of local pages
/// are processed on all the relevant CPUs. If so, the page can be recycled.
pub(crate) fn delayed_recycle_pages(irq_guard: &DisabledLocalIrqGuard) {
    let cur_cpu = irq_guard.current_cpu();
    LATR_FLUSH_ARRAY.get_on_cpu(cur_cpu).recycle();
}

cpu_local! {
    // TLB_SYNC_ACK[TO][FROM]
    static TLB_SYNC_ACK: AtomicCpuSet = AtomicCpuSet::new(CpuSet::new_empty());
}

/// Process the pending TLB flush requests on all the CPUs.
///
/// This function checks if there are any pending TLB flush requests on all the
/// remote CPUS. If so, it will process the requests.
pub(crate) fn process_pending_sync_shootdowns(irq_guard: &DisabledLocalIrqGuard) {
    let cur_cpu = irq_guard.current_cpu();
    let mut have_flushed_all = false;
    core::sync::atomic::fence(Ordering::Acquire);
    let need_check_cpus = TLB_SYNC_ACK.get_on_cpu(cur_cpu).load();
    for cpu_id in need_check_cpus.iter() {
        if cpu_id == cur_cpu {
            continue;
        }
        if !have_flushed_all {
            TlbFlushOp::All.perform_on_current();
            have_flushed_all = true;
        }
        TLB_SYNC_ACK
            .get_on_cpu(cur_cpu)
            .remove(cpu_id, Ordering::Release);
    }
}

fn process_sync_request(from_cpu: CpuId) {
    let irq_guard = trap::disable_local();
    let cur_cpu = irq_guard.current_cpu();
    if !TLB_SYNC_ACK
        .get_on_cpu(cur_cpu)
        .contains(from_cpu, Ordering::Acquire)
    {
        return;
    }
    TlbFlushOp::All.perform_on_current();
    TLB_SYNC_ACK
        .get_on_cpu(cur_cpu)
        .remove(from_cpu, Ordering::Release);
}

pub(crate) const PROCESS_PENDING_INTERVAL: usize = 10;

fn sync_tlb_with(irq_guard: &DisabledLocalIrqGuard, cpu_set: CpuSet) {
    let cur_cpu = irq_guard.current_cpu();
    for cpu in cpu_set.iter() {
        TLB_SYNC_ACK.get_on_cpu(cpu).add(cur_cpu, Ordering::Release);
    }

    inter_processor_call(&cpu_set, process_sync_request);

    for cpu in cpu_set.iter() {
        let mut process_pending_interval = 0;
        while TLB_SYNC_ACK
            .get_on_cpu(cpu)
            .contains(cur_cpu, Ordering::Acquire)
        {
            process_pending_interval += 1;
            if process_pending_interval == PROCESS_PENDING_INTERVAL {
                // Prevent deadlock since we disabled interrupts.
                process_pending_sync_shootdowns(irq_guard);
                process_pending_interval = 0;
            }
            core::hint::spin_loop();
        }
    }
}

pub(crate) fn this_cpu_init_garbage_collection() {
    crate::timer::register_callback(|| {
        // Go one by one.
        static WHO: AtomicUsize = AtomicUsize::new(0);

        // Avoid sending IPIs to CPUs that are GCing.
        cpu_local! {
            static HICCUP: AtomicBool = AtomicBool::new(false);
        }

        let num_cpus = cpu::num_cpus();
        let who = WHO.fetch_add(1, Ordering::Relaxed) % num_cpus;
        let turn = CpuId::try_from(who).unwrap();

        if HICCUP
            .get_on_cpu(turn)
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let mut set = CpuSet::new_empty();
        set.add(turn);

        // FIXME: now only BSP receives timer interrupts. we work around this
        // by sending IPIs to other CPUs.
        inter_processor_call(&set, |_from_cpu_id| {
            let irq_guard = trap::disable_local();
            process_pending_shootdowns(&irq_guard);
            delayed_recycle_pages(&irq_guard);
            let cur_cpu = irq_guard.current_cpu();
            HICCUP.get_on_cpu(cur_cpu).store(false, Ordering::Release);
        });
    });
}

fn process_pending_shootdowns(irq_guard: &DisabledLocalIrqGuard) {
    let cur_cpu = irq_guard.current_cpu();
    let mut have_flushed_all = false;
    for from_cpu in cpu::all_cpus() {
        if from_cpu == cur_cpu {
            continue;
        }
        LATR_FLUSH_ARRAY
            .get_on_cpu(from_cpu)
            .process_remote_requests(&mut have_flushed_all, cur_cpu);
    }
    TLB_SYNC_ACK.get_on_cpu(cur_cpu).store(&CpuSet::new_empty());
    core::sync::atomic::fence(Ordering::Release);
}

struct LatrArray {
    entries: spin::RwLock<Vec<(DynPage, TlbFlushOp, AtomicCpuSet)>>,
}

impl LatrArray {
    const fn new() -> Self {
        Self {
            entries: spin::RwLock::new(Vec::new()),
        }
    }

    /// Recycle the operations that can be recycled.
    ///
    /// This should be called by the current CPU.
    fn recycle(&self) {
        let mut lock = self.entries.write();
        lock.retain(|x| x.2.load().is_empty());
    }

    /// Check the remote CPU's requests and process them.
    ///
    /// This should be called by the other CPUs.
    fn process_remote_requests(&self, have_flushed_all: &mut bool, current: CpuId) {
        let lock = self.entries.read();
        if !*have_flushed_all && lock.len() > FLUSH_ALL_OPS_THRESHOLD {
            TlbFlushOp::All.perform_on_current();
            *have_flushed_all = true;
        }
        for (_page, op, cpu_set) in lock.iter() {
            if cpu_set.contains(current, Ordering::Acquire) {
                if !*have_flushed_all {
                    op.perform_on_current();
                    if op == &TlbFlushOp::All {
                        *have_flushed_all = true;
                    }
                }
                cpu_set.remove(current, Ordering::Release);
            }
        }
    }
}
