use std::borrow::Borrow;
use std::env;
use tokio_postgres::{NoTls};
use serenity::async_trait;
use serenity::prelude::*;
use serenity::model::gateway::Ready;
use serenity::model::channel::{Message, ChannelType, Reaction};
use serenity::model::id::ChannelId;
use serenity::http::Http;
use std::sync::Arc;
use serenity::model::application::command::{Command, CommandOptionType};
use serenity::model::application::interaction::application_command::CommandDataOptionValue;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};

struct Bot;

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = serenity::Client::builder(token, intents)
        .event_handler(Bot)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        eprintln!("An error occurred while running the client: {:?}", why);
    }
}

// TODO: ths is definitely not the proper way to handle connections
pub async fn connect_to_db() -> tokio_postgres::Client {
    let db_uri_env =  env::var("DB_URI").expect("Expected Database URI");
    let db_uri =  db_uri_env.as_str();

    let (client, connection) = tokio_postgres::connect(db_uri, NoTls).await.unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    
    return client;
}

pub async fn create_board(command: &ApplicationCommandInteraction) -> String {
    let client = connect_to_db().await;

    let guild_id = command.guild_id.unwrap();
    let opts =  command.data.options.clone();

    let channel_dov = opts.get(0)
        .expect("Expected channel")
        .resolved
        .as_ref()
        .expect("Expected channel value");

    let emoji_dov = opts.get(1)
        .expect("Expected emoji")
        .resolved
        .as_ref()
        .expect("Expected emoji value");

    let threshold_dov = opts.get(2)
        .expect("Expected threshold")
        .resolved
        .as_ref()
        .expect("Expected Integer value");

    if let  CommandDataOptionValue::Channel(channel) = channel_dov {
        if let  CommandDataOptionValue::String(emoji) = emoji_dov {
            if let  CommandDataOptionValue::Integer(threshold)  = threshold_dov { 

                let guild_id_int: i64 = *guild_id.as_u64() as i64;
                let channel_id_int: i64 = *channel.id.as_u64() as i64;
                let channel_id_str: String = channel.id.to_string();

                if emoji.contains("<") {
                    return String::from("I can't work with non-unicode reactions yet! Sorry!");
                }

                if emojis::get(emoji).is_none() {
                    return String::from("You seem to have passed an invalid reaction! I currently only support unicode emojis.");
                }
                
                let result = client.execute(
                    "INSERT INTO boards (emoji, threshold, guild_id, channel_id) VALUES ($1, $2, $3, $4);",
                    &[emoji, threshold, &guild_id_int, &channel_id_int],
                ).await;

                if result.is_err()  {
                    return String::from("Error creating board!");
                }

                return format!("Created! I will now post with more than {} reactions with the {} emoji, I will post it on <#{}>", threshold, emoji, channel_id_str);
            }
        }
    }

    return "Error!".to_string();
}

async fn add_to_board(message: Message, channel_id: ChannelId, board_id: i64, client: tokio_postgres::Client, http_ctx: Arc<serenity::http::Http>) {
    let message_id_int: i64 = *message.id.as_u64() as i64;

    let mapping = client
        .query("SELECT board_message_id FROM message_mapping WHERE message_id = $1 AND board_id = $2;", &[ &message_id_int, &board_id])
        .await.unwrap();

    if mapping.len() == 0  {
        let mut username =  message.author.tag();

        let nickname = message.author_nick(&http_ctx).await;
        if !nickname.is_none()  {
            username = format!("{} ({})", nickname.unwrap(), message.author.tag());
        }

        let message = format!("{}\n\n---\n**From:** {}", message.content, username);
       
        let result_msg: Result<Message, serenity::Error> = channel_id.send_message(&http_ctx, |m| {
            m.content(message)
        }).await;

        if result_msg.is_err() {
            eprintln!("Error sending board message");
            return;
        }

        let board_message = result_msg.unwrap();
        let board_message_id_int: i64 = *board_message.id.as_u64() as i64;

        
        let result_db = client.execute(
            "INSERT INTO message_mapping (message_id, board_message_id, board_id) VALUES ($1, $2, $3);",
            &[&message_id_int, &board_message_id_int, &board_id],
        ).await;

        if result_db.is_err()  {
            eprintln!("Error: Storing mapping in database!");

            let delete_result = board_message.delete(http_ctx).await;

            if delete_result.is_err() {
                eprintln!("Error: Cannot delete message for board on error");
            }
            return;
        }
    }
}

