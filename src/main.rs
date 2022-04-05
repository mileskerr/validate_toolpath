#[macro_use]
extern crate lazy_static;
use regex::Regex;
use owo_colors::OwoColorize;
use std::ops;
use std::fmt;
use std::env;
use std::fs;
use std::path;
use std::collections::HashMap;
use native_dialog::{FileDialog};

mod config;

fn main() {
    println!("Validate Toolpath v1.0");
    println!("Utility to prevent stupid toolpath mistakes");
    println!("https://github.com/mileskerr/validate_toolpath");

    let config_items = config::read_config();

    println!("---");
    println!("Please select a file...");

    let path = match get_path() {
        Ok(path) => { path }
        Err(error) => { eprintln!("Error: {}",error.red()); return; }
    };
    let contents = match get_file(path.clone()) {
        Ok(file) => { file }
        Err(error) => { eprintln!("Error: {}",error.red()); return; }
    };

    println!("Validating file \'{}\'...",path.display());
    let results = check(&contents,&config_items);

    let passed: Vec<Outcome> = results.clone().into_iter().filter(|r| r.status == Status::Pass).collect();
    let failed: Vec<Outcome> = results.clone().into_iter().filter(|r| r.status == Status::Fail).collect();
    let warnings: Vec<Outcome> = results.clone().into_iter().filter(|r| r.status == Status::Warning).collect();
    let errors: Vec<Outcome> = results.clone().into_iter().filter(|r| r.status == Status::Error).collect();

    let status = if (failed.len() + warnings.len() + errors.len()) == 0 {
        format!("{}","SUCCESS! All checks passed".green())
    } else {
        let warning_s = if warnings.len() == 1 { "warning" } else { "warnings" };
        let error_s = if errors.len() == 1 { "error" } else { "errors" };
        format!("COMPLETE: {} passed, {} failed, {} {warning_s}, {} {error_s} ", passed.len(), failed.len(), warnings.len(), errors.len())
    };
    println!("---");
    println!("{}",status.bold());
    println!("{}","press Ctrl-C to exit");
    println!("");
    for result in failed {
        println!("{}",result);
    };
    for result in warnings {
        println!("{}",result);
    };
    for result in errors {
        println!("{}",result);
    };
    for result in passed {
        println!("{}",result);
    };
    println!("");
    loop {}
}

fn check(contents: &String, config_items: &HashMap<String,f32>) -> Vec<Outcome> {
    let mut tool = Tool::Unknown;
    let mut min = Point::empty();
    let mut cut_min = Point::empty();
    let mut traverse_min: f32 = f32::MAX;
    let mut material_size = Point::empty();
    let mut heights: HashMap<i32,usize> = HashMap::new();

    for line in contents.lines() {
        if line.find("G0").is_some() || line.find("G1").is_some() { //moving
            let point = Point::from_str(line);
            min = min.min(point);
            if point.z.is_some() && material_size.z.is_some() { //has z coordinate
                let height = point.z.unwrap();
                let thickness = material_size.z.unwrap();
                if height < thickness { //cutting
                    cut_min = cut_min.min(point);
                    if tool == Tool::Endmill {
                        let height_int = (height * 1000.0) as i32;
                        let count = heights.get_mut(&height_int);
                        if let Some(t) = count {
                            *t += 1
                        } else {
                            heights.insert(height_int,1);
                        }
                    }
                }
                if height >= thickness {
                    traverse_min = (height-thickness).min(traverse_min);
                }
            }
        }
        else if line.find("(").is_some() && line.find(")").is_some() {
            let point = Point::from_str(line);
            if material_size.is_empty() && !point.is_empty() {
                material_size = point;
            }
            if line.find("Tool: Drill").is_some() {
                tool = Tool::Drill;
            } else if line.find("Tool: End Mill").is_some() {
                tool = Tool::Endmill;
            }
        }
    }


   
    vec![
        check_safe_height(traverse_min,
            *config_items.get("WARN_SAFE_HEIGHT").unwrap(),
            *config_items.get("FAIL_SAFE_HEIGHT").unwrap(),
        ),
        check_depth(min,material_size,
            *config_items.get("DEPTH_THRESHOLD").unwrap(),
        ),
        check_offset(cut_min,
            *config_items.get("MIN_OFFSET").unwrap(),
            *config_items.get("MAX_OFFSET").unwrap(),
        ),
        check_passes(heights,
            *config_items.get("MIN_PASSES").unwrap() as usize,
            *config_items.get("MAX_PASSES").unwrap() as usize,
            *config_items.get("PASS_FREQUENCY_THRESHOLD").unwrap() as usize,
        ),
    ]
}

