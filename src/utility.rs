use serenity::{
    all::{
        ChannelId, CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
        CreateWebhook, Embed, EmbedField, ExecuteWebhook, Http, Message,
    },
    Error,
};
macro_rules! copy_embed_field {
    ($src:expr, $dst:expr, $field:ident) => {
        if let Some(value) = &$src.$field {
            $dst = $dst.$field(value.clone());
        }
    };
}

fn convert_embed(embed: &Embed) -> CreateEmbed {
    let mut new_embed = CreateEmbed::default();

    copy_embed_field!(embed, new_embed, title);
    copy_embed_field!(embed, new_embed, description);
    copy_embed_field!(embed, new_embed, url);
    copy_embed_field!(embed, new_embed, timestamp);

    if let Some(colour) = embed.colour {
        new_embed = new_embed.colour(colour);
    }

    if let Some(author) = &embed.author {
        let mut auth = CreateEmbedAuthor::new(&author.name);
        if let Some(url) = &author.url {
            auth = auth.url(url);
        }
        if let Some(icon_url) = &author.icon_url {
            auth = auth.icon_url(icon_url);
        }

        new_embed = new_embed.author(auth);
    }

    if let Some(footer) = &embed.footer {
        let mut new_footer = CreateEmbedFooter::new(&footer.text);
        if let Some(icon_url) = &footer.icon_url {
            new_footer = new_footer.icon_url(icon_url);
        }
        new_embed = new_embed.footer(new_footer);
    }

    if let Some(image) = &embed.image {
        new_embed = new_embed.image(&image.url);
    }

    if let Some(thumbnail) = &embed.thumbnail {
        new_embed = new_embed.thumbnail(&thumbnail.url);
    }

    for EmbedField {
        name,
        value,
        inline,
        ..
    } in &embed.fields
    {
        new_embed = new_embed.field(name, value, *inline);
    }

    new_embed
}

pub async fn forward_message_as_webhook(
    http: &Http,
    original_message: &Message,
    target_channel_id: &ChannelId,
) -> serenity::Result<()> {
    let webhooks = target_channel_id.webhooks(http).await?;

    let webhook = if let Some(w) = webhooks.iter().find(|w| w.token.is_some()).cloned() {
        w
    } else {
        target_channel_id
            .create_webhook(http, CreateWebhook::new("Forwarder"))
            .await
            .expect("Failed to create webhook")
    };
    let channel = original_message.channel_id.to_channel(http).await?;
    let guild_id = match channel.guild() {
        Some(guild_channel) => guild_channel.guild_id,
        None => {
            // Return an error if channel is not a guild channel
            return Err(Error::Other("Expected guild channel but found none"));
        }
    };
    let exec = ExecuteWebhook::new()
        .username(&original_message.author.name)
        .avatar_url(
            original_message
                .author
                .avatar_url()
                .unwrap_or_else(|| original_message.author.default_avatar_url()),
        )
        .content(format!(
            "{}\n[Learn More →](https://discord.com/channels/{}/{}/{})",
            &original_message.content,
            &guild_id,
            &original_message.channel_id.get(),
            &original_message.id.get(),
        ));
    println!(
        "{} {} {}",
        &guild_id,
        &original_message.channel_id.get(),
        &original_message.id.get(),
    );
    let embeds: Vec<CreateEmbed> = original_message
        .embeds
        .iter()
        .cloned()
        .map(|a| convert_embed(&a))
        .collect();

    let mut exec = exec.embeds(embeds);

    for attachment in &original_message.attachments {
        if let Ok(bytes) = reqwest::get(&attachment.url).await.unwrap().bytes().await {
            exec = exec.add_file(CreateAttachment::bytes(bytes, attachment.filename.clone()));
        }
    }
    webhook.execute(http, false, exec).await.inspect_err(|e| {
        println!("Failed to send forward");
    })?;

    Ok(())
}
