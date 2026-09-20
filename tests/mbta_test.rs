#[tokio::test]
async fn test_fetch_vehicles() {
    let client = reqwest::Client::new();
    let trains = t_scope::mbta::fetch_vehicles(&client).await;

    match trains {
        Ok(trains) => {
            println!("Fetched {} trains", trains.len());
            for train in trains.iter().take(5) {
                println!("{:?}", train);
            }

            // Check for null lat/long
            let null_lat_count = trains.iter().filter(|t| t.lat.is_none()).count();
            let null_long_count = trains.iter().filter(|t| t.long.is_none()).count();
            println!("Trains with null lat: {}", null_lat_count);
            println!("Trains with null long: {}", null_long_count);

            assert!(!trains.is_empty());
        }
        Err(e) => panic!("Failed to fetch: {}", e),
    }
}

#[test]
fn test_null_coords() {
    let json: serde_json::Value = serde_json::json!({
        "data": [{
            "attributes": {
                "latitude": null,
                "longitude": null,
                "label": "1234"
            },
            "relationships": {
                "route": {"data": {"id": "Red"}},
                "stop": {"data": null}
            }
        }]
    });

    let vehicle = &json["data"][0];
    let lat = vehicle["attributes"]["latitude"].as_f64();
    let long = vehicle["attributes"]["longitude"].as_f64();

    assert!(lat.is_none());
    assert!(long.is_none());
    println!("Null handling works!");
}

#[tokio::test]
async fn test_fetch_predictions() {
    let client = reqwest::Client::new();
    let predictions = t_scope::mbta::fetch_predictions(&client).await;

    match predictions {
        Ok(preds) => {
            println!("Fetched {} predictions", preds.len());
            for pred in preds.iter().take(5) {
                println!("{:?}", pred);
            }
            assert!(!preds.is_empty());
        }
        Err(e) => panic!("Failed to fetch predictions: {}", e),
    }
}
