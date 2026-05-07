
#![windows_subsystem = "windows"]

use serde::{Serialize, Deserialize};
use eframe::egui;
use sqlx::{Row, sqlite::SqliteConnectOptions};
// use core::time;
use std::sync::mpsc::{self, Receiver};
use tokio::runtime::Runtime;
use futures::stream::{self, StreamExt};
use reqwest;
use reqwest::header::USER_AGENT;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use eframe::icon_data::from_png_bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use sqlx::{Sqlite, SqlitePool};

#[derive(Debug, Clone)]
enum GoodBad {
    Good,
    Bad,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum BadGood {
    Good,
    Bad,
    BadGames,
    BadTime,
    ErrorTime,
    ErrorGame,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    cache: Arc<Mutex<HashMap<String, (PlayerResult,u64)>>>,
    db: SqlitePool,
}

impl AppState {
    fn new(db: SqlitePool) -> Self {
        Self {
            usernames: "".into(),
            time_days: "90".into(),
            min_games: "300".into(),
            results: Vec::new(),
            loading: false,
            rx: None,
            rt: Runtime::new().unwrap(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            db,
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

        if self.loading {
            ctx.request_repaint();
        }
    }
}

impl AppState {
    fn start_check(&mut self) {
        
        if self.usernames.trim().is_empty() {
            return;
        }


        self.loading = true;
        self.results.clear();

        let (tx, rx) = 
        mpsc::channel();
        self.rx = Some(rx);

        let usernames = self.usernames.clone();
        let time = self.time_days.parse::<u64>().unwrap_or(0);
        let games = self.min_games.parse::<u64>().unwrap_or(0);
        let cache = self.cache.clone();
        let db = self.db.clone();

        self.rt.spawn(async move {
            let client = reqwest::Client::new();

            let players: Vec<String> = usernames
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .collect();

           

            let result = worker(&players, &client, &time, &games, cache, db).await;

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
    

    let rt = Runtime::new().unwrap();
    let db = rt.block_on(async{

        let file = SqliteConnectOptions::new()
        .filename("database.db")
        .create_if_missing(true);
 
        let pool =SqlitePool::connect_with(file).await
        .expect("Error while connecting to database");

        let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

        clean_db(now, &pool).await;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS player_cache (
                username TEXT PRIMARY KEY,
                data TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await
        .expect("Failed to create table");

        return pool;
    });
                
    
    
    
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
    Box::new(move |_cc| {
        Box::new(AppState::new(db.clone()))
    }),
)
}



async fn check_time(
    url: &str,
    client: &reqwest::Client,
    time: &u64,
) -> Result<(GoodBad, u64, String), String> {
    let res = client
        .get(url)
        .header(USER_AGENT, "ChessPlayerFilter/0.1 (contact: None)")
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
    cache: Arc<Mutex<HashMap<String, (PlayerResult, u64)>>>,
    db: SqlitePool,
) -> Result<Vec<PlayerResult>, String> {
   
    
    
   
    let results = stream::iter(players.iter().cloned())
        .map(|p| {
            let client = client.clone();
            let time = *time;
            let game = *game;
            let cache = cache.clone();
            let db = db.clone();

            async move {

                let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
                
                let username = p.clone();
                if let Some((cached,time)) = cache.lock().await
                .get(&username) {
                    if now - time < 1800 {
                        return cached.clone();
                    }
                }

                if let Some((db_cached, t)) = get_from_db(&db, &username).await {
                    if now - t < 3600 {
                        cache.lock().await.insert(username.clone(), (db_cached.clone(), t));
                        return db_cached;
                    }
                }

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

                let result = PlayerResult {
                    name: p.clone(),
                    status: player_status(t_status, g_status),
                    games,
                    age_days: age,
                    last_rating: current_best,
                    best_rating: best_best,
                    league: league 
                };

                let _ = 
                insert_in_db(&db, &p, &result, now).await; 

                cache.lock().await.insert(
                    username,
                    (result.clone(),now)
                );



                result
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

async fn get_from_db(
    db: &SqlitePool,
    username: &str,
) -> Option<(PlayerResult, u64)> 
{
    let row = sqlx::query(
        r#"
        SELECT data, timestamp FROM player_cache
        WHERE username = ?
        "#,   
    )
    .bind(username)
    .fetch_optional(db)
    .await
    .ok()??;

    let data_str: String = row.try_get("data").ok()?;
    let data: PlayerResult = serde_json::from_str(&data_str).ok()?;
    let timestamp:i64 = row.try_get("timestamp").ok()?;
    let timestamp = timestamp as u64;
    Some((data,timestamp)) 
    
}

async fn insert_in_db(
    db:&SqlitePool,
    username: &str,
    result: &PlayerResult,
    timestamp: u64,
) -> Result<(),sqlx::Error> 
{
    let json = serde_json::to_string(result)
    .expect("Failed to Serialize");

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO player_cache (username, data, timestamp)
        VALUES (?,?,?)
        "#
    )
    .bind(username)
    .bind(json)
    .bind(timestamp as i64)
    .execute(db)
    .await?;

    Ok(())
}



async fn clean_db(now: u64,pool: &SqlitePool) {
    sqlx::query(
    "DELETE FROM player_cache WHERE timestamp < ?"
    )
    .bind((now - 3600) as i64)
    .execute(pool)
    .await
    .ok();
}
