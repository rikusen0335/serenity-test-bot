use std::fs::File;
use std::io;
use std::{convert::TryInto, env};

use serenity::framework::standard::{Args, CommandResult};
use serenity::framework::standard::macros::command;
use serenity::model::channel;
use songbird::{SerenityInit, ffmpeg};
use serenity::client::Context;
use serenity::{async_trait, framework::{StandardFramework, standard::macros::group}, model::{channel::{ChannelType, Message}, gateway::Ready, guild::Guild, id::{ChannelId, GuildId}, voice::VoiceState}, prelude::*};
struct Handler;

static COMMAND_PREFIX: &str = "r/";

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, message: Message) {
        if let Some(channel) = message.channel_id.to_channel_cached(&ctx.cache).await.unwrap().guild() {
            if channel.name.contains("聞き専")
                && !message.content.starts_with(COMMAND_PREFIX)
                && !message.author.bot
            {
                let url = format!("http://localhost:8080/voice?text={}", message.content);
                let filename = "audio.mp3";
                let response: reqwest::Response = reqwest::get(url).await.unwrap();
                let bytes = response.bytes().await.unwrap();
                let mut out = File::create(filename).unwrap();
                io::copy(&mut bytes.as_ref(), &mut out).unwrap();

                let source = ffmpeg(filename)
                    .await
                    .expect("This might fail: handle this error!");

                let manager = songbird::get(&ctx).await
                    .expect("Songbird Voice client placed in at initialisation.").clone();



                if let Some(handler_lock) = manager.get(message.guild_id.unwrap()) {
                    let mut handler = handler_lock.lock().await;

                    handler.play_source(source);

                    println!("「{}」を再生中", message.content);
                } else {
                    println!("「{}」を再生できませんでした", message.content);
                }
            }
        }
    }

    async fn voice_state_update(&self, ctx: Context, guild_id: Option<GuildId>, old_state: Option<VoiceState>, new_state: VoiceState) {
        if let Some(old) = &old_state {
            if let Some(id) = &guild_id {
                delete_unused_voice_channel(&ctx, id, old).await;
            }
        }

        // Create a channel if joined channel is create button
        if let Some(guild_id) = &guild_id {
            create_new_voice_channel(&ctx, guild_id, &new_state).await;
        }
    }

    // Set a handler to be called on the `ready` event. This is called when a
    // shard is booted, and a READY payload is sent by Discord. This payload
    // contains data like the current user's guild Ids, current user data,
    // private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

// Create a new channel and bring user into it
async fn create_new_voice_channel(ctx: &Context, guild_id: &GuildId, state: &VoiceState) {
    if let Some(channel_join_id) = state.channel_id {
        let channel_join = channel_join_id.to_channel(&ctx.http).await.unwrap().guild().unwrap();

        // If channel name is specific one then create chann and move member into that
        if channel_join.name() == "test" {
            if let Some(guild) = &ctx.cache.guild(guild_id).await {
                let new_channel = guild
                    .create_channel(&ctx.http,
                        |c|
                        c
                        .name("my-test-channel")
                        .kind(ChannelType::Voice)
                        .position(channel_join.position.try_into().unwrap())).await.unwrap();

                if let Some(member) = &state.member {
                    member.move_to_voice_channel(&ctx.http, new_channel).await.unwrap();
                }
            }
        }
    }
}

// Delete voice channel if the channel is unused for this time.
async fn delete_unused_voice_channel(ctx: &Context, guild_id: &GuildId, state: &VoiceState) {
    let channel_left = state.channel_id.unwrap();

    if let Some(guild) = ctx.cache.guild(guild_id).await {
        let member_count = count_member(&guild, channel_left);

        if let Some(name) = channel_left.name(&ctx.cache).await {
            // Delete channel that has no members AND is not a channel to create
            if member_count == 0 && name != "test" {
                match channel_left.delete(&ctx.http).await {
                    Ok(_) => println!("Removed channel named: {} due to no members left", name),
                    Err(why) => println!("{}", why)
                };
            }
        }
    }
}

fn count_member(guild: &Guild, channel_id: ChannelId) -> usize {
    return guild
        .voice_states
        .values()
        .filter(|state| {
            match state.channel_id {
                Some(c) => c == channel_id,
                None => false,
            }
        })
        .filter(|state| {
            match &state.member {
                Some(m) => !m.user.bot,
                None => false,
            }
        })
        .count();
}

#[group]
#[commands(kite, name)]
struct General;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let framework = StandardFramework::new()
        .configure(|c| c.prefix(COMMAND_PREFIX))
        .group(&GENERAL_GROUP);

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let mut client =
        Client::builder(&token)
            .event_handler(Handler)
            .framework(framework)
            .register_songbird()
            .await.expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }

    tokio::spawn(async move {
        let _ = client.start().await.map_err(|why| println!("Client ended: {:?}", why));
    });

    tokio::signal::ctrl_c().await;
    println!("Received Ctrl-C, shutting down.");
}

#[command]
#[only_in(guilds)]
async fn kite(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states.get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            msg.reply(ctx, "先にVC入れアホ").await.unwrap();

            return Ok(());
        }
    };

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    let _handler = manager.join(guild_id, connect_to).await;

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn name(ctx: &Context, message: &Message, mut args: Args) -> CommandResult {
    let new_channel_name = args.single::<String>()?;

    let channel_id = &message
        .guild(&ctx.cache)
        .await.unwrap()
        .voice_states.get(&message.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            &message.reply(ctx, "先にVC入れアホ").await.unwrap();

            return Ok(());
        }
    };

    if let Err(why) = connect_to.edit(&ctx.http, |c| c.name(&new_channel_name)).await {
        println!("Could not change channel name caused by: {}", why);
    }

    &message.reply(&ctx.http, format!("チャンネル名を`{}`に変更しました", &new_channel_name)).await?;

    Ok(())
}