#[derive(PartialEq,Clone)]
enum Tool {
    Drill,
    Endmill,
    Unknown,
}

fn check_safe_height(traverse_min: f32, warn_safe_height: f32, fail_safe_height: f32) -> Outcome {
    let out = Outcome::new("Min Safe Height");
    if traverse_min <= fail_safe_height {
        return out.set(Status::Fail,
            format!("tool is in danger of colliding with screws:\nminimum traversing height detected: {}",traverse_min)
        );
    } else if traverse_min <= warn_safe_height {
        return out.set(Status::Warning,
            format!("tool may collide with screws:\nminimum traversing height detected: {}",traverse_min)
        );
    } else if traverse_min == f32::MAX {
        return out.set(Status::Error,
            format!("could not detect minimum traversing height")
        );
    } else {
        return out.set(Status::Pass,
            format!("tool is not in danger of colliding with screws:\nminimum traversing height detected: {}",traverse_min)
        );
    }
}


fn check_passes(heights: HashMap<i32,usize>, min_passes: usize, max_passes: usize, pass_freq_threshold: usize) -> Outcome {
    let out = Outcome::new("Number of Passes");
    let mut passes = 0;
    for (_,freq) in heights {
        if freq > pass_freq_threshold {
            passes += 1;
        }
    }
    if (1..=min_passes).contains(&passes) {
        return out.set(Status::Warning,
            format!("only {} passes detected",passes)
        );
    } else if (min_passes..max_passes).contains(&passes) {
        return out.set(Status::Pass,
            format!("{} passes detected",passes)
        );
    } else {
        return out.set(Status::Error,
            format!("unable to detect number of passes")
        );
    }

}

fn check_depth(min: Point, material_size: Point, depth_threshold: f32) -> Outcome {
    let out = Outcome::new("Depth");
    if material_size.z.is_some() {
        let thickness = material_size.z.unwrap();
        if min.z.is_some() {
            let max_depth = thickness - min.z.unwrap();
            if max_depth > thickness + depth_threshold {
                return out.set(Status::Fail,
                    format!("may cut too deep:\nmaterial thickness: {}\nmax cut depth: {}",thickness,max_depth)
                );
            } else if max_depth < thickness {
                return out.set(Status::Fail,
                    format!("may not cut through material:\nmaterial thickness: {}\nmax cut depth: {}",thickness,max_depth)
                );
            } else {
                return out.set(Status::Pass,
                    format!("material thickness: {}, max cut depth: {}",thickness,max_depth)
                );
            }
        }
    }
    return out.set(Status::Error,
        "unable to check depth. This is may be a bug".into()
    );
}
fn check_offset(min: Point, min_offset: f32, max_offset: f32) -> Outcome {
    let out = Outcome::new("Offset");
    if min.x.is_some() && min.y.is_some() {
        for i in 0..2 {
            if min[i].unwrap() > max_offset {
                return out.set(Status::Fail,
                    format!("toolpath may be offset:\nsouthwest corner of part is far from the origin, at ({}, {})",min.x.unwrap(),min.y.unwrap())
                );
            }
            if min[i].unwrap() < min_offset {
                return out.set(Status::Fail,
                    format!("toolpath may be offset:\nsouthwest corner of part is negative, at ({}, {})",min.x.unwrap(),min.y.unwrap())
                );
            }
        }
        return out.set(Status::Pass,
            format!("southeast corner of part is near the origin, at ({}, {})",min.x.unwrap(), min.y.unwrap())
        );
    }
    return out.set(Status::Error,
        "unable to check offset. This is may be a bug".into()
    );
}

