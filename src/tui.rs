use std::{
    collections::HashMap,
    process::exit,
    time::{Duration, SystemTime},
};

use crate::{
    buy_cmd::buy,
    crops::crop_registry,
    harvest_cmd,
    market::{generate_market, market_listing},
    models::{FarmState, Plot},
    persistence::{self, save_farm},
    plant_cmd::plant,
    plot_pricing::next_plot_price,
    sell::sell_crop,
    stats::compute_stats,
};
use humantime::format_duration;
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Direction, Layout},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState,
        Paragraph, Wrap,
    },
    DefaultTerminal, Frame,
};
use ratatui_notifications::{Level, Notification, Notifications};
use uuid::Uuid;

static NAVIGATION_TEXT: &str = " Change Tabs: <Tab/Shift+Tab>, Quit <q> ";
static TUTORIAL_TEXT: &str = "
Welcome to termfarm!

Basic controls:
- <Tab/Shift+Tab> Move between tabs (the current tab is shown at the top-left)
- <q> Quit

Farm controls:
- <p> Plant a crop in the first open plot
- <h> Harvest all mature crops
- <n> Purchase a new plot

Inventory controls:
- <s> Sell harvested crops

Market controls:
- <1> Buy seed 1
- <2> Buy seed 2
- <3> Buy seed 3

> Have fun :D
Press <esc> to close this tutorial
";

pub fn run() {
    let _ = ratatui::run(|terminal| App::new().run(terminal));
}

#[derive(PartialEq)]
enum Tabs {
    Farm,
    Inventory,
    Market,
}

pub struct App {
    active_tab: Tabs,
    running: bool,
    farm: FarmState,
    notifications: Notifications,
    plant_popup: bool,
    plant_list_state: ListState,
}

impl App {
    pub fn new() -> Self {
        Self {
            active_tab: Tabs::Farm,
            running: true,
            farm: persistence::load_farm(),
            notifications: Notifications::new(),
            plant_popup: false,
            plant_list_state: ListState::default(),
        }
    }

    fn tab(&mut self, reverse: bool) {
        match self.active_tab {
            Tabs::Farm => match reverse {
                true => self.active_tab = Tabs::Market,
                false => self.active_tab = Tabs::Inventory,
            },
            Tabs::Inventory => match reverse {
                true => self.active_tab = Tabs::Farm,
                false => self.active_tab = Tabs::Market,
            },
            Tabs::Market => match reverse {
                true => self.active_tab = Tabs::Inventory,
                false => self.active_tab = Tabs::Farm,
            },
        }
    }

