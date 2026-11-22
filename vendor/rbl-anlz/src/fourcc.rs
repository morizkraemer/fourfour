// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Chris Le. From rbxport v1.10.0 (https://github.com/chrisle/rbxport), crates/rbl-core/src/lib.rs (FourCc).
// Vendored into fourfour, which distributes it under GPL-3.0-or-later. Changes: vendor/rbl-anlz/UPSTREAM.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FourCc(pub [u8; 4]);

impl FourCc {
    #[inline]
    pub const fn new(tag: &[u8; 4]) -> Self {
        Self(*tag)
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).unwrap_or("????")
    }
}

impl core::fmt::Display for FourCc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
