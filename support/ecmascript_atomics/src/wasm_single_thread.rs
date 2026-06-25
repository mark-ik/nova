// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Single-worker WebAssembly backend.
//!
//! This module is only sound while the embedding Nova build does not expose
//! SharedArrayBuffer or Atomics. With one owning worker there are no concurrent
//! accesses, so ordinary unaligned reads/writes realize ArrayBuffer semantics.

use core::ptr::NonNull;

#[inline]
fn load<T: Copy>(ptr: NonNull<()>) -> T {
    // SAFETY: the public RacyPtr API validates the address and value width.
    unsafe { ptr.cast::<T>().as_ptr().read_unaligned() }
}

#[inline]
fn store<T>(ptr: NonNull<()>, value: T) {
    // SAFETY: the public RacyPtr API validates the address and value width.
    unsafe { ptr.cast::<T>().as_ptr().write_unaligned(value) }
}

macro_rules! integer_ops {
    (
        $ty:ty,
        $load_seq:ident, $load_unordered:ident,
        $store_seq:ident, $store_unordered:ident,
        $exchange:ident, $compare_exchange:ident,
        $add:ident, $and:ident, $or:ident, $xor:ident
    ) => {
        pub(crate) fn $load_seq(ptr: NonNull<()>) -> $ty {
            load(ptr)
        }
        pub(crate) fn $load_unordered(ptr: NonNull<()>) -> $ty {
            load(ptr)
        }
        pub(crate) fn $store_seq(ptr: NonNull<()>, value: $ty) {
            store(ptr, value)
        }
        pub(crate) fn $store_unordered(ptr: NonNull<()>, value: $ty) {
            store(ptr, value)
        }
        pub(crate) fn $exchange(ptr: NonNull<()>, value: $ty) -> $ty {
            let old = load(ptr);
            store(ptr, value);
            old
        }
        pub(crate) fn $compare_exchange(ptr: NonNull<()>, old: $ty, new: $ty) -> $ty {
            let current = load(ptr);
            if current == old {
                store(ptr, new);
            }
            current
        }
        pub(crate) fn $add(ptr: NonNull<()>, value: $ty) -> $ty {
            let old: $ty = load(ptr);
            store(ptr, old.wrapping_add(value));
            old
        }
        pub(crate) fn $and(ptr: NonNull<()>, value: $ty) -> $ty {
            let old: $ty = load(ptr);
            store(ptr, old & value);
            old
        }
        pub(crate) fn $or(ptr: NonNull<()>, value: $ty) -> $ty {
            let old: $ty = load(ptr);
            store(ptr, old | value);
            old
        }
        pub(crate) fn $xor(ptr: NonNull<()>, value: $ty) -> $ty {
            let old: $ty = load(ptr);
            store(ptr, old ^ value);
            old
        }
    };
}

integer_ops!(
    u8,
    atomic_load_8_seq_cst,
    atomic_load_8_unsynchronized,
    atomic_store_8_seq_cst,
    atomic_store_8_unsynchronized,
    atomic_exchange_8_seq_cst,
    atomic_cmp_xchg_8_seq_cst,
    atomic_add_8_seq_cst,
    atomic_and_8_seq_cst,
    atomic_or_8_seq_cst,
    atomic_xor_8_seq_cst
);
integer_ops!(
    u16,
    atomic_load_16_seq_cst,
    atomic_load_16_unsynchronized,
    atomic_store_16_seq_cst,
    atomic_store_16_unsynchronized,
    atomic_exchange_16_seq_cst,
    atomic_cmp_xchg_16_seq_cst,
    atomic_add_16_seq_cst,
    atomic_and_16_seq_cst,
    atomic_or_16_seq_cst,
    atomic_xor_16_seq_cst
);
integer_ops!(
    u32,
    atomic_load_32_seq_cst,
    atomic_load_32_unsynchronized,
    atomic_store_32_seq_cst,
    atomic_store_32_unsynchronized,
    atomic_exchange_32_seq_cst,
    atomic_cmp_xchg_32_seq_cst,
    atomic_add_32_seq_cst,
    atomic_and_32_seq_cst,
    atomic_or_32_seq_cst,
    atomic_xor_32_seq_cst
);
integer_ops!(
    u64,
    atomic_load_64_seq_cst,
    atomic_load_64_unsynchronized,
    atomic_store_64_seq_cst,
    atomic_store_64_unsynchronized,
    atomic_exchange_64_seq_cst,
    atomic_cmp_xchg_64_seq_cst,
    atomic_add_64_seq_cst,
    atomic_and_64_seq_cst,
    atomic_or_64_seq_cst,
    atomic_xor_64_seq_cst
);
