use core::time::Duration as CoreDuration;
use std::collections::HashMap;
use std::str::FromStr;

use async_std::task;
use chrono::{Datelike, DateTime, Duration as ChronoDuration, Local, TimeZone};
use serde::{Deserialize, Serialize};
use serenity::async_trait;
use serenity::Client;
use serenity::client::Context;
use serenity::framework::standard::StandardFramework;
use serenity::model::channel::Message;
use serenity::model::prelude::Ready;
use serenity::model::user::User;
use serenity::prelude::{EventHandler, TypeMapKey};

mod commands;
mod utils;

#[derive(Serialize, Deserialize)]
pub struct Config {
    server: ServerConfig,
    dathost: DathostConfig,
    discord: DiscordConfig,
    post_setup_msg: Option<String>,
    autoclear_hour: Option<u32>,
    scrimbot_api_config: ScrimbotApiConfig,
}


#[derive(Serialize, Deserialize)]
pub struct ScrimbotApiConfig {
    scrimbot_api_url: Option<String>,
    scrimbot_api_user: Option<String>,
    scrimbot_api_password: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ServerConfig {
    id: String,
    url: String,
}

#[derive(Serialize, Deserialize)]
pub struct DathostConfig {
    username: String,
    password: String,
    match_end_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
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

struct Draft {
    captain_a: Option<User>,
    captain_b: Option<User>,
    team_a: Vec<User>,
    team_b: Vec<User>,
    team_b_start_side: String,
    current_picker: Option<User>,
}

#[derive(PartialEq)]
enum State {
    Queue,
    MapPick,
    CaptainPick,
    DraftTypePick,
    Draft,
    SidePick,
    Ready,
}

struct Handler;

struct UserQueue;

struct ReadyQueue;

struct SteamIdCache;

struct TeamNameCache;

struct BotState;

struct Maps;

struct QueueMessages;


impl TypeMapKey for UserQueue {
    type Value = Vec<User>;
}

impl TypeMapKey for ReadyQueue {
    type Value = Vec<User>;
}

impl TypeMapKey for Config {
    type Value = Config;
}

impl TypeMapKey for SteamIdCache {
    type Value = HashMap<u64, String>;
}

impl TypeMapKey for TeamNameCache {
    type Value = HashMap<u64, String>;
}

impl TypeMapKey for BotState {
    type Value = StateContainer;
}

impl TypeMapKey for Maps {
    type Value = Vec<String>;
}

impl TypeMapKey for Draft {
    type Value = Draft;
}

impl TypeMapKey for QueueMessages {
    type Value = HashMap<u64, String>;
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

impl FromStr for Command {
    type Err = ();
    fn from_str(input: &str) -> Result<Command, Self::Err> {
        match input {
            "." => Ok(Command::UNKNOWN),
            _ if ".join".starts_with(input) => Ok(Command::JOIN),
            _ if ".leave".starts_with(input) => Ok(Command::LEAVE),
            _ if ".queue".starts_with(input) => Ok(Command::QUEUE),
            ".start" => Ok(Command::START),
            ".steamid" => Ok(Command::STEAMID),
            ".maps" => Ok(Command::MAPS),
            ".stats" => Ok(Command::STATS),
            ".teamname" => Ok(Command::TEAMNAME),
            ".kick" => Ok(Command::KICK),
            ".addmap" => Ok(Command::ADDMAP),
            ".cancel" => Ok(Command::CANCEL),
            ".captain" => Ok(Command::CAPTAIN),
            ".autodraft" => Ok(Command::AUTODRAFT),
            ".manualdraft" => Ok(Command::MANUALDRAFT),
            ".pick" => Ok(Command::PICK),
            _ if ".ready".starts_with(input) => Ok(Command::READY),
            ".gaben" => Ok(Command::READY),
            ".unready" => Ok(Command::UNREADY),
            ".ct" => Ok(Command::CT),
            ".t" => Ok(Command::T),
            ".readylist" => Ok(Command::READYLIST),
            ".removemap" => Ok(Command::REMOVEMAP),
            ".recoverqueue" => Ok(Command::RECOVERQUEUE),
            ".clear" => Ok(Command::CLEAR),
            _ if ".help".starts_with(input) => Ok(Command::HELP),
            _ => Err(()),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        if msg.author.bot { return; }
        if !msg.content.starts_with('.') { return; }
        let command = Command::from_str(&msg.content.to_lowercase()
            .trim()
            .split(' ')
            .take(1)
            .collect::<Vec<_>>()[0])
            .unwrap_or(Command::UNKNOWN);
        match command {
            Command::JOIN => commands::handle_join(&context, &msg, &msg.author).await,
            Command::LEAVE => commands::handle_leave(context, msg).await,
            Command::QUEUE => commands::handle_list(context, msg).await,
            Command::START => commands::handle_start(context, msg).await,
            Command::STEAMID => commands::handle_steam_id(context, msg).await,
            Command::MAPS => commands::handle_map_list(context, msg).await,
            Command::STATS => commands::handle_stats(context, msg).await,
            Command::TEAMNAME => commands::handle_teamname(context, msg).await,
            Command::KICK => commands::handle_kick(context, msg).await,
            Command::CANCEL => commands::handle_cancel(context, msg).await,
            Command::ADDMAP => commands::handle_add_map(context, msg).await,
            Command::REMOVEMAP => commands::handle_remove_map(context, msg).await,
            Command::CAPTAIN => commands::handle_captain(context, msg).await,
            Command::AUTODRAFT => commands::handle_auto_draft(context, msg).await,
            Command::MANUALDRAFT => commands::handle_manual_draft(context, msg).await,
            Command::PICK => commands::handle_pick(context, msg).await,
            Command::READY => commands::handle_ready(context, msg).await,
            Command::UNREADY => commands::handle_unready(context, msg).await,
            Command::CT => commands::handle_ct_option(context, msg).await,
            Command::T => commands::handle_t_option(context, msg).await,
            Command::READYLIST => commands::handle_ready_list(context, msg).await,
            Command::RECOVERQUEUE => commands::handle_recover_queue(context, msg).await,
            Command::CLEAR => commands::handle_clear(context, msg).await,
            Command::HELP => commands::handle_help(context, msg).await,
            Command::UNKNOWN => commands::handle_unknown(context, msg).await,
        }
    }
    async fn ready(&self, context: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        autoclear_queue(&context).await;
    }
}

#[tokio::main]
async fn main() -> () {
    let config = read_config().await.unwrap();
    let token = &config.discord.token;
    let framework = StandardFramework::new();
    let mut client = Client::builder(&token)
        .event_handler(Handler {})
        .framework(framework)
        .await
        .expect("Error creating client");
    {
        let mut data = client.data.write().await;
        data.insert::<UserQueue>(read_queue().await.unwrap());
        data.insert::<ReadyQueue>(Vec::new());
        data.insert::<QueueMessages>(read_queue_msgs().await.unwrap());
        data.insert::<Config>(config);
        data.insert::<SteamIdCache>(read_steam_ids().await.unwrap());
        data.insert::<TeamNameCache>(read_teamnames().await.unwrap());
        data.insert::<BotState>(StateContainer { state: State::Queue });
        data.insert::<Maps>(read_maps().await.unwrap());
        data.insert::<Draft>(Draft {
            captain_a: None,
            captain_b: None,
            current_picker: None,
            team_a: Vec::new(),
            team_b: Vec::new(),
            team_b_start_side: String::from(""),
        });
    }
    if let Err(why) = client.start().await {
        eprintln!("Client error: {:?}", why);
    }
}

async fn read_config() -> Result<Config, serde_yaml::Error> {
    let yaml = std::fs::read_to_string("config.yaml").unwrap();
    let config: Config = serde_yaml::from_str(&yaml)?;
    Ok(config)
}

async fn read_steam_ids() -> Result<HashMap<u64, String>, serde_json::Error> {
    if std::fs::read("steam-ids.json").is_ok() {
        let json_str = std::fs::read_to_string("steam-ids.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(HashMap::new())
    }
}

async fn read_teamnames() -> Result<HashMap<u64, String>, serde_json::Error> {
    if std::fs::read("teamnames.json").is_ok() {
        let json_str = std::fs::read_to_string("teamnames.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(HashMap::new())
    }
}

async fn read_maps() -> Result<Vec<String>, serde_json::Error> {
    if std::fs::read("maps.json").is_ok() {
        let json_str = std::fs::read_to_string("maps.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(Vec::new())
    }
}

async fn read_queue() -> Result<Vec<User>, serde_json::Error> {
    if std::fs::read("queue.json").is_ok() {
        let json_str = std::fs::read_to_string("queue.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(Vec::new())
    }
}

async fn read_queue_msgs() -> Result<HashMap<u64, String>, serde_json::Error> {
    if std::fs::read("queue-messages.json").is_ok() {
        let json_str = std::fs::read_to_string("queue-messages.json").unwrap();
        let json: HashMap<u64, String> = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(HashMap::new())
    }
}

async fn autoclear_queue(context: &Context) {
    let autoclear_hour = get_autoclear_hour(context).await;
    if let Some(autoclear_hour) = autoclear_hour {
        println!("Autoclear feature started");
        loop {
            let current: DateTime<Local> = Local::now();
            let mut autoclear: DateTime<Local> = Local.ymd(current.year(), current.month(), current.day())
                .and_hms(autoclear_hour, 0, 0);
            if autoclear.signed_duration_since(current).num_milliseconds() < 0 { autoclear = autoclear + ChronoDuration::days(1) }
            let time_between: ChronoDuration = autoclear.signed_duration_since(current);
            task::sleep(CoreDuration::from_millis(time_between.num_milliseconds() as u64)).await;
            {
                let mut data = context.data.write().await;
                let user_queue: &mut Vec<User> = &mut data.get_mut::<UserQueue>().unwrap();
                user_queue.clear();
                let queued_msgs: &mut HashMap<u64, String> = data.get_mut::<QueueMessages>().unwrap();
                queued_msgs.clear();
            }
        }
    }
}

async fn get_autoclear_hour(client: &Context) -> Option<u32> {
    let data = client.data.write().await;
    let config: &Config = &data.get::<Config>().unwrap();
    config.autoclear_hour
}