async fn remove_from_board(message: Message, channel_id: u64, board_id: i64, client: tokio_postgres::Client, http_ctx: Arc<serenity::http::Http>) {
    let message_id_int: i64 = *message.id.as_u64() as i64;

    let result_mapping = client
        .query("SELECT board_message_id FROM message_mapping WHERE message_id = $1 AND board_id = $2;", &[ &message_id_int, &board_id])
        .await;

    if result_mapping.is_err() {
        eprintln!("Error: Fetching mapping from database");
        return;
    }

    let mapping = result_mapping.unwrap();
    let board_message_id: u64 = mapping[0].get::<usize, i64>(0) as u64;

    let result_board_message: Result<Message, SerenityError> = http_ctx.get_message(channel_id, board_message_id).await;

    if result_board_message.is_err()  {
        eprintln!("Error: Getting board message");
        return;
    }

    let board_message = result_board_message.unwrap();

    let result_delete = board_message.delete(http_ctx).await;

    if result_delete.is_err()  {
        eprintln!("Error: Deleting board message");
        return;
    }

    let result_db = client.execute(
       "DELETE FROM message_mapping WHERE message_id = $1 AND board_id = $2;",
       &[ &message_id_int, &board_id],
    ).await;

    if result_db.is_err()  {
        eprintln!("Error: Deleting board message");
    }
}

async fn handle_board_change(ctx: Context, reaction: Reaction, remove: bool) {
    let client = connect_to_db().await;

    let emoji_str: String =  reaction.emoji.to_string();
    let emoji: &str = emoji_str.as_str();
    let guild_id_int: i64 = *reaction.guild_id.unwrap().as_u64() as i64;

    let rows = client
        .query("SELECT id, threshold, channel_id FROM boards WHERE guild_id = $1 AND emoji = $2;", &[ &guild_id_int, &emoji])
        .await.unwrap();

    if rows.len() == 0 {
        return;
    }

    let board_id: i64 = rows[0].get(0);
    let threshold: u64 = rows[0].get::<usize, i64>(1) as u64;
    let channel_id_num: u64 = rows[0].get::<usize, i64>(2) as u64;
    let channel_id: ChannelId = ChannelId(channel_id_num);
    let message: Message = ctx.http.get_message(channel_id_num, *reaction.message_id.as_u64()).await.unwrap();

    for reaction in &message.reactions {
        let http_ctx: Arc<Http> = ctx.borrow().clone().http;

        if reaction.reaction_type.unicode_eq(emoji) && reaction.count >= threshold  {
            add_to_board(message, channel_id, board_id, client, http_ctx).await;
            return;
        }
    }

    if remove  {
       remove_from_board(message, channel_id_num, board_id, client, ctx.http).await;
    }

}

#[async_trait]
impl EventHandler for Bot {

    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        handle_board_change(ctx, reaction, false).await;
    }

    async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        handle_board_change(ctx, reaction, true).await;
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        Command::set_global_application_commands(&ctx.http, |commands| {
            commands.create_application_command(|command| 
		{ 
			command.name("createboard")
			       .description("Creates a board (like Starboard)") 
			       .create_option(|option| {
							option.name("channel")
							      .description("The channel to post messages to")
							      .required(true)
                                  .channel_types(&[ChannelType::Text,])
							      .kind(CommandOptionType::Channel)
						})
                    .create_option(|option| {
							option.name("emoji")
							      .description("The emoji that people react with to get it on the board")
							      .required(true)
							      .kind(CommandOptionType::String)
						})
                    .create_option(|option| {
							option.name("threshold")
							      .description("The number of reactions required to make it onto the board")
							      .required(true)
                                  .min_int_value(1)
							      .kind(CommandOptionType::Integer)
						})

		})
        }).await.unwrap();
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let response_content = match command.data.name.as_str() {
                "createboard" => create_board(&command).await,
                _command => String::from("Error"),
            };

            let create_interaction_response =
                command.create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(response_content))
                });

            if let Err(why) = create_interaction_response.await {
                eprintln!("Cannot respond to slash command: {}", why);
            }
        }
    }
}




