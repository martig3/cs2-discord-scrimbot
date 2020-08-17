use std::str::FromStr;

use serenity::async_trait;
use serenity::Client;
use serenity::client::Context;
use serenity::framework::standard::{
    StandardFramework,
};
use serenity::model::channel::Message;
use serenity::model::prelude::Ready;
use serenity::model::user::User;
use serenity::prelude::EventHandler;
use std::env;

mod bot_service;

enum Command {
    JOIN,
    LEAVE,
    LIST,
}

struct Handler { user_queue: Vec<User> }

impl Handler {
    pub fn new() -> Self {
        Self { user_queue: Vec::new() }
    }
}

impl FromStr for Command {
    type Err = ();

    fn from_str(input: &str) -> Result<Command, Self::Err> {
        match input {
            "!join" => Ok(Command::JOIN),
            "!leave" => Ok(Command::LEAVE),
            "!list" => Ok(Command::LIST),
            _ => Err(()),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        let command = Command::from_str(&msg.content).unwrap();
        match command {
            Command::JOIN => bot_service::handle_join(context, msg, &self).await,
            Command::LEAVE => bot_service::handle_leave(context, msg, &self).await,
            Command::LIST => bot_service::handle_list(context, msg, &self).await,
        }
    }
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() -> () {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment");
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("~")); // set the bot's prefix to "~"
    let mut client = Client::new(&token)
        .event_handler(Handler::new())
        .framework(framework)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
