use nolock::queues::spsc::unbounded;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

fn bench_nolock() {
    let num_inserts = 100_000;

    let (mut rx, mut tx) = unbounded::queue::<i32>();
    let start_gate = Arc::new(Barrier::new(3));
    let end_gate = Arc::new(Barrier::new(3));

    let t1_start = start_gate.clone();
    let t1_end = end_gate.clone();
    let t1 = thread::spawn(move || {
        t1_start.wait();
        for i in 0..num_inserts {
            tx.enqueue(i).unwrap();
        }
        drop(tx);
        t1_end.wait();
    });

    let t2_start = start_gate.clone();
    let t2_end = end_gate.clone();
    let t2 = thread::spawn(move || {
        t2_start.wait();
        let mut sum = 0;
        while let Ok(val) = rx.try_dequeue() {
            sum += val;
        }
        t2_end.wait();
    });

    start_gate.wait();
    let start = Instant::now();
    end_gate.wait();
    let dur = start.elapsed();

    println!("elapsed time (millisecs): {}", dur.as_millis());

    t1.join().unwrap();
    t2.join().unwrap();
}

fn main() {
    bench_nolock();
}
