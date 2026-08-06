use std::{collections::HashMap, process::exit, time::SystemTime};

use crate::{
    market::sell_price,
    persistence::{load_farm, save_farm},
};

pub fn sell_crop(
    crop_id: String,
    mut amount: u16,
    interactive: bool,
) -> String {
    let mut farm = load_farm();

    let crops = farm.inventory.crops.get_or_insert_with(HashMap::new);
    let owned = crops.get(&crop_id).copied().unwrap_or(0);

    if amount == 0 {
        amount = owned;
    }

    *crops.entry(crop_id.clone()).or_insert(0) -= amount;
    if *crops.entry(crop_id.clone()).or_insert(0) == 0 {
        crops.remove_entry(&crop_id);
    }

    let price = sell_price(crop_id.clone(), &farm);
    let total = price * amount;

    farm.coins += total as u32;
    farm.last_updated = SystemTime::now();

    match save_farm(&farm) {
        true => match interactive {
            true => {
                println!(
                    "Sold {amount}x {crop_id} for {total} coins ({price} each)"
                );
                "".to_string()
            }
            false => {
                format!("{amount}x {crop_id} for {total} coins ({price} each)")
            }
        },
        false => {
            usefulog::err("failed to save farm");
            exit(1);
        }
    }
}
