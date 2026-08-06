use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{crops::crop_registry, models::FarmState};

#[derive(Deserialize, Serialize)]
pub struct FarmStats {
    pub ready_to_harvest: u16,
    pub growing_crops: u16,
    pub total_plots: u16,
    pub inventory_crops: u16,
    pub inventory_seeds: u16,
    pub coins: u32,
    pub market_trend: f64,
    pub next_market_rotation_in: Duration,
}

pub fn compute_stats(farm: &FarmState) -> FarmStats {
    let mut ready = 0;
    let mut growing = 0;

    for plot in &farm.plots {
        let registry = crop_registry();
        let Some(ref crop_id) = plot.planted_crop else {
            continue;
        };
        let Some(planted_at) = plot.planted_at else {
            continue;
        };
        let crop = &registry[crop_id];

        if planted_at.elapsed().map(|d| d.as_secs_f64()).unwrap_or(0.0)
            >= crop.grow_time as f64
        {
            ready += 1
        } else {
            growing += 1
        };
    }

    let crops = farm
        .inventory
        .crops
        .as_ref()
        .map_or(0, |map| map.values().sum::<u16>());
    let seeds = farm
        .inventory
        .seeds
        .as_ref()
        .map_or(0, |map| map.values().sum::<u16>());

    fn trend(farm: &FarmState) -> f64 {
        let values = farm.market.price_modifiers.values();
        if farm.market.price_modifiers.is_empty() {
            return 0.0;
        }
        values.clone().fold(0.0, |f, &x| f + x)
            / (values.clone().count() as f64)
            - 1.0
    }

    let rotation_interval: f64 = (4 * 60 * 60) as f64;
    let remaining = rotation_interval
        - farm
            .market
            .last_rotation
            .elapsed()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

    FarmStats {
        ready_to_harvest: ready,
        growing_crops: growing,
        total_plots: farm.plots.len() as u16,
        inventory_crops: crops.to_owned(),
        inventory_seeds: seeds.to_owned(),
        coins: farm.coins,
        market_trend: trend(farm),
        next_market_rotation_in: Duration::from_secs_f64(remaining),
    }
}
