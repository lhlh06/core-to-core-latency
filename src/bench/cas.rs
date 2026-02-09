use super::Count;
use core_affinity::CoreId;
use quanta::Clock;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};

const PING: bool = false;
const PONG: bool = true;

pub struct Bench {
    barrier: Barrier,
    start: AtomicBool,
    flag: AtomicBool,
}

impl Bench {
    pub fn new() -> Self {
        Self {
            barrier: Barrier::new(2),
            start: AtomicBool::new(false),
            flag: AtomicBool::new(PING),
        }
    }
}

impl super::Bench for Bench {
    // The two threads modify the same cacheline.
    // This is useful to benchmark spinlock performance.
    fn run(
        &self,
        (ping_core, pong_core): (CoreId, CoreId),
        clock: &Clock,
        num_round_trips: Count,
        num_samples: Count,
    ) -> Vec<f64> {
        let state = self;

        crossbeam_utils::thread::scope(|s| {
            let pong = s.spawn(move |_| {
                core_affinity::set_for_current(pong_core);

                state.barrier.wait();
                while !state.start.load(Ordering::Acquire) {
                    core::hint::spin_loop();
                }

                for _ in 0..(num_round_trips * num_samples) {
                    while state
                        .flag
                        .compare_exchange(PING, PONG, Ordering::Relaxed, Ordering::Relaxed)
                        .is_err()
                    {}
                }
            });

            let ping = s.spawn(move |_| {
                core_affinity::set_for_current(ping_core);

                let mut results = Vec::with_capacity(num_samples as usize);

                state.barrier.wait();
                let overhead =
                    crate::utils::measure_overhead_ns(clock, num_round_trips.try_into().unwrap());
                state.start.store(true, Ordering::Release);

                for _ in 0..num_samples {
                    // let start = clock.raw();
                    let start = crate::utils::raw_fenced(clock);
                    for _ in 0..num_round_trips {
                        while state
                            .flag
                            .compare_exchange(PONG, PING, Ordering::Relaxed, Ordering::Relaxed)
                            .is_err()
                        {}
                    }
                    // let end = clock.raw();
                    let end = crate::utils::raw_fenced(clock);
                    let duration = clock.delta(start, end).as_nanos() as u64;
                    let duration = duration - overhead;
                    results.push(duration as f64 / num_round_trips as f64 / 2.0);
                }

                results
            });

            pong.join().unwrap();
            ping.join().unwrap()
        })
        .unwrap()
    }
}
