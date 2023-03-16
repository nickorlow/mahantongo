use std::borrow::Borrow;
use std::env;

use serenity::async_trait;
use serenity::model::prelude::MessageReaction;
use serenity::prelude::*;
use serenity::model::gateway::Ready;
use serenity::model::channel::{Message, ChannelType, Reaction, ReactionType};
use serenity::model::id::ChannelId;
use serenity::http::Http;
use serenity::Error;
use std::sync::Arc;
use serenity::model::interactions::application_command::{ApplicationCommand, ApplicationCommandOptionType, ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue};
use serenity::model::application::interaction::{Interaction, InteractionResponseType};

struct Bot;

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Bot)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

pub fn create_board(command: &ApplicationCommandInteraction) -> String {
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

    if let  ApplicationCommandInteractionDataOptionValue::Channel(channel) = channel_dov {
        if let  ApplicationCommandInteractionDataOptionValue::String(emoji) = emoji_dov {
            if let  ApplicationCommandInteractionDataOptionValue::Integer(threshold)  = threshold_dov {

                let channel_id: String = channel.id.to_string();
            
                return format!("Created! I will now post with more than {} reactions with the {} emoji, I will post it on <#{}>", threshold, emoji, channel_id);

            }
        }
    }

    return "Error!".to_string();
}

#[async_trait]
impl EventHandler for Bot {

    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        // should pull from database
        let threshold: u64 = 1;
        let emoji: &str =  "😊";
        let channel_id_num: u64 = 862894859571298306;

        if(!reaction.emoji.unicode_eq(emoji)) { 
            return;
        }

        let channel_id: ChannelId = ChannelId(channel_id_num);
        let message: Message = ctx.http.get_message(channel_id_num, *reaction.message_id.as_u64()).await.unwrap();
        
        for reaction in message.reactions {
            let http_ctx: Arc<Http> = ctx.borrow().clone().http;
            println!("{},{}",reaction.reaction_type, reaction.count);

            if(reaction.reaction_type.unicode_eq(emoji) && reaction.count >= threshold) {
                let result: Result<Message, Error> = channel_id.send_message(http_ctx, |m| {
                    m.content("Made the board: \n" )
                }).await;

                if(result.is_err()) {
                    println!("Error sending board message");
                }

                break;
            }
        }
    }

    // async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
    //     
    // }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let commands = ApplicationCommand::set_global_application_commands(&ctx.http, |commands| {
            commands.create_application_command(|command| 
		{ 
			command.name("createboard")
			       .description("Creates a board (like Starboard)") 
			       .create_option(|option| {
							option.name("channel")
							      .description("The channel to post messages to")
							      .required(true)
                                  .channel_types(&[ChannelType::Text,])
							      .kind(ApplicationCommandOptionType::Channel)
						})
                    .create_option(|option| {
							option.name("emoji")
							      .description("The emoji that people react with to get it on the board")
							      .required(true)
							      .kind(ApplicationCommandOptionType::String)
						})
                    .create_option(|option| {
							option.name("threshold")
							      .description("The number of reactions required to make it onto the board")
							      .required(true)
                                  .min_int_value(1)
							      .kind(ApplicationCommandOptionType::Integer)
						})

		})
        }).await.unwrap();
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let response_content = match command.data.name.as_str() {
                "createboard" => create_board(&command),
                command => "Error".to_owned(),
            };

            let create_interaction_response =
                command.create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(response_content))
                });

            if let Err(why) = create_interaction_response.await {
                println!("Cannot respond to slash command: {}", why);
            }
        }
    }
}




