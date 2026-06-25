// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Single-worker copies for ordinary ArrayBuffer storage.

use core::ptr::NonNull;

pub(crate) const UNALIGNED_ACCESS_IS_OK: bool = true;

#[inline]
pub(crate) unsafe fn unordered_memcpy_down_unsynchronized(
    src: NonNull<()>,
    dst: NonNull<()>,
    count: usize,
) {
    // SAFETY: forwarded from the caller; regions do not overlap in this direction.
    unsafe {
        core::ptr::copy_nonoverlapping(src.as_ptr().cast::<u8>(), dst.as_ptr().cast::<u8>(), count)
    };
}

#[inline]
pub(crate) unsafe fn unordered_memcpy_up_unsynchronized(
    src: NonNull<()>,
    dst: NonNull<()>,
    count: usize,
) {
    // SAFETY: forwarded from the caller; `copy` preserves memmove semantics.
    unsafe { core::ptr::copy(src.as_ptr().cast::<u8>(), dst.as_ptr().cast::<u8>(), count) };
}
