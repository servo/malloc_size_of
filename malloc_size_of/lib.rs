// Copyright 2016-2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A crate for measuring the heap usage of data structures in a way that
//! integrates with Firefox's memory reporting, particularly the use of
//! mozjemalloc and DMD. In particular, it has the following features.
//! - It isn't bound to a particular heap allocator.
//! - It provides traits for both "shallow" and "deep" measurement, which gives
//!   flexibility in the cases where the traits can't be used.
//! - It allows for measuring blocks even when only an interior pointer can be
//!   obtained for heap allocations, e.g. `HashSet` and `HashMap`. (This relies
//!   on the heap allocator having suitable support, which mozjemalloc has.)
//! - It allows handling of types like `Rc` and `Arc` by providing traits that
//!   are different to the ones for non-graph structures.
//!
//! Suggested uses are as follows.
//! - When possible, use the `MallocSizeOf` trait. (Deriving support is
//!   provided by the `malloc_size_of_derive` crate.)
//! - If you need an additional synchronization argument, provide a function
//!   that is like the standard trait method, but with the extra argument.
//! - If you need multiple measurements for a type, provide a function named
//!   `add_size_of` that takes a mutable reference to a struct that contains
//!   the multiple measurement fields.
//! - When deep measurement (via `MallocSizeOf`) cannot be implemented for a
//!   type, shallow measurement (via `MallocShallowSizeOf`) in combination with
//!   iteration can be a useful substitute.
//! - `Rc` and `Arc` are always tricky, which is why `MallocSizeOf` is not (and
//!   should not be) implemented for them.
//! - If an `Rc` or `Arc` is known to be a "primary" reference and can always
//!   be measured, it should be measured via the `MallocUnconditionalSizeOf`
//!   trait.
//! - If an `Rc` or `Arc` should be measured only if it hasn't been seen
//!   before, it should be measured via the `MallocConditionalSizeOf` trait.
//! - Using universal function call syntax is a good idea when measuring boxed
//!   fields in structs, because it makes it clear that the Box is being
//!   measured as well as the thing it points to. E.g.
//!   `<Box<_> as MallocSizeOf>::size_of(field, ops)`.
//!
//!   Note: WebRender has a reduced fork of this crate, so that we can avoid
//!   publishing this crate on crates.io.
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
mod impls;

use alloc::boxed::Box;
use core::ffi::c_void;
use core::ops::{Deref, DerefMut};

/// Trait for measuring the "deep" heap usage of a data structure. This is the
/// most commonly-used of the traits.
pub trait MallocSizeOf {
    /// Measure the heap usage of all descendant heap-allocated structures, but
    /// not the space taken up by the value itself.
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// Trait for measuring the "shallow" heap usage of a container.
pub trait MallocShallowSizeOf {
    /// Measure the heap usage of immediate heap-allocated descendant
    /// structures, but not the space taken up by the value itself. Anything
    /// beyond the immediate descendants must be measured separately, using
    /// iteration.
    fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// Like `MallocSizeOf`, but with a different name so it cannot be used
/// accidentally with derive(MallocSizeOf). For use with types like `Rc` and
/// `Arc` when appropriate (e.g. when measuring a "primary" reference).
pub trait MallocUnconditionalSizeOf {
    /// Measure the heap usage of all heap-allocated descendant structures, but
    /// not the space taken up by the value itself.
    fn unconditional_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// `MallocUnconditionalSizeOf` combined with `MallocShallowSizeOf`.
pub trait MallocUnconditionalShallowSizeOf {
    /// `unconditional_size_of` combined with `shallow_size_of`.
    fn unconditional_shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// Like `MallocSizeOf`, but only measures if the value hasn't already been
/// measured. For use with types like `Rc` and `Arc` when appropriate (e.g.
/// when there is no "primary" reference).
pub trait MallocConditionalSizeOf {
    /// Measure the heap usage of all heap-allocated descendant structures, but
    /// not the space taken up by the value itself, and only if that heap usage
    /// hasn't already been measured.
    fn conditional_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// `MallocConditionalSizeOf` combined with `MallocShallowSizeOf`.
pub trait MallocConditionalShallowSizeOf {
    /// `conditional_size_of` combined with `shallow_size_of`.
    fn conditional_shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// A C function that takes a pointer to a heap allocation and returns its size.
type VoidPtrToSizeFn = unsafe extern "C" fn(ptr: *const c_void) -> usize;

/// A closure implementing a stateful predicate on pointers.
type VoidPtrToBoolFnMut = dyn FnMut(*const c_void) -> bool;

/// Operations used when measuring heap usage of data structures.
pub struct MallocSizeOfOps {
    /// A function that returns the size of a heap allocation.
    size_of_op: VoidPtrToSizeFn,

