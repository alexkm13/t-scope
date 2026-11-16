use memmap2::MmapMut;
use std::fs::OpenOptions;
use std::mem::size_of;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use t_scope::ipc::SharedRegion;

const STRESS_FILE: &str = "stress_test.dat";

fn random_delay() {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    std::time::Instant::now().hash(&mut hasher);
    let hash = hasher.finish();

    let micros = match hash % 100 {
        0..=4 => 500 + (hash % 1500),
        5..=19 => 100 + (hash % 400),
        _ => hash % 100,
    };

    if micros > 0 {
        std::thread::sleep(Duration::from_micros(micros));
    } else {
        std::thread::yield_now();
    }
}

fn verify_snapshot_consistency(region: *const SharedRegion, slot: u32) -> Result<u64, String> {
    unsafe {
        let snapshot = &(*region).slots[slot as usize];

        if snapshot.train_count == 0 {
            return Err("train_count is 0".to_string());
        }

        let first_lat = snapshot.trains[0].lat;
        if first_lat.is_nan() || first_lat < 0.0 {
            return Err(format!("invalid first lat: {}", first_lat));
        }

        let expected_gen = first_lat.trunc() as u64;
        let expected_train_count = ((expected_gen % 100) + 1) as u32;
        let expected_prediction_count = (expected_gen % 32) as u32;

        if snapshot.train_count != expected_train_count {
            return Err(format!(
                "TORN: train_count={} but gen {} expects {}",
                snapshot.train_count, expected_gen, expected_train_count
            ));
        }

        for i in 0..snapshot.train_count as usize {
            let train = &snapshot.trains[i];

            let expected_lat = expected_gen as f64 + i as f64 * 0.0001;
            let expected_long = -(expected_gen as f64) - i as f64 * 0.0001;

            if (train.lat - expected_lat).abs() > 0.00001 {
                return Err(format!(
                    "TORN: train[{}].lat={} but expected {} for gen {}",
                    i, train.lat, expected_lat, expected_gen
                ));
            }

            if (train.long - expected_long).abs() > 0.00001 {
                return Err(format!(
                    "TORN: train[{}].long={} but expected {} for gen {}",
                    i, train.long, expected_long, expected_gen
                ));
            }

            let expected_route = format!("GEN{:08}", expected_gen);
            let route_bytes = &train.route_id[..expected_route.len()];
            if route_bytes != expected_route.as_bytes() {
                return Err(format!(
                    "TORN: train[{}].route_id mismatch for gen {}",
                    i, expected_gen
                ));
            }

            if train.prediction_count != expected_prediction_count {
                return Err(format!(
                    "TORN: train[{}].prediction_count={} but gen {} expects {}",
                    i, train.prediction_count, expected_prediction_count, expected_gen
                ));
            }

            for j in 0..train.prediction_count as usize {
                let pred = &train.predictions[j];
                let expected_arrival = (expected_gen * 1000 + j as u64) as i64;

                if pred.arrival_time_ms != expected_arrival {
                    return Err(format!(
                        "TORN: train[{}].predictions[{}].arrival_time_ms={} but expected {} for gen {}",
                        i, j, pred.arrival_time_ms, expected_arrival, expected_gen
                    ));
                }

                if pred.stop_number != expected_gen {
                    return Err(format!(
                        "TORN: train[{}].predictions[{}].stop_number={} but expected gen {}",
                        i, j, pred.stop_number, expected_gen
                    ));
                }
            }
        }

        Ok(expected_gen)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let duration_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let mut attempts = 0;
    while !std::path::Path::new(STRESS_FILE).exists() {
        if attempts > 50 {
            return Err(format!("{} not found - start stress_writer first", STRESS_FILE).into());
        }
        std::thread::sleep(Duration::from_millis(100));
        attempts += 1;
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(STRESS_FILE)?;

    if file.metadata()?.len() != size_of::<SharedRegion>() as u64 {
        return Err("stress_test.dat has unexpected size".into());
    }

    let mut mmap = unsafe { MmapMut::map_mut(&file)? };
    let region = mmap.as_mut_ptr() as *mut SharedRegion;

    eprintln!("stress_reader: starting, duration={}s", duration_secs);

    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);

    let mut reads = 0u64;
    let mut retries = 0u64;
    let mut torn_reads = 0u64;
    let mut regressions = 0u64;
    let mut last_gen = 0u64;

    while start.elapsed() < duration {
        let curr_pub = unsafe { (*region).publication.load(Ordering::Acquire) };
        let slot = (curr_pub & 0xFFFF_FFFF) as u32;

        if slot == 3 {
            std::thread::yield_now();
            continue;
        }

        unsafe { (*region).reader_holds.store(slot, Ordering::Release) };

        random_delay();

        let recheck_pub = unsafe { (*region).publication.load(Ordering::Acquire) };
        if recheck_pub != curr_pub {
            retries += 1;
            unsafe { (*region).reader_holds.store(3, Ordering::Release) };
            continue;
        }

        random_delay();

        match verify_snapshot_consistency(region, slot) {
            Ok(generation) => {
                if generation < last_gen {
                    regressions += 1;
                    eprintln!(
                        "REGRESSION: saw gen {} after gen {} (slot {})",
                        generation, last_gen, slot
                    );
                }
                last_gen = generation;
                reads += 1;
            }
            Err(e) => {
                torn_reads += 1;
                eprintln!("TORN READ: {} (slot {}, pub {:016x})", e, slot, curr_pub);
            }
        }

        unsafe { (*region).reader_holds.store(3, Ordering::Release) };

        random_delay();

        if reads % 10000 == 0 && reads > 0 {
            eprintln!(
                "stress_reader: {} reads, {} retries, {} torn, {:.1}s elapsed",
                reads,
                retries,
                torn_reads,
                start.elapsed().as_secs_f64()
            );
        }
    }

    eprintln!();
    eprintln!("========================================");
    eprintln!("stress_reader: FINAL RESULTS");
    eprintln!("========================================");
    eprintln!("reads:        {}", reads);
    eprintln!("retries:      {}", retries);
    eprintln!("torn_reads:   {}", torn_reads);
    eprintln!("regressions:  {}", regressions);
    eprintln!("duration:     {:.2}s", start.elapsed().as_secs_f64());
    eprintln!("========================================");

    if torn_reads > 0 || regressions > 0 {
        eprintln!("FAIL: Correctness violations detected!");
        std::process::exit(1);
    } else {
        eprintln!("SUCCESS: 0 torn reads across {} cross-process publications", reads);
    }

    Ok(())
}
