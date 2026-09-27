use reqwest::Client;
use std::fs::OpenOptions;
use std::mem::size_of;
use std::sync::atomic::Ordering;
use std::time::Duration;
use memmap2::MmapMut;
use tokio::time::sleep;
use t_scope::mbta::{combine_vehicles, fetch_vehicles, fetch_predictions};
use t_scope::ipc::{SharedRegion, init_shared_region, write_snapshot};


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("shared_data.dat")?;

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
        _ => return Err("shared_data.dat has unexpected size".into()),
    };

    let region = mmap.as_mut_ptr() as *mut SharedRegion;

    loop {
        let trains = fetch_vehicles(&client).await?;
        let predictions = fetch_predictions(&client).await?;
        let snapshot = combine_vehicles(predictions, trains);

        let curr_pub = unsafe { (*region).publication.load(Ordering::Acquire) };
        let curr_slot = (curr_pub & 0xFFFF_FFFF) as u32;
        let reader_slot = unsafe { (*region).reader_holds.load(Ordering::Acquire) };

        let next_slot = {
            let candidate = (curr_slot + 1) % 3;
            if candidate != reader_slot {
                candidate
            } else {
                let candidate2 = (curr_slot + 2) % 3;
                if candidate2 != reader_slot {
                    candidate2
                } else {
                    candidate
                }
            }
        };

        let slot_ptr = unsafe { (*region).slots.as_mut_ptr().add(next_slot as usize) };
        unsafe { write_snapshot(&snapshot, slot_ptr) };

        let new_generation = (curr_pub >> 32) + 1;
        let new_pub = (new_generation << 32) | (next_slot as u64);
        unsafe { (*region).publication.store(new_pub, Ordering::Release) };

        sleep(Duration::from_secs(10)).await;
    }
}
