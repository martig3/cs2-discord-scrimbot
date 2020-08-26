use std::collections::HashMap;

use serenity::CacheAndHttp;
use serenity::client::Context;
use serenity::model::channel::{Message, ReactionType};
use serenity::model::user::User;
use serenity::utils::MessageBuilder;

use crate::{BotState, Config, Maps, State, StateContainer, SteamIdCache, UserQueue};

pub(crate) async fn handle_join(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    {
        let steam_id_cache: &HashMap<u64, String> = &data.get::<SteamIdCache>().unwrap();
        if !steam_id_cache.contains_key(msg.author.id.as_u64()) {
            let response = MessageBuilder::new()
                .mention(&msg.author)
                .push(" steamID not found for your discord user, \
                    please use `!steamid <your steamID>` to assign one. Example: `!steamid STEAM_0:1:12345678` ")
                .push("\nhttps://steamid.io/ is an easy way to find your steamID for your account")
                .build();
            if let Err(why) = msg.channel_id.say(&context.http, &response).await {
                println!("Error sending message: {:?}", why);
            }
            return;
        }
    }
    let user_queue: &mut Vec<User> = &mut data.get_mut::<UserQueue>().unwrap();
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
    let mut user_name = String::from("");
    for user in user_queue {
        user_name.push_str("\n- @");
        user_name.push_str(&user.name);
    }
    let queue_len = &user_queue.len();
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
    let mut data = context.data.write().await;
    let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
    if !user_queue.contains(&msg.author) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" is not in the queue or does not have the correct role")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    let bot_state: &mut StateContainer = &mut data.get_mut::<BotState>().unwrap();
    bot_state.state = State::MapPick;
    let maps: &Vec<String> = data.get::<Maps>().unwrap();
    let emoji_map = populate_unicode_emojis().await;
    let a_to_z = ('a'..'z').map(|f| f).collect::<Vec<_>>();
    let emoji_suffixes = a_to_z[..maps.len()].to_vec();
    let emojis: Vec<String> = emoji_suffixes
        .iter()
        .enumerate()
        .map(|(i, c)| format!(":regional_indicator_{}: `{}`\n", c, &maps[i]))
        .collect();
    let vote_text: String = emojis
        .iter()
        .map(|s| String::from(s))
        .collect();
    let response = MessageBuilder::new()
        .push_bold_line("Map Vote:")
        .push(vote_text)
        .build();
    let vote_msg = msg.channel_id.say(&context.http, &response).await.unwrap();
    for c in emoji_suffixes {
        vote_msg.react(&context.http, ReactionType::Unicode(String::from(emoji_map.get(&c).unwrap()))).await.unwrap();
    }
}

pub(crate) async fn handle_steam_id(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let steam_id_cache: &mut HashMap<u64, String> = &mut data.get_mut::<SteamIdCache>().unwrap();
    let steam_id_str: String = String::from(msg.content.trim().split(" ").take(2).collect::<Vec<_>>()[1]);
    steam_id_cache.insert(*msg.author.id.as_u64(), String::from(&steam_id_str));
    write_to_file(String::from("steam-ids.json"), serde_json::to_string(steam_id_cache).unwrap()).await;
    let response = MessageBuilder::new()
        .push("Updated steamID for ")
        .mention(&msg.author)
        .push(" to `")
        .push(&steam_id_str)
        .push("`")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_add_map(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let maps: &mut Vec<String> = data.get_mut::<Maps>().unwrap();
    if maps.len() == 26 {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" unable to add map, max amount reached.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    let map_name: String = String::from(msg.content.trim().split(" ").take(2).collect::<Vec<_>>()[1]);
    if maps.contains(&map_name) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" unable to add map, already exists.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    maps.push(String::from(&map_name));
    write_to_file(String::from("maps.json"), serde_json::to_string(maps).unwrap()).await;
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" added map: `")
        .push(&map_name)
        .push("`")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_remove_map(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let maps: &mut Vec<String> = data.get_mut::<Maps>().unwrap();
    let map_name: String = String::from(msg.content.trim().split(" ").take(2).collect::<Vec<_>>()[1]);
    if !maps.contains(&map_name) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" unable to remove map, doesn't exist in list.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    let index = maps.iter().position(|m| m == &map_name).unwrap();
    maps.remove(index);
    write_to_file(String::from("maps.json"), serde_json::to_string(maps).unwrap()).await;
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" removed map: `")
        .push(&map_name)
        .push("`")
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

pub(crate) async fn write_to_file(path: String, content: String) {
    let mut error_string = String::from("Error writing to ");
    error_string.push_str(&path);
    std::fs::write(path, content)
        .expect(&error_string);
}

pub(crate) async fn launch_server(context: &Context, msg: Message) {
    let data = context.data.write().await;
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" server is starting...")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }

    let config: &Config = data.get::<Config>().unwrap();
    let client = reqwest::Client::new();
    let dathost_username = &config.dathost.username;
    let dathost_password: Option<String> = Some(String::from(&config.dathost.password));
    let server_id = &config.server.id;
    let start_match_url = String::from("https://dathost.net/api/0.1/matches");

    let resp = client
        .put(&start_match_url)
        .form(&[("game_server_id", &server_id)])
        .basic_auth(&dathost_username, dathost_password)
        .send()
        .await
        .unwrap();
    println!("Start match response - {:#?}", resp);
}

pub(crate) async fn populate_unicode_emojis() -> HashMap<char, String> {
    // I hate this implementation and I deserve to be scolded
    // in my defense however, you have to provide unicode emojis to the api
    // if Discord allowed their shortcuts i.e. ":smile:" instead that would have been more intuitive
    let mut map = HashMap::new();
    map.insert('a', String::from("🇦"));
    map.insert('b', String::from("🇧"));
    map.insert('c', String::from("🇨"));
    map.insert('d', String::from("🇩"));
    map.insert('e', String::from("🇪"));
    map.insert('f', String::from("🇫"));
    map.insert('g', String::from("🇬"));
    map.insert('h', String::from("🇭"));
    map.insert('i', String::from("🇮"));
    map.insert('j', String::from("🇯"));
    map.insert('k', String::from("🇰"));
    map.insert('l', String::from("🇱"));
    map.insert('m', String::from("🇲"));
    map.insert('n', String::from("🇳"));
    map.insert('o', String::from("🇴"));
    map.insert('p', String::from("🇵"));
    map.insert('q', String::from("🇶"));
    map.insert('r', String::from("🇷"));
    map.insert('s', String::from("🇸"));
    map.insert('t', String::from("🇹"));
    map.insert('u', String::from("🇺"));
    map.insert('v', String::from("🇻"));
    map.insert('w', String::from("🇼"));
    map.insert('x', String::from("🇽"));
    map.insert('y', String::from("🇾"));
    map.insert('z', String::from("🇿"));
    return map;
}
