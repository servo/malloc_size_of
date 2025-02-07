# MallocSizeOf

A an allocator-agnostic crate for measuring the runtime size of a value
including the size of any heap allocations that are owned by that value.

- The core abstraction is provided by the [`MallocSizeOf`] trait which should be implemented for all
  types whose size you wish to measure.
- A derive macro for implementing this trait on structs is provided by the [malloc_size_of_derive](https://docs.rs/malloc_size_of_derive) crate
- Additionally there are [`MallocUnconditionalSizeOf`], [`MallocConditionalSizeOf`] traits for measuring
  types where ownership is shared (such as `Arc` and `Rc`).
- Each of these traits also has a "shallow" variant ([`MallocShallowSizeOf`], [`MallocUnconditionalShallowSizeOf`], and  [`MallocConditionalShallowSizeOf`]) which only measure the heap size of the value passed
  and not any nested allocations.

All of these traits rely on being provided with an instance of [`MallocSizeOfOps`] which allows size computations
to call into the allocator to ask it for the underlyinhg size of the allocations backing data structures.

This crate is used by both Servo and Firefox for memory usage calculation.

## Features

- It isn't bound to a particular heap allocator.
- It provides traits for both "shallow" and "deep" measurement, which gives
  flexibility in the cases where the traits can't be used.
- It allows for measuring blocks even when only an interior pointer can be
  obtained for heap allocations, e.g. `HashSet` and `HashMap`. (This relies
  on the heap allocator having suitable support, which `jemalloc` has.)
- It allows handling of types like `Rc` and `Arc` by providing traits that
  are different to the ones for non-graph structures.

## Suggested usage

- When possible, use the `MallocSizeOf` trait. (Deriving support is
  provided by the `malloc_size_of_derive` crate.)
- If you need an additional synchronization argument, provide a function
  that is like the standard trait method, but with the extra argument.
- If you need multiple measurements for a type, provide a function named
  `add_size_of` that takes a mutable reference to a struct that contains
  the multiple measurement fields.
- When deep measurement (via `MallocSizeOf`) cannot be implemented for a
  type, shallow measurement (via `MallocShallowSizeOf`) in combination with
  iteration can be a useful substitute.
- `Rc` and `Arc` are always tricky, which is why `MallocSizeOf` is not (and
  should not be) implemented for them.
- If an `Rc` or `Arc` is known to be a "primary" reference and can always
  be measured, it should be measured via the `MallocUnconditionalSizeOf`
  trait.
- If an `Rc` or `Arc` should be measured only if it hasn't been seen
  before, it should be measured via the `MallocConditionalSizeOf` trait.
- Using universal function call syntax is a good idea when measuring boxed
  fields in structs, because it makes it clear that the Box is being
  measured as well as the thing it points to. E.g.
  `<Box<_> as MallocSizeOf>::size_of(field, ops)`.