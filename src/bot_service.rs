use rand::{Rng, thread_rng};
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::user::User;
use serenity::utils::MessageBuilder;

use crate::{UserQueue, Config};

pub(crate) async fn handle_join(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let user_queue: &mut Vec<User> = data.get_mut::<UserQueue>().unwrap();
    if user_queue.contains(&msg.author) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" is already in the queue.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    user_queue.push(msg.author.clone());
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" has been added to the queue. Queue size: ")
        .push(user_queue.len().to_string())
        .push("/10")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_leave(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let user_queue: &mut Vec<User> = data.get_mut::<UserQueue>().unwrap();
    if !user_queue.contains(&msg.author) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" is not in the queue. Type `!join` to join the queue.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    let index = user_queue.iter().position(|r| r.id == msg.author.id).unwrap();
    user_queue.remove(index);
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" has left the queue. Queue size: ")
        .push(user_queue.len().to_string())
        .push("/10")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_list(context: Context, msg: Message) {
    let data = context.data.write().await;
    let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
    let mut user_name = "".to_string();
    for user in user_queue.to_vec() {
        user_name.push_str("\n- @");
        user_name.push_str(&user.name);
    }
    let queue_len = user_queue.len().to_string();
    let response = MessageBuilder::new()
        .push("Current queue size: ")
        .push(queue_len)
        .push("/10")
        .push(user_name)
        .build();

    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_start(context: Context, msg: Message) {
    let data = context.data.write().await;
    let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
    let config: &Config = data.get::<Config>().unwrap();
    if !user_queue.contains(&msg.author) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" is not in the queue or does not have the correct privileged role")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        // return;
    }
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" server is starting...")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }

    let client = reqwest::Client::new();
    let dathost_username = config.dathost.username.to_string();
    let dathost_password: Option<String> = Some(config.dathost.password.to_string());
    let server_id = config.server.id.to_string();
    let mut chg_password_url = "https://dathost.net/api/0.1/game-servers/"
        .to_string();
    chg_password_url.push_str(&server_id);
    let server_pass = &thread_rng().gen::<u32>().to_string();
    let resp = client
        .put(&chg_password_url)
        .form(&[("csgo_settings.password", &server_pass)])
        .send()
        .await
        .unwrap();
    println!("Change password response - {:#?}", resp.status());
    let mut stop_server_url = "https://dathost.net/api/0.1/game-servers/"
        .to_string();
    stop_server_url.push_str(&server_id);
    stop_server_url.push_str("/stop");
    let resp = client
        .post(&stop_server_url)
        .body("".to_string())
        .basic_auth(&dathost_username, dathost_password.as_ref())
        .send()
        .await
        .unwrap();
    println!("Stop server response - {:#?}", resp.status());
    let mut start_server_url = "https://dathost.net/api/0.1/game-servers/"
        .to_string();
    start_server_url.push_str(&server_id);
    start_server_url.push_str("/start");
    let resp = client
        .post(&start_server_url)
        .body("".to_string())
        .basic_auth(&dathost_username, dathost_password.as_ref())
        .send()
        .await
        .unwrap();
    println!("Start server response - {:#?}", resp.status());
    let response = MessageBuilder::new()
        .push("Server has completed startup. Check your DMs for server connection info.")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_unknown(context: Context, msg: Message) {
    let response = MessageBuilder::new()
        .push("Unknown command, type `!help` for list of commands.")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}
