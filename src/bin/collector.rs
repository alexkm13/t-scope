use reqwest::Client;
use std::fs::OpenOptions;
use std::mem::size_of;
use memmap2::{MmapMut, MmapOptions};
use t_scope::mbta::{SharedSnapshot, fetch_vehicles, fetch_predictions}; 

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let trains = fetch_vehicles(&client).await?;
    let predictions = fetch_predictions(&client).await?;
    
    let file = OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .open("shared_data.dat")?;

    file.set_len(size_of::<SharedSnapshot>() as u64)?;

    let mut mmap = unsafe {
        MmapMut::map_mut(&file)?
    };
 
    Ok(())
}