    fn owned_seeds(&self) -> Vec<(String, u16)> {
        let mut seeds = self
            .farm
            .inventory
            .seeds
            .as_ref()
            .map(|seeds| {
                seeds
                    .iter()
                    .filter(|(_, amount)| **amount > 0)
                    .map(|(seed, amount)| (seed.clone(), *amount))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        seeds.sort_by(|a, b| a.0.cmp(&b.0));

        seeds
    }

    fn plant_owned_seed(&mut self) {
        let Some(index) = self.plant_list_state.selected() else {
            return;
        };

        let owned_seeds = self.owned_seeds();

        let Some((seed_id, _)) = owned_seeds.get(index) else {
            return;
        };

        self.plant_popup = false;
        self.plant_list_state.select(None);

        let output = plant(seed_id.clone(), false);
        let notification = if output.starts_with("No available plots") {
            Notification::new(output)
                .title(" Cannot plant")
                .level(Level::Warn)
                .build()
                .unwrap()
        } else {
            Notification::new(output)
                .title(" Planted")
                .level(Level::Info)
                .build()
                .unwrap()
        };

        self.notifications.add(notification).unwrap();
    }

    pub fn run(
        &mut self,
        terminal: &mut DefaultTerminal,
    ) -> std::io::Result<()> {
        while self.running {
            terminal.draw(|frame| {
                self.draw(frame);
            })?;
            self.keybinds()?;
        }

        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.farm = persistence::load_farm();
        let registry = crop_registry();

        let total_items = self.farm.plots.len() + 1;
        let cols = (total_items as f64).sqrt().ceil() as usize;
        let rows = (total_items as f64 / cols as f64).ceil() as usize;
        let row_constraints: Vec<Constraint> =
            (0..rows).map(|_| Constraint::Fill(1)).collect();

        // MARK: master layout
        let master_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(3), Constraint::Fill(1)])
            .split(frame.area());

        // MARK: Farm tab layouts
        let farm_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(master_layout[1]);
        let mut farm_grid: Vec<ratatui::layout::Rect> =
            Vec::with_capacity(total_items);
        for r in 0..rows {
            let items_this_row = std::cmp::min(cols, total_items - r * cols);
            let col_constraints: Vec<Constraint> =
                (0..items_this_row).map(|_| Constraint::Fill(1)).collect();
            let row_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(col_constraints)
                .split(farm_rows[r]);
            for cell in row_layout.iter() {
                farm_grid.push(*cell);
            }
        }

        // MARK: Inventory tab layouts
        let mut inventory_seed_constraints: Vec<Constraint> = Vec::new();
        let mut inventory_crop_constraints: Vec<Constraint> = Vec::new();

        match &self.farm.inventory.seeds {
            Some(seeds) => {
                if seeds.is_empty() {
                    inventory_seed_constraints.push(Constraint::Length(8));
                }
                for _ in seeds {
                    inventory_seed_constraints.push(Constraint::Fill(
                        100 / seeds.iter().count() as u16,
                    ))
                }
            }
            None => inventory_seed_constraints.push(Constraint::Length(8)),
        }

        match &self.farm.inventory.crops {
            Some(crops) => {
                if crops.is_empty() {
                    inventory_crop_constraints.push(Constraint::Length(8));
                }
                for _ in crops {
                    inventory_crop_constraints.push(Constraint::Fill(
                        100 / crops.iter().count() as u16,
                    ))
                }
            }
            None => inventory_crop_constraints.push(Constraint::Length(8)),
        }

        let inventory_main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(3),
                Constraint::Fill(1),
                Constraint::Fill(1),
            ])
            .split(master_layout[1]);
        let inventory_seeds_container = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1), Constraint::Fill(1)])
            .split(inventory_main_layout[1]);
        let inventory_seeds_layout_vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints(&inventory_seed_constraints)
            .split(inventory_seeds_container[1]);
        let inventory_seeds_layout_horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(&inventory_seed_constraints)
            .split(inventory_seeds_layout_vertical[0]);

        let inventory_crops_container = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1), Constraint::Fill(1)])
            .split(inventory_main_layout[2]);
        let inventory_crops_layout_vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints(&inventory_crop_constraints)
            .split(inventory_crops_container[1]);
        let inventory_crops_layout_horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(&inventory_crop_constraints)
            .split(inventory_crops_layout_vertical[0]);

        // MARK: Market tab layouts
        let market_main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Length(3),
            ])
            .split(master_layout[1]);

        match self.active_tab {
            // MARK: Farm tab rendering
            Tabs::Farm => {
                frame.render_widget(
                    Paragraph::new("[Farm] | Inventory | Market").block(
                        Block::new()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Green))
                            .border_type(BorderType::Thick)
                            .title_top(" termfarm ")
                            .title_bottom(
                                Line::from(
                                    " Plant <p>, Harvest <h>, Buy new Plot <n>,"
                                        .to_string()
                                        + NAVIGATION_TEXT,
                                )
                                .right_aligned(),
                            ),
                    ),
                    master_layout[0],
                );
                let mut new_pos = 0;
                for (i, plot) in self.farm.plots.iter().enumerate() {
                    match plot.planted_crop.clone() {
                        Some(crop_id) => {
                            let crop = &registry[&crop_id];
                            let elapsed =
                                plot.planted_at.unwrap().elapsed().unwrap();
                            let remaining = crop.grow_time as i64
                                - elapsed.as_secs() as i64;
                            let mut color = Color::White;

                            let dur = if remaining <= 0 {
                                color = Color::Green;
                                "ready to harvest".to_string()
                            } else {
                                format_duration(Duration::from_secs(
                                    remaining as u64,
                                ))
                                .to_string()
                                    + " remaining"
                            };
                            let text =
                                format!("{} {}\n{}", crop.id, crop.icon, dur,);
                            frame.render_widget(
                                Paragraph::new(text)
                                    .block(
                                        Block::new()
                                            .borders(Borders::ALL)
                                            .border_style(
                                                Style::default().fg(color),
                                            )
                                            .border_type(BorderType::Double),
                                    )
                                    .wrap(Wrap { trim: true }),
                                farm_grid[i],
                            );

                            new_pos += 1;
                        }
                        None => {
                            frame.render_widget(
                                Paragraph::new("<empty>".gray()).block(
                                    Block::new()
                                        .borders(Borders::ALL)
                                        .border_style(
                                            Style::default().fg(Color::Gray),
                                        )
                                        .border_type(BorderType::Double),
                                ),
                                farm_grid[i],
                            );

                            new_pos += 1
                        }
                    };
                }
                frame.render_widget(
                    Paragraph::new(format!(
                        "+\nBuy a new plot for {} coins",
                        next_plot_price(self.farm.plots.len() as u16)
                    ))
                    .block(
                        Block::new()
                            .borders(Borders::ALL)
                            .border_style(Style::default())
                            .fg(Color::Green)
                            .border_type(BorderType::Thick),
                    )
                    .wrap(Wrap { trim: true }),
                    farm_grid[new_pos],
                );

                if self.plant_popup {
                    let owned_seeds = self.owned_seeds();
                    let popup_height = (owned_seeds.len() as u16 + 4)
                        .max(6)
                        .min(frame.area().height.saturating_sub(2));

                    let popup_area = frame.area().centered(
                        Constraint::Percentage(45),
                        Constraint::Length(popup_height),
                    );

                    let items = owned_seeds
                        .iter()
                        .enumerate()
                        .map(|(index, (seed_id, amount))| {
                            let shortcut = if index < 9 {
                                format!("{}", index + 1)
                            } else {
                                " ".to_string()
                            };

                            let icon = registry
                                .get(seed_id)
                                .map(|crop| crop.icon)
                                .unwrap_or("");

                            ListItem::new(Line::from(format!(
                                "{shortcut}{icon} {amount}x {seed_id}"
                            )))
                        })
                        .collect::<Vec<_>>();

                    let list = List::new(items)
                        .block(Block::new()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Cyan))
                            .border_type(BorderType::Double)
                            .title_top(" Plant a seed ")
                            .title_bottom(
                                Line::from(
                                    " 1-9 plant · ↑/↓ or j/k select · Enter plant · Esc close ",
                                    )
                                    .gray(),
                            )
                        )
                        .highlight_symbol(" > ")
                        .highlight_style(
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        );

                    frame.render_widget(Clear, popup_area);
                    frame.render_stateful_widget(
                        list,
                        popup_area,
                        &mut self.plant_list_state,
                    );
                }

                if !self.farm.has_seen_tutorial {
                    let popup_area = frame.area().centered(
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    );
                    frame.render_widget(Clear, popup_area);
                    frame.render_widget(
                        Paragraph::new(TUTORIAL_TEXT).block(
                            Block::new()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(Color::Cyan))
                                .border_type(BorderType::Double),
                        ),
                        popup_area,
                    );
                }
            }
            // MARK: Inventory tab rendering
            Tabs::Inventory => {
                frame.render_widget(
                    Paragraph::new("Farm | [Inventory] | Market").block(
                        Block::new()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Green))
                            .border_type(BorderType::Thick)
                            .title_top(" termfarm ")
                            .title_bottom(
                                Line::from(
                                    " Sell crops <s>,".to_string()
                                        + NAVIGATION_TEXT,
                                )
                                .right_aligned(),
                            ),
                    ),
                    master_layout[0],
                );
                frame.render_widget(
                    Paragraph::new(
                        format!(" Coins: {}", self.farm.coins).yellow(),
                    )
                    .block(
                        Block::new()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Yellow))
                            .border_type(BorderType::Double),
                    ),
                    inventory_main_layout[0],
                );
                match &self.farm.inventory.seeds {
                    Some(seeds) => {
                        frame.render_widget(
                            Paragraph::new("").block(
                                Block::new()
                                    .borders(Borders::ALL)
                                    .border_style(
                                        Style::default().fg(Color::Cyan),
                                    )
                                    .border_type(BorderType::Double)
                                    .title_top(" 󰹢 Seeds: "),
                            ),
                            inventory_seeds_container[0],
                        );
                        if seeds.is_empty() {
                            frame.render_widget(
                                Paragraph::new("<none>".gray()).block(
                                    Block::new()
                                        .borders(Borders::ALL)
                                        .border_style(
                                            Style::default().fg(Color::Gray),
                                        )
                                        .border_type(BorderType::Double),
                                ),
                                inventory_seeds_layout_horizontal[0],
                            );
                        }
                        let mut sorted: Vec<(&String, &u16)> =
                            seeds.iter().collect::<Vec<_>>();
                        sorted.sort_by(|a, b| a.0.cmp(b.0));
                        for (i, (seed, amount)) in sorted.iter().enumerate() {
                            let registry = crop_registry();
                            frame.render_widget(
                                Paragraph::new(format!(
                                    "{} {amount}x {seed}",
                                    registry[*seed].icon
                                ))
                                .block(
                                    Block::new()
                                        .borders(Borders::ALL)
                                        .border_style(
                                            Style::default().fg(Color::Cyan),
                                        )
                                        .border_type(BorderType::Double),
                                ),
                                inventory_seeds_layout_horizontal[i],
                            );
                        }
                    }
                    None => {
                        frame.render_widget(
                            Paragraph::new("<none>".gray()).block(
                                Block::new()
                                    .borders(Borders::ALL)
                                    .border_style(
                                        Style::default().fg(Color::Gray),
                                    )
                                    .border_type(BorderType::Double),
                            ),
                            inventory_seeds_layout_horizontal[0],
                        );
                    }
                }
                match &self.farm.inventory.crops {
                    Some(crops) => {
                        frame.render_widget(
                            Paragraph::new("").block(
                                Block::new()
                                    .borders(Borders::ALL)
                                    .border_style(
                                        Style::default().fg(Color::Cyan),
                                    )
                                    .border_type(BorderType::Double)
                                    .title_top("  Crops: "),
                            ),
                            inventory_crops_container[0],
                        );
                        if crops.is_empty() {
                            frame.render_widget(
                                Paragraph::new("<none>".gray()).block(
                                    Block::new()
                                        .borders(Borders::ALL)
                                        .border_style(
                                            Style::default().fg(Color::Gray),
                                        )
                                        .border_type(BorderType::Double),
                                ),
                                inventory_crops_layout_horizontal[0],
                            );
                        }
                        let mut sorted: Vec<(&String, &u16)> =
                            crops.iter().collect::<Vec<_>>();
                        sorted.sort_by(|a, b| a.0.cmp(b.0));
                        for (i, (crop, amount)) in sorted.iter().enumerate() {
                            let registry = crop_registry();
                            frame.render_widget(
                                Paragraph::new(format!(
                                    "{} {amount}x {crop}",
                                    registry[*crop].icon
                                ))
                                .block(
                                    Block::new()
                                        .borders(Borders::ALL)
                                        .border_style(
                                            Style::default().fg(Color::Cyan),
                                        )
                                        .border_type(BorderType::Double),
                                ),
                                inventory_crops_layout_horizontal[i],
                            );
                        }
                    }
                    None => {
                        frame.render_widget(
                            Paragraph::new("<none>".gray()).block(
                                Block::new()
                                    .borders(Borders::ALL)
                                    .border_style(
                                        Style::default().fg(Color::Gray),
                                    )
                                    .border_type(BorderType::Double),
                            ),
                            inventory_crops_layout_horizontal[0],
                        );
                    }
                }
            }
            // MARK: Market tab rendering
            Tabs::Market => {
                let stats = compute_stats(&self.farm);

                frame.render_widget(
                    Paragraph::new("Farm | Inventory | [Market]").block(
                        Block::new()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Green))
                            .border_type(BorderType::Thick)
                            .title_top(" termfarm ")
                            .title_bottom(
                                Line::from(
                                    " Buy seed <1/2/3>, Rotate Now <r>,".to_string()
                                        + NAVIGATION_TEXT,
                                )
                                .right_aligned(),
                            ),
                    ),
                    master_layout[0],
                );
                frame.render_widget(
                    Paragraph::new(
                        format!(" Coins: {}", self.farm.coins).yellow(),
                    )
                    .block(
                        Block::new()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Yellow))
                            .border_type(BorderType::Double),
                    ),
                    market_main_layout[0],
                );
                frame.render_widget(
                    Paragraph::new(
                        format!(
                            "󰑓 Rotates In: {}",
                            format_duration(Duration::from_secs(
                                stats.next_market_rotation_in.as_secs()
                            ))
                        )
                        .magenta(),
                    )
                    .block(
                        Block::new()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Magenta))
                            .border_type(BorderType::Double),
                    ),
                    market_main_layout[1],
                );
                frame.render_widget(
                    Paragraph::new("").block(
                        Block::new()
                            .title_top(" 󰹢 Seeds: ")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Cyan))
                            .border_type(BorderType::Double),
                    ),
                    market_main_layout[2],
                );

                for (i, seed) in
                    self.farm.market.available_seeds.clone().iter().enumerate()
                {
                    let crop = &registry[seed];
                    let listing = market_listing(seed.to_string(), &self.farm);
                    let modifier = listing.modifier - 1.0;

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

                    let buy_line = if listing.buy_price
                        != listing.base_buy_price
                    {
                        Line::from(vec![
                            Span::from("Buy: "),
                            Span::styled(
                                format!("{}", listing.base_buy_price),
                                Style::default()
                                    .fg(Color::DarkGray)
                                    .add_modifier(Modifier::CROSSED_OUT),
                            ),
                            Span::from(format!(" {} coins", listing.buy_price)),
                        ])
                    } else {
                        Line::from(format!("Buy: {} coins", listing.buy_price))
                    };

                    let grow_time = format_duration(Duration::from_secs(
                        listing.grow_time as u64,
                    ));

                    let final_text = Text::from(vec![
                        Line::from(format!(
                            "({}) {} {}",
                            i + 1,
                            crop.icon,
                            crop.id
                        )),
                        buy_line,
                        Line::from(format!(
                            "Sell: {} coins ({} profit)",
                            listing.sell_price,
                            listing.sell_price - listing.buy_price
                        )),
                        Line::from(format!("Grow: {}", grow_time)),
                        Line::from(format!("({} {})", trend, pct)),
                    ]);

                    frame.render_widget(
                        Paragraph::new(final_text).block(
                            Block::new()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(Color::Cyan))
                                .border_type(BorderType::Double),
                        ),
                        market_main_layout[i + 3],
                    );
                }
                frame.render_widget(
                    Paragraph::new(
                        " Rotate now for 350 coins <r>".red(),
                    )
                    .block(
                        Block::new()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Red))
                            .border_type(BorderType::Double),
                    ),
                    market_main_layout[6],
                );
            }
        }

        self.notifications.tick(Duration::from_millis(16));
        self.notifications.render(frame, frame.area());
    }

    fn keybinds(&mut self) -> std::io::Result<()> {
        let tick_rate = Duration::from_millis(1);

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key_event)
                    if key_event.kind == KeyEventKind::Press =>
                {
                    if self.plant_popup {
                        let owned_seed_count = self.owned_seeds().len();

                        match key_event.code {
                            KeyCode::Esc => {
                                self.plant_popup = false;
                                self.plant_list_state.select(None);
                            }

                            KeyCode::Char(key @ '1'..='9') => {
                                let index = key
                                    .to_digit(10)
                                    .map(|value| value as usize - 1)
                                    .unwrap_or(0);

                                if index < owned_seed_count {
                                    self.plant_list_state.select(Some(index));
                                    self.plant_owned_seed();
                                }
                            }

                            KeyCode::Up | KeyCode::Char('k') => {
                                if owned_seed_count > 0 {
                                    let selected = self
                                        .plant_list_state
                                        .selected()
                                        .unwrap_or(0);

                                    self.plant_list_state.select(Some(
                                        selected.saturating_sub(1),
                                    ));
                                }
                            }

                            KeyCode::Down | KeyCode::Char('j') => {
                                if owned_seed_count > 0 {
                                    let selected = self
                                        .plant_list_state
                                        .selected()
                                        .unwrap_or(0);

                                    let next = (selected + 1)
                                        .min(owned_seed_count - 1);

                                    self.plant_list_state.select(Some(next));
                                }
                            }

                            KeyCode::Enter => {
                                if let Some(index) =
                                    self.plant_list_state.selected()
                                    && index < owned_seed_count {
                                        self.plant_owned_seed();
                                    }
                            }

                            _ => {}
                        }

                        return Ok(());
                    }

                    match key_event.code {
                        KeyCode::Char('q') => {
                            persistence::save_farm(&self.farm);
                            self.running = false
                        }
                        KeyCode::Tab if !self.plant_popup => self.tab(false),
                        KeyCode::BackTab if !self.plant_popup => self.tab(true),
                        KeyCode::Char('h')
                            if self.active_tab == Tabs::Farm
                                && !self.plant_popup =>
                        {
                            let text = harvest_cmd::harvest(false);
                            let notif = Notification::new(text)
                                .title(" Harvested:")
                                .level(Level::Info)
                                .build()
                                .unwrap();

                            self.notifications.add(notif).unwrap();
                        }
                        KeyCode::Char('s')
                            if self.active_tab == Tabs::Inventory =>
                        {
                            let crops = self
                                .farm
                                .inventory
                                .crops
                                .get_or_insert_with(HashMap::new);
                            if crops.is_empty() {
                                let notif = Notification::new(
                                    "You don't have any crops to sell",
                                )
                                .title(" No crops to sell!")
                                .level(Level::Warn)
                                .build()
                                .unwrap();

                                self.notifications.add(notif).unwrap();
                            } else {
                                for (crop_id, amount) in crops {
                                    let output = sell_crop(
                                        crop_id.to_string(),
                                        *amount,
                                        false,
                                    );
                                    let notif = Notification::new(output)
                                        .title(" Sold")
                                        .build()
                                        .unwrap();
                                    self.notifications.add(notif).unwrap();
                                }
                            }
                        }
                        KeyCode::Char('1')
                            if self.active_tab == Tabs::Market =>
                        {
                            let output = buy(
                                self.farm.market.available_seeds[0].clone(),
                                1,
                                false,
                            );
                            let notif = Notification::new(output)
                                .title(" Bought")
                                .build()
                                .unwrap();
                            self.notifications.add(notif).unwrap();
                        }
                        KeyCode::Char('2')
                            if self.active_tab == Tabs::Market =>
                        {
                            let output = buy(
                                self.farm.market.available_seeds[1].clone(),
                                1,
                                false,
                            );
                            let notif = Notification::new(output)
                                .title(" Bought")
                                .build()
                                .unwrap();
                            self.notifications.add(notif).unwrap();
                        }
                        KeyCode::Char('3')
                            if self.active_tab == Tabs::Market =>
                        {
                            let output = buy(
                                self.farm.market.available_seeds[2].clone(),
                                1,
                                false,
                            );
                            let notif = Notification::new(output)
                                .title(" Bought")
                                .build()
                                .unwrap();
                            self.notifications.add(notif).unwrap();
                        }
                        KeyCode::Char('r') if self.active_tab == Tabs::Market => {
                            if self.farm.coins >= 350 {
                                self.farm.coins -= 350;
                                self.farm.market = generate_market();
                                match save_farm(&self.farm) {
                                    true => (),
                                    false => {
                                        usefulog::err("Failed to save farm");
                                        exit(1);
                                    }
                                }
                            } else {
                                let notif = Notification::new("Not enough coins to rotate the market")
                                    .title(" Not enough coins")
                                    .level(Level::Warn)
                                    .build()
                                    .unwrap();
                                self.notifications.add(notif).unwrap();
                            }
                        }
                        KeyCode::Char('n')
                            if self.active_tab == Tabs::Farm
                                && !self.plant_popup =>
                        {
                            let current_plots = self.farm.plots.len();
                            let price = next_plot_price(current_plots as u16);

                            if self.farm.coins < price as u32 {
                                let notif = Notification::new(
                                    "Not enough coins to purchase a new plot!",
                                )
                                .title(" Not enough coins")
                                .level(Level::Warn)
                                .build()
                                .unwrap();
                                self.notifications.add(notif).unwrap();
                            } else {
                                self.farm.coins -= price as u32;
                                self.farm.plots.push(Plot {
                                    id: Uuid::new_v4(),
                                    planted_crop: None,
                                    planted_at: None,
                                });
                                self.farm.last_updated = SystemTime::now();

                                match save_farm(&self.farm) {
                                    true => (),
                                    false => {
                                        usefulog::err("Failed to save farm");
                                        exit(1);
                                    }
                                }
                            }
                        }
                        KeyCode::Char('p') if self.active_tab == Tabs::Farm => {
                            let owned_seeds = self.owned_seeds();

                            if owned_seeds.is_empty() {
                                let notification = Notification::new(
                                    "You don't have any seeds to plant",
                                )
                                .title(" No seeds")
                                .level(Level::Warn)
                                .build()
                                .unwrap();

                                self.notifications.add(notification).unwrap();
                            } else {
                                self.plant_list_state.select(Some(0));
                                self.plant_popup = true;
                            }
                        }
                        KeyCode::Esc if self.plant_popup => {
                            self.plant_popup = false;
                            self.plant_list_state.select(None);
                        }
                        KeyCode::Esc if !self.farm.has_seen_tutorial => {
                            self.farm.has_seen_tutorial = true;
                            save_farm(&self.farm);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}
