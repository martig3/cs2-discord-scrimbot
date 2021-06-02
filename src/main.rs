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
mod bot_service;

#[derive(Serialize, Deserialize)]
struct Config {
    server: ServerConfig,
    dathost: DathostConfig,
    discord: DiscordConfig,
    post_setup_msg: Option<String>,
    autoclear_hour: Option<u32>,
    scrimbot_api_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ServerConfig {
    id: String,
    url: String,
}

#[derive(Serialize, Deserialize)]
struct DathostConfig {
    username: String,
    password: String,
    match_end_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct DiscordConfig {
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

impl TypeMapKey for TeamNameCache{
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

enum Command {
    JOIN,
    LEAVE,
    LIST,
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
            _ if ".join".starts_with(input) => Ok(Command::JOIN),
            ".leave" => Ok(Command::LEAVE),
            ".list" => Ok(Command::LIST),
            _ if ".queue".starts_with(input) => Ok(Command::LIST),
            ".start" => Ok(Command::START),
            ".steamid" => Ok(Command::STEAMID),
            ".maps" => Ok(Command::MAPS),
            ".stats" => Ok(Command::STATS),
            ".teamname" => Ok(Command::TEAMNAME),
            ".kick" => Ok(Command::KICK),
            ".addmap" => Ok(Command::ADDMAP),
            ".cancel" => Ok(Command::CANCEL),
            ".captain" => Ok(Command::CAPTAIN),
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
            Command::JOIN => bot_service::handle_join(&context, &msg, &msg.author).await,
            Command::LEAVE => bot_service::handle_leave(context, msg).await,
            Command::LIST => bot_service::handle_list(context, msg).await,
            Command::START => bot_service::handle_start(context, msg).await,
            Command::STEAMID => bot_service::handle_steam_id(context, msg).await,
            Command::MAPS => bot_service::handle_map_list(context, msg).await,
            Command::STATS => bot_service::handle_stats(context, msg).await,
            Command::TEAMNAME => bot_service::handle_teamname(context, msg).await,
            Command::KICK => bot_service::handle_kick(context, msg).await,
            Command::CANCEL => bot_service::handle_cancel(context, msg).await,
            Command::ADDMAP => bot_service::handle_add_map(context, msg).await,
            Command::REMOVEMAP => bot_service::handle_remove_map(context, msg).await,
            Command::CAPTAIN => bot_service::handle_captain(context, msg).await,
            Command::PICK => bot_service::handle_pick(context, msg).await,
            Command::READY => bot_service::handle_ready(context, msg).await,
            Command::UNREADY => bot_service::handle_unready(context, msg).await,
            Command::CT => bot_service::handle_ct_option(context, msg).await,
            Command::T => bot_service::handle_t_option(context, msg).await,
            Command::READYLIST => bot_service::handle_ready_list(context, msg).await,
            Command::RECOVERQUEUE => bot_service::handle_recover_queue(context, msg).await,
            Command::CLEAR => bot_service::handle_clear(context, msg).await,
            Command::HELP => bot_service::handle_help(context, msg).await,
            Command::UNKNOWN => bot_service::handle_unknown(context, msg).await,
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
        data.insert::<UserQueue>(Vec::new());
        data.insert::<ReadyQueue>(Vec::new());
        data.insert::<QueueMessages>(HashMap::new());
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
