use rand::{thread_rng, Rng};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{env, fs};
use std::ops::Add;
use std::process::exit;
use std::time::Duration;
use pest::Parser;
use pest::error::Error;


#[derive(Parser)]
#[grammar = "./acf/acf.pest"]
pub struct AcfParser;

#[derive(Debug, Clone)]
pub enum AcfValue<'a> {
    Pair(&'a str, &'a str),
    Collection(&'a str, Vec<AcfValue<'a>>),
    Null
}

// Read's a file and attempts to parse it into an AcfValue
pub fn file_to_acf(file: &str) -> Result<AcfValue, Error<Rule>> {
    // Just panic here if the file is invalid
    let acf = AcfParser::parse(Rule::collection, file)?.next().expect("Acf file provided could not be read");

    use pest::iterators::Pair;
    fn parse_value(pair: Pair<Rule>) -> AcfValue {
        match pair.as_rule() {
            Rule::pair => {
                let mut pair = pair.into_inner();
                let name = pair.next().unwrap().as_str().trim_matches('\"');
                let result = pair.next().unwrap().as_str().trim_matches('\"');


                AcfValue::Pair(
                    name,
                    result,
                )
            },
            Rule::collection => {
                let mut inner_rules = pair.into_inner();
                let name = inner_rules.next().unwrap().as_str().trim_matches('\"');

                let contents = inner_rules
                    .map(|rule| {
                        parse_value(rule)
                    })
                    .collect();

                AcfValue::Collection(
                    name,
                    contents
                )
            },
            _ => AcfValue::Null
        }
    }

    Ok(parse_value(acf))
}

// Extracts workshop mods from an acf file
// Return's an empty if no workshop is found.
pub fn extract_workshop(content: Vec<AcfValue>) -> HashMap<&str, u64> {
    let mut workshops = HashMap::new();

    content.into_iter().for_each(|insides| {
        if let AcfValue::Collection(name, body) = insides {
            if name != "WorkshopItemsInstalled" {
                return;
            }

            body.into_iter().for_each(|workshop| {
                if let AcfValue::Collection(name, metadata) = workshop {
                    let result = metadata.into_iter().find(|pair| {
                        if let AcfValue::Pair(name, _) = pair {
                            if name == &"timeupdated" {
                                return true;
                            }
                        }

                        false
                    });

                    if let Some(AcfValue::Pair(_, value)) = result {
                        workshops.insert(
                            name,
                            value.parse::<u64>().unwrap()
                        );
                    }


                }
            })
        }
    });

    workshops
}
