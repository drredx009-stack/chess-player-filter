
#![windows_subsystem = "windows"]


use eframe::egui;
use std::sync::mpsc::{self, Receiver};
use tokio::runtime::Runtime;
use futures::stream::{self, StreamExt};
use reqwest;
use reqwest::header::USER_AGENT;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use eframe::icon_data::from_png_bytes;

#[derive(Debug, Clone)]
enum GoodBad {
    Good,
    Bad,
}

#[derive(Debug, Clone)]
enum BadGood {
    Good,
    Bad,
    BadGames,
    BadTime,
    ErrorTime,
    ErrorGame,
}

#[derive(Debug, Clone)]
struct PlayerResult {
    name: String,
    status: BadGood,
    age_days: u64,
    games: u64,
    last_rating: u64,
    best_rating: u64,
    league: String,
}

struct AppState {
    usernames: String,
    time_days: String,
    min_games: String,
    results: Vec<PlayerResult>,
    loading: bool,
    rx: Option<Receiver<Vec<PlayerResult>>>,
    rt: Runtime,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            usernames: "".into(),
            time_days: "90".into(),
            min_games: "300".into(),
            results: Vec::new(),
            loading: false,
            rx: None,
            rt: Runtime::new().unwrap(),
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(20, 20, 20);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(30, 30, 30);
        ctx.set_visuals(visuals);

        if let Some(rx) = &self.rx {
            if let Ok(data) = rx.try_recv() {
                self.results = data;
                self.loading = false;
                self.rx = None;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);

                egui::Frame::group(ui.style())
                    .inner_margin(egui::Margin::same(20.0))
                    .show(ui, |ui| {
                        ui.set_max_width(900.0);

                        ui.heading("♟ Chess Player Filter");
                        ui.label("Analyze player account quality");

                        ui.add_space(15.0);

                        ui.group(|ui| {
                            ui.label("Usernames (comma separated)");

                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(30, 30, 30))
                                .rounding(5.0)
                                .inner_margin(egui::Margin::same(6.0))
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.usernames)
                                            .hint_text("e.g. magnus, hikaru"),
                                    );
                                });

                            ui.add_space(8.0);

                            ui.label("Account age (days)");

                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(30, 30, 30))
                                .rounding(5.0)
                                .inner_margin(egui::Margin::same(6.0))
                                .show(ui, |ui| {
                                    ui.text_edit_singleline(&mut self.time_days);
                                });

                            ui.add_space(8.0);

                            ui.label("Minimum games");

                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(30, 30, 30))
                                .rounding(5.0)
                                .inner_margin(egui::Margin::same(6.0))
                                .show(ui, |ui| {
                                    ui.text_edit_singleline(&mut self.min_games);
                                });

                            ui.add_space(10.0);

                            ui.add_enabled_ui(!self.loading, |ui| {
                                let button = egui::Button::new("Check Player")
                                    .fill(egui::Color32::from_rgb(60, 140, 240))
                                    .rounding(6.0)
                                    .min_size(egui::vec2(ui.available_width(), 40.0));

                                if ui.add(button).clicked() {
                                    self.start_check();
                                }
                            });
                        });

                        ui.separator();

                        if self.loading {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Checking players...");
                            });
                        }

                        ui.add_space(10.0);

                        egui::Frame::group(ui.style())
                            .fill(egui::Color32::from_rgb(25, 25, 25))
                            .inner_margin(egui::Margin::same(15.0))
                            .show(ui, |ui| {
                                ui.heading("Results");

                                ui.add_space(10.0);

                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .max_height(300.0)
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());

                                        for r in &self.results {
                                            let (text, color) = match &r.status {
                                                BadGood::Good => ("GOOD ✔", egui::Color32::GREEN),
                                                BadGood::Bad => ("BAD ✖", egui::Color32::RED),
                                                BadGood::BadGames => ("LOW GAMES ⚠", egui::Color32::YELLOW),
                                                BadGood::BadTime => ("TOO NEW ⚠", egui::Color32::LIGHT_YELLOW),
                                                BadGood::ErrorTime => ("ERROR TIME", egui::Color32::RED),
                                                BadGood::ErrorGame => ("ERROR GAME", egui::Color32::RED),
                                            };

                                            ui.horizontal(|ui| {
                                                ui.label(format!("Name: [{}]",&r.name));
                                                ui.label(format!("Games: [{}]", display(r.games)));
                                                ui.label(format!("Age: [{} days]", display(r.age_days)));
                                                ui.label(format!("Last_elo: [{}]", display(r.last_rating)));
                                                ui.label(format!("Best_elo: [{}]", display(r.best_rating)));
                                                ui.label(format!("League: [{}]", r.league));

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        ui.colored_label(color, text);
                                                    },
                                                );
                                            });

                                            ui.separator();
                                        }
                                    });
                            });
                    });
            });
        });

        ctx.request_repaint();
    }
}

