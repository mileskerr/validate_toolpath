#[macro_use]
extern crate lazy_static;
use std::error::Error;
use regex::Regex;
use std::ops;
use std::fmt;
use std::env;
use std::fs;
use std::path;
use native_dialog::{FileDialog, MessageDialog, MessageType};


const DEPTH_THRESHOLD: f32 = 0.0625;
const MIN_OFFSET: f32 = -0.1;
const MAX_OFFSET: f32 = 0.75;

fn main() {
    let path = get_path().unwrap();

    let contents = fs::read_to_string(path.clone())
        .expect("Something went wrong reading the file");

    println!("file {:?}:",path);
    let results = check(&contents);
    for result in results {
        println!("{}",result);
    };
    loop {}
}

fn check(contents: &String) -> Vec<Outcome> {
    let mut min = Point::empty();
    let mut cut_min = Point::empty();
    let mut material_size = Point::empty();

    for line in contents.lines() {
        if line.starts_with("(") && line.ends_with(")") {
            let point = Point::from_str(line);
            if material_size.is_empty() && !point.is_empty() {
                material_size = point;
            }
        }
        if line.find("G0").is_some() || line.find("G1").is_some() {
            let point = Point::from_str(line);
            min = min.min(point);
            if point.z.is_some() && material_size.z.is_some() {
                if point.z.unwrap() < material_size.z.unwrap() {
                    cut_min = cut_min.min(point);
                }
            }
        }
    }

   
    println!("{:?}",material_size);
    println!("{:?}",min);
    vec![
        check_depth(min,material_size),
        check_offset(cut_min),
    ]
}

fn check_depth(min: Point, material_size: Point) -> Outcome {
    let mut out = Outcome::new("Check Depth");
    if material_size.z.is_some() {
        let thickness = material_size.z.unwrap();
        if min.z.is_some() {
            let max_depth = thickness - min.z.unwrap();
            if max_depth > thickness + DEPTH_THRESHOLD {
                return out.set(Status::Fail,
                    format!("toolpath may be cutting too deep:\nmaterial thickness: {}\nmax cut depth: {}",thickness,max_depth)
                );
            } else if max_depth < thickness {
                return out.set(Status::Fail,
                    format!("toolpath may not cut through material:\nmaterial thickness: {}\nmax cut depth: {}",thickness,max_depth)
                );
            } else {
                return out.set(Status::Pass,
                    format!("( material thickness: {}, max cut depth: {} )",thickness,max_depth)
                );
            }
        }
    }
    return out.set(Status::Error,
        "unable to check depth. This is may be a bug".into()
    );
}
fn check_offset(min: Point) -> Outcome {
    let mut out = Outcome::new("Check Offset");
    if min.x.is_some() && min.y.is_some() {
        for i in 0..2 {
            if min[i].unwrap() > MAX_OFFSET {
                return out.set(Status::Fail,
                    format!("toolpath may be offset:\nsouthwest corner of part is far from the origin, at ({}, {})",min.x.unwrap(),min.y.unwrap())
                );
            }
            if min[i].unwrap() < MIN_OFFSET {
                return out.set(Status::Fail,
                    format!("toolpath may be offset:\nsouthwest corner of part is negative, at ({}, {})",min.x.unwrap(),min.y.unwrap())
                );
            }
        }
        return out.set(Status::Pass,
            format!("( southeast corner of part is near the origin, at ({}, {}) )",min.x.unwrap(), min.y.unwrap())
        );
    }
    return out.set(Status::Error,
        "unable to check offset. This is may be a bug".into()
    );
}

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
        write!(f, "{}: [{}]\n{}", self.name, self.status, message)
    }
}

enum Status {
    Pass,
    Fail,
    Error,
}
impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Pass => write!(f,"PASS"),
            Status::Fail => write!(f,"FAIL"),
            Status::Error => write!(f,"ERROR"),
        }
    }
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
                Regex::new(r"X[= ]*-?[0-9]*\.[0-9]*").unwrap(),
                Regex::new(r"Y[= ]*-?[0-9]*\.[0-9]*").unwrap(),
                Regex::new(r"Z[= ]*-?[0-9]*\.[0-9]*").unwrap(),
            ];
            static ref NUM_RE: Regex = Regex::new(r"-?[0-9]*\.[0-9]*").unwrap();
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
