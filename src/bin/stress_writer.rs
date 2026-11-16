use memmap2::MmapMut;
use std::fs::OpenOptions;
use std::mem::size_of;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use t_scope::ipc::{init_shared_region, SharedRegion};

const STRESS_FILE: &str = "stress_test.dat";

fn random_delay() {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    std::time::Instant::now().hash(&mut hasher);
    let hash = hasher.finish();

    let micros = if hash % 100 < 5 {
        (hash % 1000) as u64
    } else {
        (hash % 100) as u64
    };

    if micros > 0 {
        std::thread::sleep(Duration::from_micros(micros));
    } else {
        std::thread::yield_now();
    }
}

fn write_self_checking_snapshot(region: *mut SharedRegion, generation: u64, slot: u32) {
    let train_count = ((generation % 100) + 1) as u32;
    let prediction_count = (generation % 32) as u32;

    unsafe {
        let snapshot = &mut (*region).slots[slot as usize];
        snapshot.train_count = train_count;

        for i in 0..train_count as usize {
            let train = &mut snapshot.trains[i];

            train.lat = generation as f64 + i as f64 * 0.0001;
            train.long = -(generation as f64) - i as f64 * 0.0001;

            let route_str = format!("GEN{:08}", generation);
            train.route_id = [0u8; 16];
            train.route_id[..route_str.len().min(16)]
                .copy_from_slice(&route_str.as_bytes()[..route_str.len().min(16)]);

            let vehicle_str = format!("V{:08}-{:04}", generation, i);
            train.vehicle_id = [0u8; 16];
            train.vehicle_id[..vehicle_str.len().min(16)]
                .copy_from_slice(&vehicle_str.as_bytes()[..vehicle_str.len().min(16)]);

            train.prediction_count = prediction_count;

            for j in 0..prediction_count as usize {
                train.predictions[j].arrival_time_ms = (generation * 1000 + j as u64) as i64;
                train.predictions[j].departure_time_ms = (generation * 1000 + j as u64 + 500) as i64;
                train.predictions[j].stop_number = generation;
                train.predictions[j].direction_id = generation % 2;
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let duration_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(STRESS_FILE)?;

    let file_len = file.metadata()?.len();
    let expected_len = size_of::<SharedRegion>() as u64;

    let mut mmap = match file_len {
        0 => {
            file.set_len(expected_len)?;
            let mut mmap = unsafe { MmapMut::map_mut(&file)? };
            init_shared_region(&mut mmap);
            mmap
        }
        len if len == expected_len => unsafe { MmapMut::map_mut(&file)? },
        _ => {
            file.set_len(expected_len)?;
            let mut mmap = unsafe { MmapMut::map_mut(&file)? };
            init_shared_region(&mut mmap);
            mmap
        }
    };

    let region = mmap.as_mut_ptr() as *mut SharedRegion;

    eprintln!("stress_writer: starting, duration={}s", duration_secs);

    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);
    let mut writes = 0u64;
    let mut stalls = 0u64;

    while start.elapsed() < duration {
        let curr_pub = unsafe { (*region).publication.load(Ordering::Acquire) };
        let curr_slot = (curr_pub & 0xFFFF_FFFF) as u32;
        let curr_gen = curr_pub >> 32;
        let reader_slot = unsafe { (*region).reader_holds.load(Ordering::SeqCst) };

        let candidate1 = (curr_slot + 1) % 3;
        let candidate2 = (curr_slot + 2) % 3;

        let next_slot = if candidate1 != reader_slot {
            candidate1
        } else if candidate2 != reader_slot {
            candidate2
        } else {
            stalls += 1;
            std::thread::yield_now();
            continue;
        };

        let new_gen = curr_gen + 1;

        random_delay();
        write_self_checking_snapshot(region, new_gen, next_slot);
        random_delay();

        let new_pub = (new_gen << 32) | (next_slot as u64);
        unsafe { (*region).publication.store(new_pub, Ordering::SeqCst) };

        writes += 1;

        if writes % 10000 == 0 {
            eprintln!(
                "stress_writer: {} writes, {} stalls, {:.1}s elapsed",
                writes,
                stalls,
                start.elapsed().as_secs_f64()
            );
        }
    }

    eprintln!(
        "stress_writer: DONE. {} writes, {} stalls in {:.2}s",
        writes,
        stalls,
        start.elapsed().as_secs_f64()
    );

    Ok(())
}
