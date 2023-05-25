use clap::{
    ArgAction,
    Parser,
    ValueHint,
};

use std::{
    fs::{
        remove_file,
        remove_dir,
        read_dir,
        File,
    },
    path::{
        Path,
        PathBuf,
    },
    ffi::{
        OsStr,
        OsString,
    },
    collections::{
        HashMap,
        HashSet,
        hash_map::Entry::{
            Vacant,
            Occupied,
        },
    },
    io,
    io::{
        Read,
        BufReader,
    },
};

use walkdir::WalkDir;
use junk;
use dialoguer::Confirm;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, value_hint = ValueHint::DirPath)]
    source_dir: PathBuf,

    #[arg(long, value_hint = ValueHint::DirPath)]
    compare_dir: PathBuf,

    #[arg(long, action = ArgAction::SetTrue)]
    run: bool,
}

fn main() {
    let args = Args::parse();
    let dry_run = !args.run;

    if !dry_run {
        if !Confirm::new().with_prompt(format!("Do you really want to delete files into {:?}?", args.source_dir)).interact().unwrap_or(false) {
            return;
        }
    };

    let filename_path_map = match get_filename_path_map(&args.compare_dir) {
        Ok(map) => map,
        Err(err) => {
            println!("IO Error: {:?}", err);
            return;
        },
    };

    delete_duplicate_files(&args.source_dir, &filename_path_map, dry_run);
}

fn delete_duplicate_files(dir: &Path, filename_path_map: &HashMap<OsString, HashSet<PathBuf>>, dry_run: bool) -> bool {
    let paths = match read_dir(dir) {
        Ok(paths) => paths,
        Err(err) => {
            println!("Read Dir Error: {:?}", err);
            return false;
        },
    };
    let mut all_deleted = true;
    for path in paths {
        let path = match path {
            Ok(path) => path.path(),
            Err(err) => {
                println!("Read Dir Entry Error: {:?}", err);
                all_deleted = false;
                continue;
            },
        };
        if path.is_dir() {
            let children_all_deleted = delete_duplicate_files(&path, filename_path_map, dry_run);
            if children_all_deleted {
                println!("Delete Empty Dir {:?} (dry_run={:?})", path, dry_run);
                if !dry_run {
                    match remove_dir(path) {
                        Ok(_) => (),
                        Err(err) => {
                            println!("Delete Empty Dir Error: {:?}", err);
                            all_deleted = false;
                        },
                    };
                }
            } else {
                all_deleted = false;
            }
        } else {
            let filename = match path.file_name() {
                Some(filename) => filename,
                None => {
                    println!("Couldnt Get Filename: {:?}", path);
                    all_deleted = false;
                    continue;
                },
            };
            if is_junk_filename(filename) {
                println!("Delete Junk File {:?} (dry_run={:?})", path, dry_run);
                if !dry_run {
                    match remove_file(path) {
                        Ok(_) => (),
                        Err(err) => {
                            println!("Delete Junk File Error: {:?}", err);
                            all_deleted = false;
                        },
                    };
                }
            } else {
                if let Some(current_paths) = filename_path_map.get(filename) {
                    let exists_same_file = current_paths.iter().any(|current_path| is_same_file(current_path, &path));
                    if exists_same_file {
                        println!("Delete Duplicated File {:?} (dry_run={:?})", path, dry_run);
                        if !dry_run {
                            match remove_file(path) {
                                Ok(_) => (),
                                Err(err) => {
                                    println!("Delete Duplicated File Error: {:?}", err);
                                    all_deleted = false;
                                },
                            };
                        };
                    } else {
                        println!("Different content {:?}", path);
                        all_deleted = false
                    }
                } else {
                    println!("Original filename {:?}", path);
                    all_deleted = false;
                }
            }
        }
    }
    return all_deleted;
}

fn get_filename_path_map(dir: &Path) -> Result<HashMap<OsString, HashSet<PathBuf>>, io::Error> {
    let mut map: HashMap<OsString, HashSet<PathBuf>> = HashMap::new();
    for entry in WalkDir::new(dir).into_iter().filter_entry(|e| !is_junk_filename(e.file_name())) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue
        };

        let filename: OsString = entry.file_name().into();
        let path: PathBuf = entry.path().into();
        let path_set: &mut HashSet<PathBuf> = match map.entry(filename) {
            Occupied(occupied) => occupied.into_mut(),
            Vacant(vacant) => vacant.insert(HashSet::new()),
        };
        path_set.insert(path);
    };
    Ok(map)
}

fn is_same_file(a: &Path, b: &Path) -> bool {
    let a_size = match a.metadata() {
        Ok(metadata) => metadata.len(),
        Err(_) => return false,
    };
    let b_size = match b.metadata() {
        Ok(metadata) => metadata.len(),
        Err(_) => return false,
    };
    if a_size != b_size {
        return false;
    }

    let a_file = match File::open(a) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let b_file = match File::open(b) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let mut a_reader = BufReader::new(a_file);
    let mut b_reader = BufReader::new(b_file);

    let mut buf_a = [0; 1024];
    let mut buf_b = [0; 1024];

    loop {
        let len_a = match a_reader.read(&mut buf_a) {
            Ok(len) => len,
            Err(_) => return false,
        };
        let len_b = match b_reader.read(&mut buf_b) {
            Ok(len) => len,
            Err(_) => return false,
        };

        if len_a != len_b || buf_a[..len_a] != buf_b[..len_b] {
            return false;
        }
        if len_a == 0 {
            return true;
        }
    }
}

fn is_junk_filename(filename: &OsStr) -> bool {
    filename.to_str().map_or(false, |s| junk::is(s))
}

