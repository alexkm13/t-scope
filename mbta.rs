use serde::Deserialize;
use apiresponse::ApiResponse;
use serde_json::json;
use chrono::{DateTime, FixedOffset};

#[derive(Deserialize)]
#[derive(Debug)]
pub struct Train {
    pub lat: Option<f64>,
    pub long: Option<f64>,
    pub route_id: Option<String>,
    pub stop_id: Option<String>,
    pub vehicle_id: String,
}

#[derive(Deserialize)]
#[derive(Debug)]
pub struct PredictedTimes {
   pub arrival_time: Option<DateTime<FixedOffset>>,
   pub departure_time: Option<DateTime<FixedOffset>>,
   pub status: Option<String>,
   pub stop_number: u64,
   pub direction_id: u64,
   pub vehicle_id: Option<String>,
}


pub async fn fetch_vehicles(client: &reqwest::Client) -> Result<Vec<Train>, Box<dyn std::error::Error>> {
    let url = "https://api-v3.mbta.com/vehicles";
    let response = client.get(url).send().await?;
    let body = response.text().await?;
    let mut trains: Vec<Train> = Vec::new();
    
    let json: serde_json::Value = serde_json::from_str(&body)?;
    if let Some(vehicles) = json["data"].as_array() {
        for vehicle in vehicles {
            let train = Train {
                lat: vehicle["attributes"]["latitude"].as_f64(),
                long: vehicle["attributes"]["longitude"].as_f64(),
                route_id: vehicle["relationships"]["route"]["data"]["id"].as_str().map(|s| s.to_string()),
                stop_id: vehicle["relationships"]["stop"]["data"]["id"].as_str().map(|s| s.to_string()),
                vehicle_id: vehicle["id"].as_str().unwrap().to_string(),
            };

            trains.push(train);
        }
    }
    
    Ok(trains)
}

pub async fn fetch_predictions(client: &reqwest::Client) -> Result<Vec<PredictedTimes>, Box<dyn std::error::Error>> {
    let url = "https://api-v3.mbta.com/predictions?filter[route]=Red,Orange,Blue,Green-B,Green-C,Green-D,Green-E";
    let response = client.get(url).send().await?;
    let body = response.text().await?;
    let mut predicted_times: Vec<PredictedTimes> = Vec::new();
    
    let json: serde_json::Value = serde_json::from_str(&body)?;
    if let Some(times) = json["data"].as_array() {
        for time in times {
            let predicted_time = PredictedTimes {
                 arrival_time: time["attributes"]["arrival_time"].as_str().and_then(|s| s.parse().ok()),
                 departure_time: time["attributes"]["departure_time"].as_str().and_then(|s| s.parse().ok()),
                 status: time["attributes"]["status"].as_str().map(|s| s.to_string()),
                 stop_number: time["attributes"]["stop_sequence"].as_u64().unwrap(),
                 direction_id: time["attributes"]["direction_id"].as_u64().unwrap(),
                 vehicle_id: time["relationships"]["vehicle"]["data"]["id"].as_str().map(|s| s.to_string()),
            };
            predicted_times.push(predicted_time);
        };
     }
    Ok(predicted_times)
}
