use colorize::AnsiColor;
use humantime::format_duration;

use crate::{crops::crop_registry, persistence::load_farm};
use std::time::Duration;

pub fn view() {
    let farm = load_farm();
    let registry = crop_registry();

    println!(" Plots");
    println!("-------");

    for (index, plot) in farm.plots.iter().enumerate() {
        let plot_number = index + 1;

        match plot.planted_crop.clone() {
            Some(crop_id) => match plot.planted_at {
                Some(at) => {
                    let crop = &registry[&crop_id];
                    let elapsed = at.elapsed().unwrap();
                    let remaining =
                        crop.grow_time as i64 - elapsed.as_secs() as i64;

                    if remaining <= 0 {
                        println!(
                            "[{plot_number}] {} {} {}",
                            crop.icon,
                            crop.id,
                            "ready to harvest".green()
                        )
                    } else {
                        println!(
                            "[{plot_number}] {} {} {} left",
                            crop.icon,
                            crop.id,
                            format_duration(Duration::from_secs(
                                remaining as u64
                            ))
                        )
                    }
                }
                None => {
                    usefulog::err(
                        "found crop without a planted_at value! this should not be possible.",
                    );
                    std::process::exit(1);
                }
            },
            None => println!("[{plot_number}] empty"),
        }
    }
}
