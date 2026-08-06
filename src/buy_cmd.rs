use crate::{
    crops::crop_registry,
    market::buy_price,
    persistence::{load_farm, save_farm},
};
use std::{collections::HashMap, process::exit, time::SystemTime};

pub fn buy(seed_id: String, amount: u16, interactive: bool) -> String {
    let mut farm = load_farm();
    let registry = crop_registry();

    if amount == 0 {
        usefulog::err("amount must be greater than 0");
        exit(1);
    }
    if !registry.contains_key(&seed_id) {
        usefulog::err(format!("Unknown seed: {seed_id}"));
        exit(1);
    }
    if !farm.market.available_seeds.contains(&seed_id) {
        usefulog::err(format!("Seed {seed_id} is not currently available"));
        exit(1);
    }

    let crop = &registry[&seed_id];
    let unit_price = buy_price(seed_id.clone(), &farm);
    let total_price = unit_price * amount;

    if farm.coins < total_price as u32 {
        match interactive {
            true => {
                println!("Not enough coins.");
                println!(
                    "> You need {total_price}, but only have {}",
                    farm.coins
                );
                exit(1);
            }
            false => {
                return format!(
                    "Not enough coins.\nYou need {total_price}, but only have {}",
                    farm.coins
                );
            }
        }
    }

    farm.coins -= total_price as u32;
    let seeds = farm.inventory.seeds.get_or_insert_with(HashMap::new);
    *seeds.entry(seed_id).or_insert(0) += amount;
    farm.last_updated = SystemTime::now();

    match save_farm(&farm) {
        true => match interactive {
            true => {
                println!(
                    "󰄐 Bought {amount}x {} {} seeds for {total_price} coins ({unit_price} each)",
                    crop.icon, crop.id
                );
                "".to_string()
            }
            false => {
                format!(
                    "󰄐 Bought {amount}x {} {} seeds for {total_price} coins ({unit_price} each)",
                    crop.icon, crop.id
                )
            }
        },
        false => {
            usefulog::err("failed to save farm");
            exit(1);
        }
    }
}
