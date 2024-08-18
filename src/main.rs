use clap::Parser;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Paths to search (relative to home directory unless starting with /)
    #[arg(required = true, num_args = 1..)]
    paths: Vec<String>,

    /// Maximum depth to search
    #[arg(short, long, default_value_t = 1)]
    depth: usize,
}

fn expand_path(path: &str, home: &Path) -> PathBuf {
    if path.starts_with('/') {
        PathBuf::from(path)
    } else if path.starts_with("~") {
        home.join(&path[2..])
    } else {
        home.join(path)
    }
}

fn is_git_repository(dir: &Path) -> bool {
    if dir.join(".git").is_dir() {
        // Este comando falla si es un dir normal. si es bare devuelve false pero
        // no error aunque no entra en este
        // .map(|output| output.status.success())
        // .unwrap_or(false)
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .arg("rev-parse")
            .arg("--is-inside-work-tree")
            .output();
        // let stdout_utf8 = std::str::from_utf8(&output.stdout);
        let stdout_str = match output {
            Ok(output) => String::from_utf8(output.stdout)
                .unwrap_or_else(|_| "".to_string())
                .trim()
                .to_string(),
            Err(_) => String::new(),
        };
        // println!("Output: {}", stdout_str);
        // println!("{:?}", is_git_dir);
        stdout_str.eq("true")
    } else {
        false
    }
}

fn is_bare_repository(dir: &Path) -> bool {
    if dir.join("HEAD").is_file() {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .arg("rev-parse")
            .arg("--is-inside-git-dir")
            .output();
        // let stdout_utf8 = std::str::from_utf8(&output.stdout);
        let stdout_str = match output {
            Ok(output) => String::from_utf8(output.stdout)
                .unwrap_or_else(|_| "".to_string())
                .trim()
                .to_string(),
            Err(_) => String::new(),
        };
        // println!("Output: {}", stdout_str);
        // println!("{:?}", is_git_dir);

        if stdout_str.eq("true") {
            // Check if the directory name is not ".git"
            return dir.file_name().map_or(false, |name| name != ".git");
        } else {
            return false;
        };
    }
    false
}

#[derive(Clone)]
struct ProjectDir {
    dir_type: DirType,
    path: PathBuf,
}

#[derive(Clone, PartialEq)]
enum DirType {
    BareGit,
    WorkTree,
    Git,
    Dir,
}

fn list_worktrees(bare_repo_path: &Path) -> Option<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(bare_repo_path)
        .arg("worktree")
        .arg("list")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();

    for line in stdout.lines() {
        if line.contains("(bare)") {
            continue;
        }
        // Split line on whitespace and take the first part (the path)
        if let Some((path, _)) = line.split_once(' ') {
            worktrees.push(PathBuf::from(path));
        }
    }

    Some(worktrees)
}

fn process_entries(
    full_path: &Path,
    depth: usize,
    recursivity_level: Option<usize>,
) -> Vec<ProjectDir> {
    let mut results = Vec::new();
    let level = recursivity_level.unwrap_or(0);

    WalkDir::new(full_path)
        .min_depth(1)
        .max_depth(depth)
        .into_iter()
        .filter_entry(|e| e.file_type().is_dir())
        .for_each(|entry| {
            if let Ok(entry) = entry {
                let path = entry.path();

                let dir_type = if is_bare_repository(path) {
                    DirType::BareGit
                } else if is_git_repository(path) {
                    DirType::Git
                } else {
                    DirType::Dir
                };

                results.push(ProjectDir {
                    dir_type: dir_type.clone(),
                    path: path.to_path_buf(),
                });

                if let DirType::Git = dir_type {
                    // Recursively process the directory for additional Git repositories
                    // let mut nested_results = process_entries(path, 2)
                    //     .iter()
                    //     .filter(|&x| x.dir_type != DirType::Dir)
                    //     .cloned()
                    //     .collect();
                    // results.append(&mut nested_results);
                }
                if let DirType::Dir = dir_type {
                    if level <= 1 {
                        // Recursively process the directory for additional Git repositories
                        let mut nested_results = process_entries(path, 1, Some(level + 1))
                            .iter()
                            .filter(|&x| x.dir_type != DirType::Dir)
                            .cloned()
                            .collect();
                        results.append(&mut nested_results);
                    }
                }
                if let DirType::BareGit = dir_type {
                    if let Some(worktrees) = list_worktrees(path) {
                        for worktree in worktrees {
                            results.push(ProjectDir {
                                dir_type: DirType::WorkTree, // Mark worktrees as Git repos
                                path: worktree,
                            });
                        }
                    }
                }
            }
        });

    results
}

fn main() {
    let args = Args::parse();
    let home = PathBuf::from(env::var("HOME").expect("Failed to get HOME directory"));

    for path in &args.paths {
        let full_path = expand_path(path, &home);
        let entries = process_entries(&full_path, args.depth, None);

        for entry in entries {
            match entry.dir_type {
                DirType::BareGit => println!("(bare) {}", entry.path.display()),
                DirType::Git => println!("(git) {}", entry.path.display()),
                DirType::Dir => println!("(dir) {}", entry.path.display()),
                DirType::WorkTree => println!("(wt) {}", entry.path.display()),
            }
        }
    }
}
