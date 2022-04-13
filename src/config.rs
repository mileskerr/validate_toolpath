use regex::Regex;
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
                $( $("\n\r//",$desc,)?"\n\r//default: ",stringify!($default),"\n\r",$name," = ",stringify!($default),"\n\n",)*
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
    let (mut config_items,default_config) = config_items!(
        ("MACHINE_SIZE_X", 24.0, "width of machine area in inches"),
        ("MACHINE_SIZE_Y", 48.0, "depth of machine area in inches"),
        ("MIN_PASSES", 2.0, "warning if there are less than or equal to this number of passes"),
        ("MAX_PASSES", 6.0, "warning if there are greater than or equal to this number of passes"),
        ("DEPTH_THRESHOLD", 0.0625, "the maximum amount the endmill should be allowed to cut into the table"),
        ("MIN_OFFSET", -0.2, "fail offset check if southwest part corner is further southwest than this"),
        ("MAX_OFFSET", 0.75, "fail offset check if southwest part corner is further northeast than this"),
        ("WARN_SAFE_HEIGHT", 0.15, "warning if min traversal height is lower than this"),
        ("FAIL_SAFE_HEIGHT", 0.1, "failure if min traversal height is lower than this"),
        ("PASS_FREQUENCY_THRESHOLD", 20.0, "require at least this many lines of g-code in each pass"),
    );

    if let Ok(contents) = fs::read_to_string("./config.txt") {
        for l in contents.lines() {
            if !(l.starts_with("//") || l.starts_with("#")) {
                l.split_terminator(&[' ', '=', ':'][..]).nth(0).map(|k| {
                    let k = k.trim();
                    let value = config_items.get_mut(k);
                    if let Some(v) = value {
                        if let Some(new) = NUM_RE.find(l) {
                            *v = new.as_str().parse::<f32>().unwrap();
                        }
                    }
                });
            }
        }
    } else {
        eprintln!("{}","Warning: no config file found, creating default config");
        create_config(default_config.to_string()).unwrap_or(());
    }
    config_items
}

fn create_config(default: String) -> std::io::Result<()> {
    let mut file = File::create("config.txt")?;
    file.write_all(default.as_bytes())?;
    Ok(())
}
