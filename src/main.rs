use std::str::FromStr;

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
}

#[derive(Serialize, Deserialize)]
struct ServerConfig {
    id: String,
}

#[derive(Serialize, Deserialize)]
struct DathostConfig {
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct DiscordConfig {
    token: String,
}

enum Command {
    JOIN,
    LEAVE,
    LIST,
    START,
    UNKNOWN,
}

struct Handler;

struct UserQueue;

impl TypeMapKey for UserQueue {
    type Value = Vec<User>;
}

impl TypeMapKey for Config {
    type Value = Config;
}

impl FromStr for Command {
    type Err = ();

    fn from_str(input: &str) -> Result<Command, Self::Err> {
        match input {
            "!join" => Ok(Command::JOIN),
            "!leave" => Ok(Command::LEAVE),
            "!list" => Ok(Command::LIST),
            "!start" => Ok(Command::START),
            _ => Err(()),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        if msg.author.bot { return; }
        if !msg.content.starts_with("!") { return; }
        let command = Command::from_str(&msg.content).unwrap_or(Command::UNKNOWN);
        match command {
            Command::JOIN => bot_service::handle_join(context, msg).await,
            Command::LEAVE => bot_service::handle_leave(context, msg).await,
            Command::LIST => bot_service::handle_list(context, msg).await,
            Command::START => bot_service::handle_start(context, msg).await,
            Command::UNKNOWN => bot_service::handle_unknown(context, msg).await,
        }
    }
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() -> () {
    let config = read_config().await.unwrap();
    let token = config.discord.token.to_string();
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("~"));
    let mut client = Client::new(&token)
        .event_handler(Handler {})
        .framework(framework)
        .await
        .expect("Error creating client");
    {
        let mut data = client.data.write().await;
        data.insert::<UserQueue>(Vec::new());
        data.insert::<Config>(config);
    }
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

async fn read_config() -> Result<Config, serde_yaml::Error> {
    let yaml = std::fs::read_to_string("config.yaml").unwrap();
    let config: Config = serde_yaml::from_str(&yaml)?;
    Ok(config)
}