impl AppState {
    fn start_check(&mut self) {
        
        if self.usernames.trim().is_empty() {
            return;
        }


        self.loading = true;
        self.results.clear();

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        let usernames = self.usernames.clone();
        let time = self.time_days.parse::<u64>().unwrap_or(0);
        let games = self.min_games.parse::<u64>().unwrap_or(0);

        self.rt.spawn(async move {
            let client = reqwest::Client::new();

            let players: Vec<String> = usernames
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();

            let result = worker(&players, &client, &time, &games).await;

            if let Ok(r) = result {
                let _ = tx.send(r);
            } else {
                eprintln!("Worker failed");
            }

        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let icon = include_bytes!("icons8-chess-64.png");
    let icon = from_png_bytes(icon).ok();

    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([950.0, 650.0])
            .with_resizable(false)
            .with_title("Chess Player Filter v0.1")
            .with_icon(icon.unwrap()),
        ..Default::default()
    };

    eframe::run_native(
        "Chess Player Filter",
        options,
        Box::new(|_cc| Box::new(AppState::default())),
    )
}



async fn check_time(
    url: &str,
    client: &reqwest::Client,
    time: &u64,
) -> Result<(GoodBad, u64, String), String> {
    let res = client
        .get(url)
        .header(USER_AGENT, "ChessPlayerFilter/1.0 (contact: drredx009@gmail.com)")
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;

    let body: Value = res
        .json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))?;

    let joined = body["joined"].as_u64().ok_or("No joined field")?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let age_days = (now - joined) / 86400;

    let status = if age_days > *time {
        GoodBad::Good
    } else {
        GoodBad::Bad
    };


    let league_rank = body["league"]
    .to_string();

    let league_rank = if league_rank == "null" {
       "Unranked".to_string()
    } else {
        league_rank
    };

    Ok((status, age_days , league_rank))
}

async fn check_game(
    url: &str,
    client: &reqwest::Client,
    game: &u64,
) -> Result<(GoodBad, u64, u64, u64), String> {
    let res = client
        .get(url)
        .header(USER_AGENT, "Mozilla/5.0")
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;

    let body: Value = res
        .json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))?;

    let mut total = 0;

    for mode in ["chess_daily", "chess_rapid", "chess_bullet", "chess_blitz"] {
        let record = &body[mode]["record"];

        total += record["win"].as_u64().unwrap_or(0);
        total += record["loss"].as_u64().unwrap_or(0);
        total += record["draw"].as_u64().unwrap_or(0);
    }

    let status = if total > *game {
        GoodBad::Good
    } else {
        GoodBad::Bad
    };

    let mut current_best:u64 = 0;
    for rating in ["chess_daily", "chess_rapid", "chess_bullet", "chess_blitz"] {
        let best = &body[rating]["last"]["rating"].as_u64().unwrap_or(0); 
        if best > &current_best {
            current_best = *best;
        }
    }

    let mut best_best:u64 = 0;
    for rating in ["chess_daily", "chess_rapid", "chess_bullet", "chess_blitz"] {
        let best = &body[rating]["best"]["rating"].as_u64().unwrap_or(0); 
        if best > &best_best {
            best_best = *best;
        }
    }

    Ok((status, total,current_best,best_best))
}

fn player_status(x: GoodBad, y: GoodBad) -> BadGood {
    match (x, y) {
        (GoodBad::Good, GoodBad::Good) => BadGood::Good,
        (GoodBad::Bad, GoodBad::Bad) => BadGood::Bad,
        (GoodBad::Bad, GoodBad::Good) => BadGood::BadTime,
        (GoodBad::Good, GoodBad::Bad) => BadGood::BadGames,
    }
}

async fn worker(
    players: &Vec<String>,
    client: &reqwest::Client,
    time: &u64,
    game: &u64,
) -> Result<Vec<PlayerResult>, String> {
    let results = stream::iter(players.clone())
        .map(|p| {
            let client = client.clone();
            let time = *time;
            let game = *game;

            async move {
                let url1 = format!("https://api.chess.com/pub/player/{}", p);
                let url2 = format!("https://api.chess.com/pub/player/{}/stats", p);

                let (t_status, age, league) =
                    match check_time(&url1, &client, &time).await {
                        Ok(v) => v,
                        Err(_) => {
                            return PlayerResult {
                                name: p,
                                status: BadGood::ErrorTime,
                                games: 0,
                                age_days: 0,
                                last_rating: 0,
                                best_rating: 0,
                                league: "Error".to_string(),
                            }
                        }
                    };

                let (g_status, games,current_best,best_best) =
                    match check_game(&url2, &client, &game).await {
                        Ok(v) => v,
                        Err(_) => {
                            return PlayerResult {
                                name: p,
                                status: BadGood::ErrorGame,
                                games: 0,
                                age_days: age,
                                last_rating: 0,
                                best_rating: 0,
                                league: "Error".to_string()
                            }
                        }
                    };

                PlayerResult {
                    name: p.clone(),
                    status: player_status(t_status, g_status),
                    games,
                    age_days: age,
                    last_rating: current_best,
                    best_rating: best_best,
                    league: league 
                }
            }
        })
        .buffer_unordered(3)
        .collect::<Vec<_>>()
        .await;

    let mut results = results;
    results.sort_by(|a,b| a.best_rating.cmp(&b.best_rating).reverse());
    results.sort_by_key(|r| rank_status(&r.status));

    Ok(results)
}



fn rank_status(status: &BadGood) -> u8 {
    match status {
        BadGood::Good => 0,
        BadGood::BadTime => 1,
        BadGood::BadGames => 2,
        BadGood::Bad => 3,
        BadGood::ErrorTime => 4,
        BadGood::ErrorGame => 5,
    }
}

fn display(val: u64) -> String {
    if val == 0 { "N/A".into() } else { val.to_string() }
}