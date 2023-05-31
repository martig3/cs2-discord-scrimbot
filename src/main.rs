use crate::admin::admin;
use crate::queue::queue;
use anyhow::Error;
use anyhow::Result;
use commands::admin;
use commands::queue;
use commands::start::start;
use commands::stats::stats;
use commands::steamid::steam_id;
use commands::teamname::teamname;
use dotenvy::dotenv;
use futures::lock::Mutex;
use poise::{builtins::create_application_commands, Event, Framework, FrameworkOptions};
use serde::{Deserialize, Serialize};
use serenity::model::gateway::GatewayIntents;
use serenity::model::user::User;
use std::collections::HashMap;
mod commands;
mod utils;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    dathost: DathostConfig,
    discord: DiscordConfig,
    post_setup_msg: Option<String>,
    autoclear_hour: Option<u32>,
    scrimbot_api_config: Option<ScrimbotApiConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ScrimbotApiConfig {
    scrimbot_api_url: Option<String>,
    scrimbot_api_user: Option<String>,
    scrimbot_api_password: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DathostConfig {
    username: String,
    password: String,
    match_end_url: Option<String>,
    server_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    token: String,
    admin_role_id: u64,
    team_a_channel_id: Option<u64>,
    team_b_channel_id: Option<u64>,
    emote_ct_id: Option<u64>,
    emote_t_id: Option<u64>,
    emote_ct_name: Option<String>,
    emote_t_name: Option<String>,
    assign_role_id: Option<u64>,
}

#[derive(PartialEq)]
struct StateContainer {
    state: State,
}

#[derive(Clone)]
pub struct Draft {
    captain_a: Option<User>,
    captain_b: Option<User>,
    team_a: Vec<User>,
    team_b: Vec<User>,
    team_b_start_side: String,
    current_picker: Option<User>,
    map_votes: HashMap<User, Vec<String>>,
    selected_map: String,
}

#[derive(Clone, PartialEq)]
pub enum State {
    Queue,
    MapPick,
    CaptainPick,
    DraftTypePick,
    Draft,
    SidePick,
    Ready,
}

pub enum Command {
    JOIN,
    LEAVE,
    QUEUE,
    START,
    STEAMID,
    STATS,
    TEAMNAME,
    MAPS,
    ADDMAP,
    CANCEL,
    REMOVEMAP,
    KICK,
    CAPTAIN,
    AUTODRAFT,
    MANUALDRAFT,
    PICK,
    READY,
    UNREADY,
    CT,
    T,
    READYLIST,
    RECOVERQUEUE,
    CLEAR,
    HELP,
    UNKNOWN,
}

pub struct Data {
    pub user_queue: Mutex<Vec<User>>,
    pub ready_queue: Mutex<Vec<User>>,
    pub queue_messages: Mutex<HashMap<u64, String>>,
    pub config: Config,
    pub steam_id_cache: Mutex<HashMap<u64, String>>,
    pub team_names: Mutex<HashMap<u64, String>>,
    pub state: Mutex<State>,
    pub maps: Mutex<Vec<String>>,
    pub draft: Mutex<Draft>,
}

type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("csgo-discord-scrimbot", log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let config = read_config().await?;

    let framework = Framework::<_, Error>::builder()
        .options(FrameworkOptions {
            commands: vec![queue(), admin(), steam_id(), teamname(), start(), stats()],
            event_handler: move |context, event, framework, _data| {
                Box::pin(async move {
                    if let Event::Ready { data_about_bot } = event {
                        let commands_builder =
                            create_application_commands(&framework.options().commands);
                        let commands_count = commands_builder.0.len();
                        for guild in &data_about_bot.guilds {
                            let guild = guild.id.to_partial_guild(&context).await?;

                            let commands_builder = commands_builder.clone();
                            guild
                                .id
                                .set_application_commands(&context, |builder| {
                                    *builder = commands_builder;
                                    builder
                                })
                                .await?;

                            log::info!(
                                "Registered {} commands for `{}`.",
                                commands_count,
                                guild.name
                            );
                        }
                    }
                    Ok(())
                })
            },
            ..Default::default()
        })
        .token(config.discord.token)
        .intents(GatewayIntents::empty())
        .setup(move |_context, _ready, _framework| {
            Box::pin(async move {
                Ok(Data {
                    state: Mutex::new(State::Queue),
                    config: read_config().await?,
                    draft: Mutex::new(Draft {
                        captain_a: None,
                        captain_b: None,
                        current_picker: None,
                        team_a: Vec::new(),
                        team_b: Vec::new(),
                        team_b_start_side: String::from(""),
                        map_votes: HashMap::new(),
                        selected_map: String::new(),
                    }),
                    maps: Mutex::new(read_maps().await?),
                    queue_messages: Mutex::new(read_queue_msgs().await?),
                    steam_id_cache: Mutex::new(read_steam_ids().await?),
                    team_names: Mutex::new(read_teamnames().await?),
                    ready_queue: Mutex::new(Vec::new()),
                    user_queue: Mutex::new(read_queue().await?),
                })
            })
        });

    if let Err(error) = framework.run().await {
        log::error!("Error: {}", error);
    } else {
        log::info!("Started csgo-matchbot")
    }
    Ok(())
}

async fn read_config() -> Result<Config, serde_yaml::Error> {
    let yaml = std::fs::read_to_string("config/config.yaml").unwrap();
    let config: Config = serde_yaml::from_str(&yaml)?;
    Ok(config)
}

async fn read_steam_ids() -> Result<HashMap<u64, String>, serde_json::Error> {
    if std::fs::read("data/steam-ids.json").is_ok() {
        let json_str = std::fs::read_to_string("data/steam-ids.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(HashMap::new())
    }
}

async fn read_teamnames() -> Result<HashMap<u64, String>, serde_json::Error> {
    if std::fs::read("data/teamnames.json").is_ok() {
        let json_str = std::fs::read_to_string("data/teamnames.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(HashMap::new())
    }
}

async fn read_maps() -> Result<Vec<String>, serde_json::Error> {
    if std::fs::read("data/maps.json").is_ok() {
        let json_str = std::fs::read_to_string("data/maps.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(Vec::new())
    }
}

async fn read_queue() -> Result<Vec<User>, serde_json::Error> {
    if std::fs::read("data/queue.json").is_ok() {
        let json_str = std::fs::read_to_string("data/queue.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(Vec::new())
    }
}

async fn read_queue_msgs() -> Result<HashMap<u64, String>, serde_json::Error> {
    if std::fs::read("data/queue-messages.json").is_ok() {
        let json_str = std::fs::read_to_string("data/queue-messages.json").unwrap();
        let json: HashMap<u64, String> = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(HashMap::new())
    }
}
