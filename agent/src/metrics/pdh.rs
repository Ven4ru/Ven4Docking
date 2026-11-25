//! PDH query opened once; wildcard counters are expanded by PDH on every collection.

#[cfg(windows)]
mod imp {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Performance::*;

    pub struct Pdh {
        q: PDH_HQUERY,
        counters: Vec<Option<PDH_HCOUNTER>>,
    }

    // PDH handles may be used from any thread (one owner at a time).
    unsafe impl Send for Pdh {}

    impl Pdh {
        /// Opens a query with the given English counter paths. Missing counters stay `None`.
        pub fn open(paths: &[&str]) -> Option<Pdh> {
            unsafe {
                let mut q = PDH_HQUERY::default();
                if PdhOpenQueryW(PCWSTR::null(), 0, &mut q) != 0 {
                    return None;
                }
                let counters = paths
                    .iter()
                    .map(|p| {
                        let mut c = PDH_HCOUNTER::default();
                        (PdhAddEnglishCounterW(q, &HSTRING::from(*p), 0, &mut c) == 0).then_some(c)
                    })
                    .collect();
                let _ = PdhCollectQueryData(q);
                Some(Pdh { q, counters })
            }
        }

        pub fn collect(&self) {
            unsafe {
                let _ = PdhCollectQueryData(self.q);
            }
        }

        /// (instance name, value) pairs of counter `idx` from the last collection.
        pub fn array(&self, idx: usize) -> Vec<(String, f64)> {
            let Some(Some(c)) = self.counters.get(idx) else { return Vec::new() };
            let fmt = PDH_FMT(0x0000_0200 | 0x0000_8000); // DOUBLE | NOCAP100
            unsafe {
                let (mut size, mut count) = (0u32, 0u32);
                let r = PdhGetFormattedCounterArrayW(*c, fmt, &mut size, &mut count, None);
                if r != PDH_MORE_DATA || size == 0 {
                    return Vec::new();
                }
                let mut buf = vec![0u64; (size as usize).div_ceil(8) + 1];
                let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
                if PdhGetFormattedCounterArrayW(*c, fmt, &mut size, &mut count, Some(items)) != 0 {
                    return Vec::new();
                }
                let mut out = Vec::with_capacity(count as usize);
                for i in 0..count as usize {
                    let it = &*items.add(i);
                    if it.FmtValue.CStatus <= 1 {
                        out.push((it.szName.to_string().unwrap_or_default(), it.FmtValue.Anonymous.doubleValue));
                    }
                }
                out
            }
        }
    }

    impl Drop for Pdh {
        fn drop(&mut self) {
            unsafe {
                let _ = PdhCloseQuery(self.q);
            }
        }
    }
}

#[cfg(windows)]
pub use imp::Pdh;
