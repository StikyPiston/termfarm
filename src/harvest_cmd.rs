use std::{collections::HashMap, time::SystemTime};

use crate::{
    crops::crop_registry,
    persistence::{load_farm, save_farm},
};

pub fn harvest(interactive: bool) -> String {
    let mut farm = load_farm();
    let registry = crop_registry();

    let mut harvested: HashMap<String, u16> = HashMap::new();

    for plot in &mut farm.plots {
        let Some(ref crop_id) = plot.planted_crop else {
            continue;
        };
        let Some(ref planted_at) = plot.planted_at else {
            continue;
        };
        let crop = &registry[crop_id];

        let grow_time = crop.grow_time;
        let age = planted_at.elapsed().unwrap().as_secs();

        if age < grow_time as u64 {
            continue;
        }

        let crop_count = farm
            .inventory
            .crops
            .get_or_insert_with(HashMap::new)
            .entry(crop_id.clone())
            .or_insert(0);

        *crop_count += 1;
        *harvested.entry(crop_id.to_string()).or_insert(0) += 1;

        plot.planted_crop = None;
        plot.planted_at = None;
    }

    farm.last_updated = SystemTime::now();
    match save_farm(&farm) {
        true => {
            if interactive {
                if harvested.is_empty() {
                    println!(" No crops to harvest");
                    "".to_string()
                } else {
                    println!("󱕓 Harvested:");
                    for (crop, amount) in harvested {
                        println!(" +{amount} {crop}")
                    }
                    "".to_string()
                }
            } else {
                if harvested.is_empty() {
                    " No crops to harvest".to_string()
                } else {
                    let mut content = String::new();
                    for (crop, amount) in harvested {
                        content
                            .push_str(format!("+ {amount} {crop}\n").as_str());
                    }
                    content
                }
            }
        }
        false => {
            usefulog::err("failed to save farm");
            std::process::exit(1);
        }
    }
}
