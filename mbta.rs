use serde::Deserialize;
use apiresponse::ApiResponse;
use serde_json::json;
use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;


#[derive(Deserialize)]
#[derive(Debug)]
pub struct Train {
    pub lat: Option<f64>,
    pub long: Option<f64>,
    pub route_id: Option<String>, // switch to array of ascii
    pub stop_id: Option<String>, // switch to array of ascii 
    pub vehicle_id: String, // switch to array of ascii
}

#[derive(Deserialize)]
#[derive(Debug)]
pub struct PredictedTimes {
   pub arrival_time: Option<DateTime<FixedOffset>>, // turn into ms 
   pub departure_time: Option<DateTime<FixedOffset>>, // turn into ms
   pub status: Option<String>, // switch to array of ascii
   pub stop_number: u64,
   pub direction_id: u64,
   pub vehicle_id: Option<String>, // switch to array of ascii
}

#[derive(Deserialize)]
#[derive(Debug)]
pub struct TrainState {
    pub lat: Option<f64>,
    pub long: Option<f64>,
    pub route_id: Option<String>,
    pub stop_id: Option<String>,
    pub vehicle_id: String, 
    pub predictions: Vec<PredictedTimes>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SharedPrediction {
    pub arrival_time_ms: i64,
    pub departure_time_ms: i64,
    pub status: [u8; 32],
    pub stop_number: u64,
    pub direction_id: u64,
}

impl Default for SharedPrediction {
    fn default() -> Self {
        SharedPrediction {
            arrival_time_ms: i64::MIN,
            departure_time_ms: i64::MIN,
            status: [0u8; 32],
            stop_number: 0,
            direction_id: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SharedTrainState {
    pub lat: f64,
    pub long: f64,
    pub route_id: [u8; 16],
    pub stop_id: [u8; 16],
    pub vehicle_id: [u8; 16],
    pub prediction_count: u32,
    pub predictions: [SharedPrediction; 64],
}

impl Default for SharedTrainState {
    fn default() -> Self {
        SharedTrainState {
            lat: f64::NAN,
            long: f64::NAN,
            route_id: [0u8; 16],
            stop_id: [0u8; 16],
            vehicle_id: [0u8; 16],
            prediction_count: 0,
            predictions: [SharedPrediction::default(); 64],
        }
    }
}

#[repr(C)]
pub struct SharedSnapshot {
    pub train_count: u32,
    pub trains: [SharedTrainState; 512],
}

pub struct Snapshot {
    pub trains: Vec<TrainState>,
}
pub fn turn_into_arr<const N: usize>(s: &str) -> [u8; N] {
    let mut arr = [0u8; N];
    let bytes = s.as_bytes();
    let len = bytes.len().min(N);
    arr[..len].copy_from_slice(&bytes[..len]);
    arr
}

pub fn convert_predicted_times(pred: &PredictedTimes) -> SharedPrediction {
    SharedPrediction {
        arrival_time_ms: pred.arrival_time.map(|dt| dt.timestamp_millis()).unwrap_or(i64::MIN),
        departure_time_ms: pred.departure_time.map(|dt| dt.timestamp_millis()).unwrap_or(i64::MIN),
        status: turn_into_arr(pred.status.as_deref().unwrap_or("")),
        stop_number: pred.stop_number,
        direction_id: pred.direction_id,
    }
}

pub fn convert_train_state(train: &TrainState) -> SharedTrainState {
    assert!(train.predictions.len() <= 64);
    let mut shared_train = SharedTrainState {
        lat: train.lat.unwrap_or(f64::NAN),
        long: train.long.unwrap_or(f64::NAN),
        route_id: turn_into_arr(train.route_id.as_deref().unwrap_or("")),
        stop_id: turn_into_arr(train.stop_id.as_deref().unwrap_or("")),
        vehicle_id: turn_into_arr(&train.vehicle_id),
        prediction_count: train.predictions.len().min(64) as u32,
        predictions: [SharedPrediction::default(); 64],
    };

    for (i, pred) in train.predictions.iter().take(64).enumerate() {
        shared_train.predictions[i] = convert_predicted_times(pred);
    }

    shared_train
}

pub fn convert_snapshot(snapshot: &Snapshot) -> SharedSnapshot {
    assert!(snapshot.trains.len() <= 512);

    let mut shared_snap = SharedSnapshot::default();
    shared_snap.train_count = snapshot.trains.len() as u32;

    for (i, train) in snapshot.trains.iter().enumerate() {
        shared_snap.trains[i] = convert_train_state(train);
    }

    shared_snap
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

pub fn combine_vehicles(predicted_times: Vec<PredictedTimes>, vehicles: Vec<Train>) -> Snapshot { 
    let mut trains: HashMap<String, TrainState> = HashMap::new();

    for v in vehicles {
        let state = TrainState {
            lat: v.lat,
            long: v.long,
            route_id: v.route_id,
            stop_id: v.stop_id,
            vehicle_id: v.vehicle_id.clone(),
            predictions: Vec::new(),
        };
        trains.insert(v.vehicle_id, state);
    }

    for p in predicted_times {
        if let Some(vehicle_id) = p.vehicle_id.clone() {
            if let Some(train) = trains.get_mut(&vehicle_id) {
                train.predictions.push(p);
            }
        }
    }

    Snapshot {
        trains: trains.into_values().collect(),
    }
}
