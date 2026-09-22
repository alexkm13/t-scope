mod mbta;

#[tokio::main]
async fn main() {
    let client = reqwest::Client::new();
    match mbta::fetch_vehicles(&client).await {
        Ok(trains) => {
            for train in trains {
                println!("{:?}", train);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
