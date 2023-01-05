extern crate pest;
#[macro_use]
extern crate pest_derive;
mod acf;

use acf::parser;
use acf::parser::AcfValue;

use clap::Parser as ClapParser;
use rand::{thread_rng, Rng};
use rayon::prelude::*;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Add;
use std::process::exit;
use std::time::{Duration, SystemTime};
use std::{env, fs};

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

    // HACK for changelogs, written manually in a different call
    #[serde(skip_deserializing)]
    changelog: String,
}
///

/// Discord webhook formatting
#[derive(Serialize, Debug)]
struct DiscordWebhook<'a> {
    username: &'a str,
    avatar_url: &'a str,
    content: &'a str,
    embeds: Vec<DiscordEmbeds>,
}

#[derive(Serialize, Debug)]
struct DiscordEmbeds {
    title: String,
    description: String,
    color: String,
    thumbnail: DiscordEmbedThumbnail,
    url: String,
}

#[derive(Serialize, Debug)]
struct DiscordEmbedThumbnail {
    url: String,
}
///

/// Arguments
#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, env("STEAM_WEB_KEY"), help("Steam web api key to do calls with"))]
    steam_key: String,

    #[arg(long, env("DISCORD_WEB_KEY"), requires("user_agent"), help("Pass in a discord webhook to enable discord support, you must also set header_info. Only pass in everything after 'https://discord.com/api/webhooks/'"))]
    discord_key: Option<String>,

    #[arg(long, env("ACF"), help("Points to a workshop .acf file"))]
    acf_file: String,

    #[arg(long, env("USER_AGENT"), help("Required if using discord for changelog support, format it such as `Wrench - email@email.com` so people can reach out if need be"))]
    user_agent: Option<String>,
}
///

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Try set color_eyre up, otherwise we dont care if it errors,
    // not worth cluttering up the terminal
    let _ = color_eyre::install();

    let args = Args::parse();

    let acf_file = fs::read_to_string(args.acf_file).unwrap();
    let acf: AcfValue =
        parser::file_to_acf(&acf_file).expect("Could not parse the acf file provided");

    let workshop: Option<HashMap<&str, u64>> = match acf {
        AcfValue::Collection(_, body) => Some(parser::extract_workshop(body)),
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

    let url = format!("https://api.steampowered.com/IPublishedFileService/GetDetails/v1/?key={}{}&short_description=true&includeforsaledata=false&includemetadata=false&appid=108600&strip_description_bbcode=true", args.steam_key, all_workshops);

    let client = reqwest::ClientBuilder::new().gzip(true);
    let client = client
        .build()
        .expect("Could not build reqwest client, this is bad");

    let res: SteamApiResponse = client.get(url).send().await?.json().await?;

    let mut update_list: Vec<WorkshopContent> = res
        .response
        .publishedfiledetails
        .into_par_iter()
        .filter(|details| {
            let timestamp = workshop.get(details.publishedfileid.as_str());

            if timestamp.is_none() {
                return false;
            }

            let timestamp = timestamp.unwrap();

            if &details.time_updated > timestamp {
                println!("{:?} needs updating!", details.title);
                return true;
            }

            false
        })
        .collect();

    if update_list.is_empty() {
        exit(1);
    }

    // TODO: Allow people to change the url and username
    if let Some(key) = args.discord_key {
        let mut rng = thread_rng();
        let time = SystemTime::now()
            .add(Duration::from_secs(60 * 5))
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let content = format!("<bzzt> The server is due to restart at <t:{:?}>, the following mods have been updated: <kssht>", time);

        let header = &args.user_agent.unwrap_or_default();
        for mut update in update_list.iter_mut() {
            update.changelog = fetch_changelog(&update.publishedfileid, header)
                .await
                .unwrap_or_else(|| "No change log could be fetched".to_string());
        }

        let embeds = update_list
            .into_iter()
            .map(|update_list| DiscordEmbeds {
                title: update_list.title,
                color: {
                    let n: u16 = rng.gen();
                    n.to_string()
                },
                thumbnail: DiscordEmbedThumbnail {
                    url: update_list.preview_url,
                },
                url: format!(
                    "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
                    update_list.publishedfileid
                ),
                description: update_list.changelog,
            })
            .collect();

        let msg: DiscordWebhook = DiscordWebhook {
            avatar_url: "https://i.imgur.com/mXpcHBX.png",
            username: "Server maid",
            content: content.as_str(),
            embeds,
        };

        let client = reqwest::Client::new();
        client
            .post(format!("https://discord.com/api/webhooks/{key}"))
            .json(&msg)
            .send()
            .await?;
    }

    Ok(())
}

async fn fetch_changelog(id: &String, header: &String) -> Option<String> {
    let client = reqwest::ClientBuilder::new();
    let client = client.gzip(true).build();

    if let Err(why) = client {
        println!("Could not build reqwest client for changelogs, {why}");
        return None;
    }

    let req = client
        .unwrap()
        .get(format!(
            "https://steamcommunity.com/sharedfiles/filedetails/changelog/{id}"
        ))
        .header(USER_AGENT, header)
        .send()
        .await;

    if let Err(why) = req {
        println!("Could not do changelog request, {why}");
        return None;
    }

    let text = req
        .unwrap()
        .text()
        .await
        .expect("Could not convert text to utf8");

    let text = text.as_str();

    if text.len() > u32::MAX as usize {
        println!(
            "Text body bigger then {}, body was {}. Can not be parsed by tl.",
            u32::MAX,
            text
        );
        return None;
    }

    let dom = tl::parse(text, tl::ParserOptions::default()).unwrap();

    let parser = dom.parser();
    let latest_cl = dom
        .query_selector(".workshopAnnouncement")
        .and_then(|mut iter| iter.next());

    if latest_cl.is_none() {
        println!(
            "Could not find .workshopAnnouncement for id {id}, most likely no change log provided"
        );
        return None;
    }

    let content = latest_cl.unwrap().get(parser).unwrap();
    let p = content
        .as_tag()
        .unwrap()
        .query_selector(parser, "p")
        .and_then(|mut iter| iter.next());

    if p.is_none() {
        println!("Could not find p tag for id {id}, most likely no change log provided");
        return None;
    }

    let text = p.unwrap().get(parser).unwrap().inner_html(parser);

    if text.is_empty() {
        println!("Mod {id} has no changelog, p tag is empty");
        return None;
    }

    Some(text.replace("<br></br>", "\n"))
}
