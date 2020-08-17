use std::cell::RefCell;

use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::user::User;
use serenity::utils::MessageBuilder;


pub(crate) async fn handle_join(context: Context, msg: Message, handler: &crate::Handler) {
    let author = &msg.author;
    if handler.user_queue.contains(author) {
        let response = MessageBuilder::new()
            .mention(author)
            .push(" is already in the queue.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    let rc_queue = RefCell::new(&handler.user_queue);
    rc_queue.borrow_mut().push(author.to_owned());
    let response = MessageBuilder::new()
        .mention(author)
        .push(" has been added to the queue.")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_leave(context: Context, msg: Message, handler: &crate::Handler) {
    let author = &msg.author.to_owned();
    if !handler.user_queue.contains(&author) {
        let response = MessageBuilder::new()
            .mention(author)
            .push(" is not in the queue. Type `!join` to join the queue.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    // need to re-write this bit
    let mut queue: Vec<User> = handler.user_queue.to_vec();
    let index = queue.iter().position(|r| r.id == author.id).unwrap();
    queue.remove(index);
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" has been added to the queue.")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_list(context: Context, msg: Message, handler: &crate::Handler) {
    // need to re-write this bit to add all the users in the queue
    let response = MessageBuilder::new()
        .push("Current queue size: ")
        .push(handler.user_queue.len().to_string())
        .push("/10")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}


