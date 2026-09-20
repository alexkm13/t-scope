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

#[test]
fn test_combine_vehicles() {
    use t_scope::mbta::{Train, PredictedTimes, combine_vehicles};
    use chrono::{DateTime, FixedOffset};

    let vehicles = vec![
        Train {
            lat: Some(42.35),
            long: Some(-71.05),
            route_id: Some("Red".to_string()),
            stop_id: Some("70061".to_string()),
            vehicle_id: "R-123".to_string(),
        },
        Train {
            lat: Some(42.36),
            long: Some(-71.06),
            route_id: Some("Orange".to_string()),
            stop_id: Some("70001".to_string()),
            vehicle_id: "O-456".to_string(),
        },
    ];

    let predictions = vec![
        PredictedTimes {
            arrival_time: Some("2026-09-19T19:30:00-04:00".parse::<DateTime<FixedOffset>>().unwrap()),
            departure_time: Some("2026-09-19T19:31:00-04:00".parse::<DateTime<FixedOffset>>().unwrap()),
            status: Some("Stopped at station".to_string()),
            stop_number: 5,
            direction_id: 1,
            vehicle_id: Some("R-123".to_string()),
        },
        PredictedTimes {
            arrival_time: None,
            departure_time: None,
            status: None,
            stop_number: 3,
            direction_id: 0,
            vehicle_id: Some("UNKNOWN".to_string()), // vehicle not in map
        },
    ];

    let snapshot = combine_vehicles(predictions, vehicles);

    assert_eq!(snapshot.trains.len(), 2);

    // Find the R-123 train and check it got prediction data
    let red_train = snapshot.trains.iter().find(|t| t.vehicle_id == "R-123").unwrap();
    assert_eq!(red_train.predictions.len(), 1);
    assert!(red_train.predictions[0].arrival_time.is_some());
    assert_eq!(red_train.predictions[0].stop_number, 5);
    assert_eq!(red_train.predictions[0].direction_id, 1);
    assert_eq!(red_train.predictions[0].status, Some("Stopped at station".to_string()));

    // O-456 should have no predictions
    let orange_train = snapshot.trains.iter().find(|t| t.vehicle_id == "O-456").unwrap();
    assert!(orange_train.predictions.is_empty());

    println!("combine_vehicles test passed!");
}

#[tokio::test]
async fn test_combine_vehicles_live() {
    let client = reqwest::Client::new();
    let vehicles = t_scope::mbta::fetch_vehicles(&client).await.unwrap();
    let predictions = t_scope::mbta::fetch_predictions(&client).await.unwrap();

    let snapshot = t_scope::mbta::combine_vehicles(predictions, vehicles);

    println!("Combined {} trains", snapshot.trains.len());

    // Count how many have prediction data
    let with_predictions = snapshot.trains.iter().filter(|t| !t.predictions.is_empty()).count();
    println!("Trains with predictions: {}", with_predictions);

    for train in snapshot.trains.iter().take(5) {
        println!("{:?}", train);
    }

    assert!(!snapshot.trains.is_empty());
}
