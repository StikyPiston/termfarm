use crate::crops::crop_registry;
use crate::models::{FarmState, MarketState};
use crate::persistence::save_farm;
use rand::seq::SliceRandom;
use rand::RngExt;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::time::SystemTime;

static MARKET_ROTATION_INTERVAL: f64 = (4 * 60 * 60) as f64;
static MARKET_MAX_ITEMS: usize = 3;
static PRICE_MODIFIER_RANGE: RangeInclusive<f64> = 0.7..=1.3;

pub fn generate_market() -> MarketState {
    let registry = crop_registry();
    let mut rng = rand::rng();

    let mut all_seeds: Vec<&String> = registry.keys().collect();
    all_seeds.shuffle(&mut rng);
    let selection: Vec<String> = all_seeds
        .into_iter()
        .take(MARKET_MAX_ITEMS)
        .cloned()
        .collect();

    let mut modifiers: HashMap<String, f64> = HashMap::new();
    for seed in &selection {
        modifiers.insert(
            seed.to_string(),
            rng.random_range(PRICE_MODIFIER_RANGE.clone()),
        );
    }

    MarketState {
        available_seeds: selection.to_vec(),
        price_modifiers: modifiers,
        last_rotation: SystemTime::now(),
    }
}

pub fn update_market_if_needed(farm: &mut FarmState) {
    if farm
        .market
        .last_rotation
        .elapsed()
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        < MARKET_ROTATION_INTERVAL
    {
        return;
    };

    farm.market = generate_market();
    let _ = save_farm(farm);
}

pub struct MarketListing {
    pub base_buy_price: u16,
    pub buy_price: u16,
    pub sell_price: u16,
    pub grow_time: u16,
    pub modifier: f64,
}

pub fn market_listing(crop_id: String, farm: &FarmState) -> MarketListing {
    let registry = crop_registry();
    let crop = &registry[&crop_id];
    let modifier = farm
        .market
        .price_modifiers
        .get(&crop_id)
        .copied()
        .unwrap_or(1.0);

    MarketListing {
        base_buy_price: crop.base_buy_price,
        buy_price: ((crop.base_buy_price as f64) * modifier) as u16,
        sell_price: ((crop.base_sell_price as f64) * modifier) as u16,
        grow_time: crop.grow_time,
        modifier,
    }
}

pub fn buy_price(crop_id: String, farm: &FarmState) -> u16 {
    let registry = crop_registry();
    let crop = &registry[&crop_id];
    let modifier = farm.market.price_modifiers[&crop_id];
    ((crop.base_buy_price as f64) * modifier) as u16
}

pub fn sell_price(crop_id: String, farm: &FarmState) -> u16 {
    let registry = crop_registry();
    let crop = &registry[&crop_id];
    let modifier = farm
        .market
        .price_modifiers
        .get(&crop_id)
        .copied()
        .unwrap_or(1.0);
    ((crop.base_sell_price as f64) * modifier) as u16
}
