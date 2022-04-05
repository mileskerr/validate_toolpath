use regex::Regex;
use owo_colors::OwoColorize;
use std::fs;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;

lazy_static! {
    static ref NUM_RE: Regex = Regex::new(r"-?[0-9]+\.?[0-9]*").unwrap();
}

macro_rules! config_items {
    ( $( ($name:literal, $default:literal $(,$desc:literal)?), )* ) => {{
        lazy_static! {
            static ref DEFAULT_CONFIG: String = [
                $( $("\n//",$desc,)?"\n//default: ",stringify!($default),"\n",$name," = ",stringify!($default),"\n\n",)*
            ].concat();
        }
        (
            HashMap::from([
                $(($name.into(),$default),)*
            ]),
            &DEFAULT_CONFIG
        )
    }}
}

pub fn read_config() -> HashMap<String,f32> {
    let (mut config_items,DEFAULT_CONFIG) = config_items!(
        ("PASS_FREQUENCY_THRESHOLD", 80.0, "require at least this many lines of g-code in each pass"),
        ("MIN_PASSES", 2.0, "warning if there are less than or equal to this number of passes"),
        ("MAX_PASSES", 10.0, "consider the program to be in error if it finds more passes than this"),
        ("DEPTH_THRESHOLD", 0.0625, "the maximum amount the endmill should be allowed to cut into the table"),
        ("MIN_OFFSET", -0.1, "fail offset check if southwest part corner is further southwest than this"),
        ("MAX_OFFSET", 0.75, "fail offset check if southwest part corner is further northeast than this"),
        ("WARN_SAFE_HEIGHT", 0.75, "warning if min traversal height is lower than this"),
        ("FAIL_SAFE_HEIGHT", 0.75, "failure if min traversal height is lower than this"),
    );

    if let Ok(contents) = fs::read_to_string("./config.txt") {
        for l in contents.lines() {
            if !(l.starts_with("//") || l.starts_with("#")) {
                l.split_terminator(&[' ', '=', ':'][..]).nth(0).map(|k| {
                    let value = config_items.get_mut(k);
                    if let Some(v) = value {
                        if let Some(new) = NUM_RE.find(l) {
                            //println!("{}, {}",k,v);
                            *v = new.as_str().parse::<f32>().unwrap();
                        }
                    }
                });
            }
        }
    } else {
        eprintln!("{}","Warning: no config file found, creating default config".yellow());
        create_config(DEFAULT_CONFIG.to_string());
    }
    config_items
}


fn create_config(default: String) -> std::io::Result<()> {
    let mut file = File::create("config.txt")?;
    file.write_all(default.as_bytes())?;
    Ok(())
}
