extern crate pest;
#[macro_use]

extern crate pest_derive;

use acf::parser;

use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{env, fs};
use std::ops::Add;
use std::process::exit;
use std::time::{Duration, SystemTime};


/// Steam Api mapping
#[derive(Deserialize, Debug)]
struct SteamApiResponse {
    response: Response,
}

#[derive(Deserialize, Debug)]
struct Response {
    publishedfiledetails: Vec<WorkshopContent>,
}

#[derive(Deserialize, Debug)]
struct WorkshopContent {
    title: String,
    publishedfileid: String,
    time_updated: u64,
    preview_url: String,
}
///

/// Discord webhook formatting
#[derive(Serialize, Debug)]
struct DiscordWebhook<'a> {
    username: &'a str,
    avatar_url: &'a str,
    content: &'a str,
    embeds: Vec<DiscordEmbeds>
}

#[derive(Serialize, Debug)]
struct DiscordEmbeds {
    title: String,
    // description: String,
    color: String,
    thumbnail: DiscordEmbedThumbnail,
    url: String,
}

#[derive(Serialize, Debug)]
struct DiscordEmbedThumbnail {
    url: String,
}
///




#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Required variables
    let key = env::var("STEAM_WEB_KEY").expect("Missing steam web api key");
    let path = env::var("ACF").expect("Missing ACF file path");

    // Optional
    let discord_key = env::var("DISCORD_WEB_HOOK");

    let acf_file = fs::read_to_string(path).unwrap();
    let acf: AcfValue = parse_acf_file(&acf_file).expect("Could not parse the acf file provided");

    let workshop: Option<HashMap<&str, u64>> = match acf {
       AcfValue::Collection(_, body) => {
            Some(workshops_from_acf(body))
        },
        _ => None,
    };

    if workshop.is_none() {
        println!("ACF file does not contain any workshop content..");
        exit(1);
    }

    let workshop = workshop.unwrap();

    let all_workshops = {
        let mut total = String::new();
        for (pos, map) in workshop.iter().enumerate() {
            total.push_str(format!("&publishedfileids%5B{}%5D={}", pos, map.0).as_str())
        }

        total
    };

    let url = format!("https://api.steampowered.com/IPublishedFileService/GetDetails/v1/?key={}{}&short_description=true&includeforsaledata=false&includemetadata=false&appid=108600&strip_description_bbcode=true", key, all_workshops );

    let res: SteamApiResponse = reqwest::get(url)
        .await?
        .json()
        .await?;


    let mut update_list:  Vec<WorkshopContent> = Vec::new();

    res.response.publishedfiledetails.into_iter().for_each(|details | {
        let timestamp = workshop.get(details.publishedfileid.as_str());

        if let None = timestamp {
            return
        }

        let timestamp = timestamp.unwrap();

        if &details.time_updated > timestamp {
            println!("{:?} needs updating!", details.title);

            update_list.push(details);
        }
    });

    if update_list.is_empty() {
        exit(1);
    }

    // TODO: Allow people to change the url and username
    if let Ok(key) = discord_key {
        let mut rng = thread_rng();
        let time = SystemTime::now().add(Duration::from_secs(60 * 5)).duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        let content = format!("<bzzt> The server is due to restart at <t:{:?}>, the following mods have been updated: <kssht>", time);

        let embeds: Vec<DiscordEmbeds> = update_list.into_iter().map(|update_list| {
            DiscordEmbeds{
                title: update_list.title,
                color: {
                    let n: u16 = rng.gen();
                    n.to_string()
                },
                thumbnail: DiscordEmbedThumbnail{ url: update_list.preview_url },
                url: format!("https://steamcommunity.com/sharedfiles/filedetails/?id={}", update_list.publishedfileid)
            }
        }).collect();

        let msg: DiscordWebhook = DiscordWebhook {
            avatar_url: "https://i.imgur.com/mXpcHBX.png",
            username: "Server maid",
            content: content.as_str(),
            embeds,
        };

        let client = reqwest::Client::new();
        client.post(format!("https://discord.com/api/webhooks/{}", key))
            .json(&msg)
            .send()
            .await?;
    }

    Ok(())
}