    /// Like `size_of_op`, but can take an interior pointer. Optional because
    /// not all allocators support this operation. If it's not provided, some
    /// memory measurements will actually be computed estimates rather than
    /// real and accurate measurements.
    enclosing_size_of_op: Option<VoidPtrToSizeFn>,

    /// Check if a pointer has been seen before, and remember it for next time.
    /// Useful when measuring `Rc`s and `Arc`s. Optional, because many places
    /// don't need it.
    have_seen_ptr_op: Option<Box<VoidPtrToBoolFnMut>>,
}

impl MallocSizeOfOps {
    pub fn new(
        size_of: VoidPtrToSizeFn,
        malloc_enclosing_size_of: Option<VoidPtrToSizeFn>,
        have_seen_ptr: Option<Box<VoidPtrToBoolFnMut>>,
    ) -> Self {
        MallocSizeOfOps {
            size_of_op: size_of,
            enclosing_size_of_op: malloc_enclosing_size_of,
            have_seen_ptr_op: have_seen_ptr,
        }
    }

    /// Check if an allocation is empty. This relies on knowledge of how Rust
    /// handles empty allocations, which may change in the future.
    fn is_empty<T: ?Sized>(ptr: *const T) -> bool {
        // The correct condition is this:
        //   `ptr as usize <= ::std::mem::align_of::<T>()`
        // But we can't call align_of() on a ?Sized T. So we approximate it
        // with the following. 256 is large enough that it should always be
        // larger than the required alignment, but small enough that it is
        // always in the first page of memory and therefore not a legitimate
        // address.
        ptr as *const usize as usize <= 256
    }

    /// Call `size_of_op` on `ptr`, first checking that the allocation isn't
    /// empty, because some types (such as `Vec`) utilize empty allocations.
    pub unsafe fn malloc_size_of<T: ?Sized>(&self, ptr: *const T) -> usize {
        if MallocSizeOfOps::is_empty(ptr) {
            0
        } else {
            (self.size_of_op)(ptr as *const c_void)
        }
    }

    /// Is an `enclosing_size_of_op` available?
    pub fn has_malloc_enclosing_size_of(&self) -> bool {
        self.enclosing_size_of_op.is_some()
    }

    /// Call `enclosing_size_of_op`, which must be available, on `ptr`, which
    /// must not be empty.
    pub unsafe fn malloc_enclosing_size_of<T>(&self, ptr: *const T) -> usize {
        assert!(!MallocSizeOfOps::is_empty(ptr));
        (self.enclosing_size_of_op.unwrap())(ptr as *const c_void)
    }

    /// Call `have_seen_ptr_op` on `ptr`.
    pub fn have_seen_ptr<T>(&mut self, ptr: *const T) -> bool {
        let have_seen_ptr_op = self
            .have_seen_ptr_op
            .as_mut()
            .expect("missing have_seen_ptr_op");
        have_seen_ptr_op(ptr as *const c_void)
    }
}

/// Measurable that defers to inner value and used to verify MallocSizeOf implementation in a
/// struct.
#[derive(Clone)]
pub struct Measurable<T: MallocSizeOf>(pub T);

impl<T: MallocSizeOf> Deref for Measurable<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: MallocSizeOf> DerefMut for Measurable<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