#[derive(Clone)]
struct Outcome {
    name: String,
    message: String,
    status: Status,
}
impl Outcome {
    fn new(name: &str) -> Outcome {
        Outcome {
            name: name.into(),
            message: "if you are reading this, there is a bug in the code.".into(),
            status: Status::Error,
        }
    }
    fn new_full(name: &str, status: Status, message: String) -> Outcome {
        Outcome {
            name: name.into(),
            status: status,
            message: message,
        }
    }
    fn set(mut self, status: Status, message: String) -> Outcome {
        self.status = status;
        self.message = message;
        self
    }
}
impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut message = self.message.clone();
        message.insert_str(0," > ");
        message = message.replace('\n', "\n    ");
        match self.status {
            Status::Pass => {
                write!(f, "[{}] {}:\n{}", self.status, self.name, message.cyan())
            }
            _=> {
                write!(f, "[{}] {}:\n{}", self.status, self.name, message.red())
            }
        }
    }
}

#[derive(PartialEq, Clone)]
enum Status {
    Pass,
    Fail,
    Warning,
    Error,
}
impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Pass => write!(f,"{}","PASS".green().bold()),
            Status::Fail => write!(f,"{}","FAIL".red().bold()),
            Status::Warning => write!(f,"{}","WARNING".yellow().bold()),
            Status::Error => write!(f,"{}","ERROR".red().bold()),
        }
    }
}
fn get_file(path: path::PathBuf) -> Result<String,String> {
    fs::read_to_string(path.clone()).map_err(|_| format!("couldn't read file: '{}'", path.display()))
}
fn get_path() -> Result<path::PathBuf,String> {
    if env::args().len() > 1 {
        let path = env::args().nth(1).unwrap();
        path.parse::<path::PathBuf>().map_err(|_| "no such path".into())
    } else {
        match FileDialog::new()
            .set_location("~/Desktop")
            .add_filter("Mach3Mill Toolpath", &["txt"])
            .show_open_single_file().transpose() {
            Some(r) => {
                r.map_err(|_| "invalid file".into())
            } None => {
                Err("no file specified".into())
            }
        }
    }
}

#[derive(Clone,Copy,Debug,PartialEq)]
struct Point {
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
}
impl Point {
    fn empty() -> Point {
        Point { x: None, y: None, z: None }
    }
    fn new(x: Option<f32>, y: Option<f32>, z: Option<f32>) -> Point {
        Point { x, y, z }
    }
    fn from_str(input: &str) -> Point {
        lazy_static! {
            static ref RE: [Regex;3] = [
                Regex::new(r"X[= ]*-?[0-9]+\.[0-9]*").unwrap(),
                Regex::new(r"Y[= ]*-?[0-9]+\.[0-9]*").unwrap(),
                Regex::new(r"Z[= ]*-?[0-9]+\.[0-9]*").unwrap(),
            ];
            static ref NUM_RE: Regex = Regex::new(r"-?[0-9]+\.[0-9]*").unwrap();
        }
        let mut point = Point::empty();
        for i in 0..3 {
            point[i] = RE[i].find(input).map(|ma| {
                let number = NUM_RE.find(ma.as_str()).unwrap().as_str();
                number.parse::<f32>().unwrap()
            });
        }
        point
    }
    fn min(&self, other: Point) -> Point {
        let mut new = Point::empty();
        for i in 0..3 {
            new[i] = if self[i].is_some() {
                other[i].map_or(self[i],|v| Some(v.min(self[i].unwrap())))
            } else { other[i] };
        }
        new
    }
    fn max(&self, other: Point) -> Point {
        let mut new = Point::empty();
        for i in 0..3 {
            new[i] = if self[i].is_some() {
                other[i].map_or(self[i],|v| Some(v.max(self[i].unwrap())))
            } else { other[i] };
        }
        new
    }
    fn is_empty(&self) -> bool {
        self.x.is_none() && self.y.is_none() && self.z.is_none()
    }
}
impl ops::Index<usize> for Point {
    type Output = Option<f32>;
    fn index(&self, i: usize) -> &Self::Output {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!(),
        }
    }
}
impl ops::IndexMut<usize> for Point {
    fn index_mut(&mut self, i: usize) -> &mut Self::Output {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => panic!(),
        }
    }
}
