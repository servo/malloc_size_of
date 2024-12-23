use crate::{MallocShallowSizeOf, MallocSizeOf, MallocSizeOfOps};
use core::cell::{Cell, RefCell};

use core::hash::Hash;
use core::marker::PhantomData;
use core::mem::size_of;
use core::num::{NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize};
use core::num::{NonZeroU128, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize};
use core::ops::{Range, RangeFrom, RangeInclusive, RangeTo};
use core::sync::atomic::AtomicBool;
use core::sync::atomic::{AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize};
use core::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, AtomicU8, AtomicUsize};

use alloc::borrow::{Cow, ToOwned};
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::{
    collections::{HashMap, HashSet},
    hash::BuildHasher,
    sync::Mutex,
};

// Our one exception to being completely dependency-free. The void crate is tiny, unlikely to ever
// do another release, and likely to go away if/when the never type stabilises.
#[cfg(feature = "void")]
impl MallocSizeOf for void::Void {
    #[inline]
    fn size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
        void::unreachable(*self)
    }
}

/// For use on types where size_of() returns 0.
#[macro_export]
macro_rules! malloc_size_of_is_0(
    ($($ty:ty),+) => (
        $(
            impl $crate::MallocSizeOf for $ty {
                #[inline(always)]
                fn size_of(&self, _: &mut $crate::MallocSizeOfOps) -> usize {
                    0
                }
            }
        )+
    );
    ($($ty:ident<$($gen:ident),+>),+) => (
        $(
        impl<$($gen: $crate::MallocSizeOf),+> $crate::MallocSizeOf for $ty<$($gen),+> {
            #[inline(always)]
            fn size_of(&self, _: &mut $crate::MallocSizeOfOps) -> usize {
                0
            }
        }
        )+
    );
);

malloc_size_of_is_0!((), bool, char, str);
malloc_size_of_is_0!(u8, u16, u32, u64, u128, usize);
malloc_size_of_is_0!(i8, i16, i32, i64, i128, isize);
malloc_size_of_is_0!(f32, f64);

malloc_size_of_is_0!(AtomicBool);
malloc_size_of_is_0!(AtomicU8, AtomicU16, AtomicU32, AtomicU64, AtomicUsize);
malloc_size_of_is_0!(AtomicI8, AtomicI16, AtomicI32, AtomicI64, AtomicIsize);
malloc_size_of_is_0!(
    NonZeroU8,
    NonZeroU16,
    NonZeroU32,
    NonZeroU64,
    NonZeroUsize,
    NonZeroU128
);
malloc_size_of_is_0!(
    NonZeroI8,
    NonZeroI16,
    NonZeroI32,
    NonZeroI64,
    NonZeroIsize,
    NonZeroI128
);

impl<T: ?Sized> MallocSizeOf for &'_ T {
    fn size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
        // Zero makes sense for a non-owning reference.
        0
    }
}

impl<T: ?Sized> MallocSizeOf for &'_ mut T {
    fn size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
        // Zero makes sense for a non-owning reference.
        0
    }
}

// PhantomData is always 0.
impl<T> MallocSizeOf for PhantomData<T> {
    fn size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
        0
    }
}

impl<T: MallocSizeOf, const N: usize> MallocSizeOf for [T; N] {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.iter().fold(0, |acc, item| acc + item.size_of(ops))
    }
}

impl<T1, T2> MallocSizeOf for (T1, T2)
where
    T1: MallocSizeOf,
    T2: MallocSizeOf,
{
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.0.size_of(ops) + self.1.size_of(ops)
    }
}

impl<T1, T2, T3> MallocSizeOf for (T1, T2, T3)
where
    T1: MallocSizeOf,
    T2: MallocSizeOf,
    T3: MallocSizeOf,
{
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.0.size_of(ops) + self.1.size_of(ops) + self.2.size_of(ops)
    }
}

impl<T1, T2, T3, T4> MallocSizeOf for (T1, T2, T3, T4)
where
    T1: MallocSizeOf,
    T2: MallocSizeOf,
    T3: MallocSizeOf,
    T4: MallocSizeOf,
{
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.0.size_of(ops) + self.1.size_of(ops) + self.2.size_of(ops) + self.3.size_of(ops)
    }
}

impl<T: MallocSizeOf> MallocSizeOf for [T] {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        let mut n = 0;
        for elem in self.iter() {
            n += elem.size_of(ops);
        }
        n
    }
}

impl<T: MallocSizeOf> MallocSizeOf for Range<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.start.size_of(ops) + self.end.size_of(ops)
    }
}
impl<T: MallocSizeOf> MallocSizeOf for RangeInclusive<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.start().size_of(ops) + self.end().size_of(ops)
    }
}
impl<T: MallocSizeOf> MallocSizeOf for RangeTo<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.end.size_of(ops)
    }
}
impl<T: MallocSizeOf> MallocSizeOf for RangeFrom<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.start.size_of(ops)
    }
}

impl<T: MallocSizeOf> MallocSizeOf for Option<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        match self {
            Some(val) => val.size_of(ops),
            None => 0,
        }
    }
}

impl<T: MallocSizeOf, E: MallocSizeOf> MallocSizeOf for Result<T, E> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        match *self {
            Ok(ref x) => x.size_of(ops),
            Err(ref e) => e.size_of(ops),
        }
    }
}

