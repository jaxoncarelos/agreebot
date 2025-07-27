use dotenv::dotenv;
use serenity::all::{
    ChannelId, Context, CreateMessage, EventHandler, GatewayIntents, GuildId, Message, MessageId, MessageReference,
    MessageReferenceKind, Reaction, ReactionType,
};
use serenity::async_trait;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use utility::forward_message_as_webhook;
mod utility;

struct Handler {
    posted_map: Arc<Mutex<HashMap<MessageId, bool>>>,
    channel_id_map: Arc<Mutex<HashMap<GuildId, ChannelId>>>,
    conn: Arc<Mutex<sqlite::Connection>>,
}

const EMOJI_AGREE: u64 = 230782152164245505;
const COUNT_THRESHOLD: u64 = 1;
const MESSAGE_TIME_PASSED_THRESHOLD: u64 = 3;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, new_message: Message) {
        if new_message.author.bot {
            return;
        }
        let guild_id = new_message.guild_id.unwrap();
        let guild = guild_id.to_partial_guild(&ctx).await.unwrap();
        // owner of guild or me the developer
        let is_owner = (guild.owner_id == new_message.author.id)
            || (new_message.author.id == 859472531974520832);

        if is_owner && new_message.content.trim().starts_with(".setchanid") {
            let channel_id = new_message.content.split_whitespace().nth(1).unwrap();
            let channel_id = ChannelId::new(channel_id.parse::<u64>().unwrap());
            let mut channel_id_map = self.channel_id_map.lock().await;
            channel_id_map.insert(guild_id, channel_id);

            let conn = self.conn.lock().await;
            conn.execute(
                format!(
                    "INSERT OR REPLACE INTO channel_id (guild_id, channel_id) VALUES ({}, {})",
                    guild_id.get() as i64,
                    channel_id.get() as i64
                )
                .as_str(),
            )
            .unwrap();

            println!("Channel ID set for guild: {}", guild_id);
        }
    }
    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        if let ReactionType::Custom { ref id, .. } = reaction.emoji {
            if id.get() != EMOJI_AGREE {
                return;
            }
        }
        println!("Valid reaction added!");
        let reaction_count: u64 = reaction
            .message(&ctx)
            .await
            .unwrap()
            .reactions
            .iter()
            .find(|reaction| {
                if let ReactionType::Custom { ref id, .. } = reaction.reaction_type {
                    id.get() == EMOJI_AGREE
                } else {
                    false
                }
            })
            .unwrap()
            .count;
        println!("Count of {}", reaction_count);
        if reaction_count == COUNT_THRESHOLD {
            println!("Hit threshold, forwarding now");
            let message = reaction.message(&ctx).await.unwrap();

            if message
                .timestamp
                .checked_add_days(chrono::naive::Days::new(MESSAGE_TIME_PASSED_THRESHOLD))
                .unwrap()
                < chrono::Utc::now()
            {
                return;
            }

            let mut posted = self.posted_map.lock().await;
            if posted.contains_key(&reaction.message_id) {
                return;
            }
            posted.insert(reaction.message_id, true);
            let channel_id_map = self.channel_id_map.lock().await;
            let channel_id = channel_id_map.get(&reaction.guild_id.unwrap());

            let Some(channel_id) = channel_id else { return };

            if !message.components.is_empty() {
                channel_id
                    .send_message(
                        &ctx.http,
                        CreateMessage::new().reference_message(
                            MessageReference::new(
                                MessageReferenceKind::Forward,
                                message.channel_id,
                            )
                            .message_id(message.id)
                            .guild_id(reaction.guild_id.unwrap())
                            .fail_if_not_exists(true),
                        ),
                    )
                    .await
                    .unwrap();
            } else {
                forward_message_as_webhook(&ctx.http, &message, channel_id)
                    .await
                    .unwrap();
            }
        }
    }
}
#[tokio::main]
async fn main() {
    let connection = sqlite::open("channel_id.db").unwrap();
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS channel_id (
            guild_id INTEGER PRIMARY KEY,
            channel_id INTEGER NOT NULL
        )",
        )
        .unwrap();
    dotenv().ok();
    let token = env::var("TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::MESSAGE_CONTENT;
    let handler = Handler {
        posted_map: Arc::new(Mutex::new(HashMap::new())),
        channel_id_map: Arc::new(Mutex::new(HashMap::new())),
        conn: Arc::new(Mutex::new(connection)),
    };

    let query = "SELECT * FROM channel_id";

    for row in handler
        .conn
        .lock()
        .await
        .prepare(query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
    {
        let guild_id = row.read::<i64, _>(0);
        let channel_id = row.read::<i64, _>(1);
        let guild_id = GuildId::new(guild_id as u64);
        let channel_id = ChannelId::new(channel_id as u64);
        let mut channel_id_map = handler.channel_id_map.lock().await;
        channel_id_map.insert(guild_id, channel_id);
    }

    let mut client = serenity::Client::builder(&token, intents)
        .event_handler(handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
