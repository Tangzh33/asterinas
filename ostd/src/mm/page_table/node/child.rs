// SPDX-License-Identifier: MPL-2.0

//! This module specifies the type of the children of a page table node.

use core::{mem::ManuallyDrop, panic};

use super::{page::DynPageRef, PageTableEntryTrait, RawNodeRef, RawPageTableNode};
use crate::{
    arch::mm::{PageTableEntry, PagingConsts},
    mm::{
        page::{meta::MapTrackingStatus, DynPage},
        page_prop::PageProperty,
        vm_space::Token,
        Paddr, PagingConstsTrait, PagingLevel,
    },
};

/// A child of a page table node.
///
/// This is a owning handle to a child of a page table node. If the child is
/// either a page table node or a page, it holds a reference count to the
/// corresponding page.
#[derive(Debug)]
pub(in crate::mm) enum Child<
    E: PageTableEntryTrait = PageTableEntry,
    C: PagingConstsTrait = PagingConsts,
> where
    [(); C::NR_LEVELS as usize]:,
{
    PageTable(RawPageTableNode<E, C>),
    Page(DynPage, PageProperty),
    /// Pages not tracked by handles.
    Untracked(Paddr, PagingLevel, PageProperty),
    Token(Token),
    None,
}

#[derive(Debug)]
pub(in crate::mm) enum ChildRef<
    'a,
    E: PageTableEntryTrait = PageTableEntry,
    C: PagingConstsTrait = PagingConsts,
> where
    [(); C::NR_LEVELS as usize]:,
{
    PageTable(RawNodeRef<'a, E, C>),
    Page(DynPageRef<'a>, PageProperty),
    /// Pages not tracked by handles.
    Untracked(Paddr, PagingLevel, PageProperty),
    Token(Token),
    None,
}

impl<E: PageTableEntryTrait, C: PagingConstsTrait> ChildRef<'_, E, C>
where
    [(); C::NR_LEVELS as usize]:,
{
    /// Converts a PTE back to a child.
    ///
    /// # Safety
    ///
    /// The provided PTE must be originated from [`Child::into_pte`]. And the
    /// provided information (level and tracking status) must be the same with
    /// the lost information during the conversion. Strictly speaking, the
    /// provided arguments must be compatible with the original child (
    /// specified by [`Child::is_compatible`]).
    ///
    /// This method should be only used no more than once for a PTE that has
    /// been converted from a child using the [`Child::into_pte`] method.
    ///
    /// The reference mustn't outlive the lifetime of the PTE.
    pub(super) unsafe fn from_pte(
        pte: &E,
        level: PagingLevel,
        is_tracked: MapTrackingStatus,
    ) -> Self {
        if !pte.is_present() {
            let paddr = pte.paddr();
            if paddr == 0 {
                return Self::None;
            } else {
                // SAFETY: The physical address is written as a valid token.
                return Self::Token(unsafe { Token::from_raw_inner(paddr) });
            }
        }

        let paddr = pte.paddr();

        if !pte.is_last(level) {
            // SAFETY: The physical address points to a valid page table node
            // at the given level.
            return Self::PageTable(unsafe { RawNodeRef::from_raw_parts(paddr, level - 1) });
        }

        match is_tracked {
            MapTrackingStatus::Tracked => {
                // SAFETY: The physical address points to a valid page.
                let page = unsafe { DynPageRef::from_raw(paddr) };
                Self::Page(page, pte.prop())
            }
            MapTrackingStatus::Untracked => ChildRef::Untracked(paddr, level, pte.prop()),
            MapTrackingStatus::NotApplicable => panic!("Invalid tracking status"),
        }
    }

    pub(in crate::mm) fn to_owned(&self) -> Child<E, C> {
        match self {
            ChildRef::PageTable(node) => Child::PageTable((*node.deref()).clone_shallow()),
            ChildRef::Page(page, prop) => Child::Page((*page).clone(), *prop),
            ChildRef::Untracked(pa, level, prop) => Child::Untracked(*pa, *level, *prop),
            ChildRef::Token(token) => Child::Token(*token),
            ChildRef::None => Child::None,
        }
    }

    pub(super) unsafe fn assume_owned(self) -> Child<E, C> {
        match self {
            ChildRef::PageTable(node) => Child::PageTable(unsafe {
                RawPageTableNode::from_raw_parts(node.deref().paddr(), node.deref().level())
            }),
            ChildRef::Page(page, prop) => {
                Child::Page(unsafe { DynPage::from_raw(page.paddr()) }, prop)
            }
            ChildRef::Untracked(pa, level, prop) => Child::Untracked(pa, level, prop),
            ChildRef::Token(token) => Child::Token(token),
            ChildRef::None => Child::None,
        }
    }
}

impl<E: PageTableEntryTrait, C: PagingConstsTrait> Child<E, C>
where
    [(); C::NR_LEVELS as usize]:,
{
    /// Returns whether the child is compatible with the given node.
    ///
    /// In other words, it checks whether the child can be a child of a node
    /// with the given level and tracking status.
    pub(super) fn is_compatible(
        &self,
        node_level: PagingLevel,
        is_tracked: MapTrackingStatus,
    ) -> bool {
        match self {
            Child::PageTable(pt) => node_level == pt.level() + 1,
            Child::Page(p, _) => {
                if node_level != p.level() {
                    log::error!(
                        "Incompatible mapped page: node_level={}, page_level={}",
                        node_level,
                        p.level()
                    );
                }
                if is_tracked != MapTrackingStatus::Tracked {
                    log::error!("Incompatible mapped page: is_tracked={:?}", is_tracked);
                }
                node_level == p.level() && is_tracked == MapTrackingStatus::Tracked
            }
            Child::Untracked(_, level, _) => {
                node_level == *level && is_tracked == MapTrackingStatus::Untracked
            }
            Child::None | Child::Token(_) => true,
        }
    }

    /// Converts a child into a owning PTE.
    ///
    /// By conversion it loses information about whether the page is tracked
    /// or not. Also it loses the level information. However, the returned PTE
    /// takes the ownership (reference count) of the child.
    ///
    /// Usually this is for recording the PTE into a page table node. When the
    /// child is needed again by reading the PTE of a page table node, extra
    /// information should be provided using the [`Child::from_pte`] method.
    pub(super) fn into_pte(self) -> E {
        match self {
            Child::PageTable(pt) => {
                let pt = ManuallyDrop::new(pt);
                E::new_pt(pt.paddr())
            }
            Child::Page(page, prop) => {
                let level = page.level();
                E::new_page(page.into_raw(), level, prop)
            }
            Child::Untracked(pa, level, prop) => E::new_page(pa, level, prop),
            Child::None => E::new_absent(),
            Child::Token(token) => E::new_token(token),
        }
    }
}
