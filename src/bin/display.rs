use memmap2::MmapOptions;
use std::fs::OpenOptions;
use t_scope::mbta::SharedSnapshot;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = OpenOptions::new()
    .read(true)
    .open("shared_data.dat")?;

    let mmap = unsafe {
        MmapOptions::new().map(&file)?
    };
    
    let ptr = mmap.as_ptr() as *const SharedSnapshot;
    let shared = unsafe { &*ptr };
    
    for i in 0..shared.train_count as usize {
        let train = &shared.trains[i];

        println!(
            "train {}: lat={}, long={}, predictions={}",
            i,
            train.lat,
            train.long,
            train.prediction_count
        );
    }

    Ok(())
}
