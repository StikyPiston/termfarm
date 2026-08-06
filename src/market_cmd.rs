use crate::{
    crops::crop_registry,
    market::buy_price,
    persistence::{load_farm, save_farm},
    stats::compute_stats,
};
use humantime::format_duration;
use std::time::Duration;

pub fn market() {
    let farm = load_farm();
    match save_farm(&farm) {
        true => (),
        false => {
            usefulog::err("failed to save farm");
            std::process::exit(1);
        }
    };
    let stats = compute_stats(&farm);

    println!("󰄐 Seed Market");
    println!("-------------");
    println!(" Balance: {}", farm.coins);
    println!(
        "󰑓 Rotates In: {}",
        format_duration(Duration::from_secs(
            stats.next_market_rotation_in.as_secs()
        ))
    );
    println!("-------------");
    println!();

    for seed in farm.market.available_seeds.clone() {
        let registry = crop_registry();
        let crop = &registry[&seed];
        let price = buy_price(seed.clone(), &farm);
        let modifier = farm.market.price_modifiers[&seed.clone()] - 1.0;

        let trend = {
            if modifier > 0.0 {
                "󰔵"
            } else if modifier < 0.0 {
                "󰔳"
            } else {
                "󰔴"
            }
        };
        let pct = format!("{:.0}%", modifier * 100.0);

        println!("{} {} - {price} coins ({trend} {pct})", crop.icon, crop.id)
    }
}
