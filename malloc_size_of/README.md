# MallocSizeOf

[![Build Status](https://github.com/servo/malloc_size_of/actions/workflows/main.yml/badge.svg)](https://github.com/servo/malloc_size_of/actions)
[![Crates.io](https://img.shields.io/crates/v/malloc_size_of.svg)](https://crates.io/crates/malloc_size_of)
[![Docs](https://docs.rs/malloc_size_of/badge.svg)](https://docs.rs/malloc_size_of)
![Crates.io License](https://img.shields.io/crates/l/malloc_size_of)
[![dependency status](https://deps.rs/repo/github/servo/malloc_size_of/status.svg)](https://deps.rs/repo/github/servo/malloc_size_of)

A an allocator-agnostic crate for measuring the runtime size of a value
including the size of any heap allocations that are owned by that value.

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
