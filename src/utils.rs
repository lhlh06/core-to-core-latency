use crate::bench::Count;
use quanta::Clock;
use std::time::Duration;

pub fn black_box<T>(dummy: T) -> T {
    unsafe { std::ptr::read_volatile(&dummy) }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline(always)]
pub fn raw_fenced(clock: &Clock) -> u64 {
    use std::sync::atomic::compiler_fence;
    compiler_fence(std::sync::atomic::Ordering::SeqCst);
    unsafe {
        core::arch::x86_64::_mm_lfence();
    }
    let t = clock.raw();
    unsafe {
        core::arch::x86_64::_mm_lfence();
    }
    compiler_fence(std::sync::atomic::Ordering::SeqCst);
    t
}

pub fn delay_cycles(num_iterations: usize) {
    static VALUE: usize = 0;
    for _ in 0..num_iterations {
        // unsafe { std::arch::asm!("nop"); } might not work on all platforms
        black_box(&VALUE);
    }
}

// Returns the duration of doing `num_iterations` of clock.raw()
pub fn clock_read_overhead_sum(clock: &Clock, num_iterations: Count) -> Duration {
    let start = clock.raw();
    for _ in 0..(num_iterations - 1) {
        black_box(clock.raw());
    }
    let end = clock.raw();
    clock.delta(start, end)
}

/// Returns the duration of `Median` overhead of 2x[raw_fenced] and 1 read tsc clock
pub fn measure_overhead_ns(clock: &Clock, num_iterations: usize) -> u64 {
    let mut overhead = Vec::with_capacity(num_iterations);

    for _ in 0..num_iterations {
        let start = raw_fenced(clock);
        let end = raw_fenced(clock);
        let ns = clock.delta_as_nanos(start, end);
        overhead.push(ns);
    }

    overhead.sort_by(|a, b| a.partial_cmp(b).unwrap());
    overhead[overhead.len() / 2]
}

// This big feature condition is on CpuId::default(), we'll use the same.
#[cfg(any(
    all(target_arch = "x86", not(target_env = "sgx"), target_feature = "sse"),
    all(target_arch = "x86_64", not(target_env = "sgx"))
))]
pub fn get_cpuid() -> Option<raw_cpuid::CpuId<raw_cpuid::native_cpuid::CpuIdReaderNative>> {
    Some(raw_cpuid::CpuId::default())
}

#[cfg(not(any(
    all(target_arch = "x86", not(target_env = "sgx"), target_feature = "sse"),
    all(target_arch = "x86_64", not(target_env = "sgx"))
)))]
pub fn get_cpuid() -> Option<raw_cpuid::CpuId> {
    None
}

pub fn assert_rdtsc_usable(clock: &quanta::Clock) {
    let cpuid = get_cpuid().expect("This benchmark is only compatible with x86");

    assert!(
        cpuid
            .get_advanced_power_mgmt_info()
            .expect("CPUID failed")
            .has_invariant_tsc(),
        "This benchmark only runs with a TscInvariant=true"
    );

    const NUM_ITERS: Count = 10_000;
    let clock_read_overhead =
        clock_read_overhead_sum(clock, NUM_ITERS).as_nanos() as f64 / NUM_ITERS as f64;
    eprintln!(
        "Reading the clock via RDTSC takes {:.2}ns",
        clock_read_overhead
    );
    assert!(
        (0.1..1000.0).contains(&clock_read_overhead),
        "The timing to read the clock is either not-consistant or too slow"
    );
}

pub fn get_cpu_brand() -> Option<String> {
    get_cpuid()
        .and_then(|c| c.get_processor_brand_string())
        .map(|c| c.as_str().to_string())
}

pub fn show_cpuid_info() {
    if let Some(brand) = get_cpu_brand() {
        eprintln!("CPU: {}", brand);
    }
}
