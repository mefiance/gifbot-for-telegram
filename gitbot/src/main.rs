
use std::env;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use teloxide::prelude::*;
use teloxide::types::{InlineQueryResult, InlineQueryResultGif };


#[derive(Deserialize)]
struct KlipyItem {
    #[serde(default)]
    file: Value,
}
#[derive(Deserialize, Default)]
struct KlipyData {
    #[serde(default)]
    data: Vec<KlipyItem>
}
#[derive(Deserialize)]
struct KlipyEnvelope {
    #[serde(default)]
    data: KlipyData,
}

const TIER_ORDER_FULL: [&str; 4] = ["hd", "md", "sm", "xs"];
const TIER_ORDER_PREVIEW: [&str; 4] = ["sm", "md", "xs", "hd"];

fn format_gif(file: &Value, tiers: &[&str]) -> Option<(String, Option<u32>, Option<u32>)> {
    for tier in tiers{
        let gif = &file[*tier]["gif"];
        if let Some(url) = gif["url"].as_str() {
            let width = number_field(&gif["width"]);
            let height = number_field(&gif["height"]);
            return Some((url.to_string(), width, height));
        }
    }
    None
}

fn number_field(v: &Value) -> Option<u32> {
    if let Some(n) = v.as_i64() {
        return Some(n as u32)
    }
    v.as_str().and_then(|s| s.parse::<u32>().ok())
}

async fn fetch_gif( client: &Client, klipy_key: &str, query: &str, customer_id: &str ) -> Result<Vec<KlipyItem>, reqwest::Error> {
    let action = if query.is_empty() { "trending" } else { "search" };
    let url = format!("https://api.klipy.com/api/v1/{key}/gifs/{action}", key = klipy_key, action = action
    );
    let mut req = client
        .get(&url)
        .query(&[("per_page", "24"), ("customer_id", customer_id,)]);

    if !query.is_empty() {
        req = req.query(&[("q", query)]);
    }
    let envelope: KlipyEnvelope = req.send().await?.json().await?;
    Ok(envelope.data.data)
}

async fn handle_inline (bot: Bot, q: InlineQuery, client: &Client, klipy_key: &str,) -> ResponseResult<()> {
    let customer_id = q.from.id.to_string();
    let items = fetch_gif(&client, &klipy_key, q.query.trim(), &customer_id).await.unwrap_or_else(|e| {
        log::error!("Klippy doesn't respond {e}");
        Vec::new()
    });

    let results: Vec<InlineQueryResult> = items
        .into_iter()
        .enumerate()
        .filter_map(|(i, item)|{
            let (full_url, w, h) = format_gif(&item.file, &TIER_ORDER_FULL)?;
            let (preview_url, _, _) = format_gif(&item.file, &TIER_ORDER_PREVIEW).unwrap_or((full_url.clone(), w, h));

            let git_url = full_url.parse().ok()?;
            let thumb_url = preview_url.parse().ok()?;

            let mut r = InlineQueryResultGif::new(i.to_string(), git_url, thumb_url);
            r.gif_width = w;
            r.gif_height = h;
            Some(InlineQueryResult::Gif(r))
        })
        .collect();
    bot.answer_inline_query(&q.id, results).cache_time(10).is_personal(true).await?;

    Ok(())
}
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting klipy-bot...");

    let klipy_key = env::var("KLIPY_KEY").unwrap();
    let http = Client::new();
    let bot = Bot::from_env();

    let handler = Update::filter_inline_query().endpoint(handle_inline);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![klipy_key])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}