impl<T: MallocSizeOf + Copy> MallocSizeOf for Cell<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.get().size_of(ops)
    }
}

impl<T: MallocSizeOf> MallocSizeOf for RefCell<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.borrow().size_of(ops)
    }
}

impl<B: ?Sized + ToOwned> MallocSizeOf for Cow<'_, B>
where
    B::Owned: MallocSizeOf,
{
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        match *self {
            Cow::Borrowed(_) => 0,
            Cow::Owned(ref b) => b.size_of(ops),
        }
    }
}

impl MallocSizeOf for String {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        unsafe { ops.malloc_size_of(self.as_ptr()) }
    }
}

impl<T: ?Sized> MallocShallowSizeOf for Box<T> {
    fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        unsafe { ops.malloc_size_of(&**self) }
    }
}

impl<T: MallocSizeOf + ?Sized> MallocSizeOf for Box<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        self.shallow_size_of(ops) + (**self).size_of(ops)
    }
}

impl<T> MallocShallowSizeOf for Vec<T> {
    fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        unsafe { ops.malloc_size_of(self.as_ptr()) }
    }
}

impl<T: MallocSizeOf> MallocSizeOf for Vec<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        let mut n = self.shallow_size_of(ops);
        for elem in self.iter() {
            n += elem.size_of(ops);
        }
        n
    }
}

impl<T> MallocShallowSizeOf for VecDeque<T> {
    fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        if ops.has_malloc_enclosing_size_of() {
            if let Some(front) = self.front() {
                // The front element is an interior pointer.
                unsafe { ops.malloc_enclosing_size_of(front) }
            } else {
                // This assumes that no memory is allocated when the VecDeque is empty.
                0
            }
        } else {
            // An estimate.
            self.capacity() * size_of::<T>()
        }
    }
}

impl<T: MallocSizeOf> MallocSizeOf for VecDeque<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        let mut n = self.shallow_size_of(ops);
        for elem in self.iter() {
            n += elem.size_of(ops);
        }
        n
    }
}

impl<K, V> MallocShallowSizeOf for BTreeMap<K, V>
where
    K: Eq + Hash,
{
    fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        if ops.has_malloc_enclosing_size_of() {
            self.values()
                .next()
                .map_or(0, |v| unsafe { ops.malloc_enclosing_size_of(v) })
        } else {
            self.len() * (size_of::<V>() + size_of::<K>() + size_of::<usize>())
        }
    }
}

impl<K, V> MallocSizeOf for BTreeMap<K, V>
where
    K: Eq + Hash + MallocSizeOf,
    V: MallocSizeOf,
{
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        let mut n = self.shallow_size_of(ops);
        for (k, v) in self.iter() {
            n += k.size_of(ops);
            n += v.size_of(ops);
        }
        n
    }
}

#[cfg(feature = "std")]
impl<T, S> MallocShallowSizeOf for HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        if ops.has_malloc_enclosing_size_of() {
            // The first value from the iterator gives us an interior pointer.
            // `ops.malloc_enclosing_size_of()` then gives us the storage size.
            // This assumes that the `HashSet`'s contents (values and hashes)
            // are all stored in a single contiguous heap allocation.
            self.iter()
                .next()
                .map_or(0, |t| unsafe { ops.malloc_enclosing_size_of(t) })
        } else {
            // An estimate.
            self.capacity() * (size_of::<T>() + size_of::<usize>())
        }
    }
}

#[cfg(feature = "std")]
impl<T, S> MallocSizeOf for HashSet<T, S>
where
    T: Eq + Hash + MallocSizeOf,
    S: BuildHasher,
{
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        let mut n = self.shallow_size_of(ops);
        for t in self.iter() {
            n += t.size_of(ops);
        }
        n
    }
}

#[cfg(feature = "std")]
impl<K, V, S> MallocShallowSizeOf for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        // See the implementation for HashSet for details.
        if ops.has_malloc_enclosing_size_of() {
            self.values()
                .next()
                .map_or(0, |v| unsafe { ops.malloc_enclosing_size_of(v) })
        } else {
            self.capacity() * (size_of::<V>() + size_of::<K>() + size_of::<usize>())
        }
    }
}

#[cfg(feature = "std")]
impl<K, V, S> MallocSizeOf for HashMap<K, V, S>
where
    K: Eq + Hash + MallocSizeOf,
    V: MallocSizeOf,
    S: BuildHasher,
{
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        let mut n = self.shallow_size_of(ops);
        for (k, v) in self.iter() {
            n += k.size_of(ops);
            n += v.size_of(ops);
        }
        n
    }
}

/// If a mutex is stored directly as a member of a data type that is being measured,
/// it is the unique owner of its contents and deserves to be measured.
///
/// If a mutex is stored inside of an Arc value as a member of a data type that is being measured,
/// the Arc will not be automatically measured so there is no risk of overcounting the mutex's
/// contents.
#[cfg(feature = "std")]
impl<T: MallocSizeOf> MallocSizeOf for Mutex<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        (*self.lock().unwrap()).size_of(ops)
    }
}

// XXX: we don't want MallocSizeOf to be defined for Rc and Arc. If negative
// trait bounds are ever allowed, this code should be uncommented.
// (We do have a compile-fail test for this: rc_arc_must_not_derive_malloc_size_of.rs)
//impl<T> !MallocSizeOf for Arc<T> { }
//impl<T> !MallocShallowSizeOf for Arc<T> { }
