use clap::Parser;
use glob::glob;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use toml::Table;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(
    version,
    about = "Compares the contents of requirement files (requirements*.txt and pyproject.toml) with the output of `pip freeze`",
    author = "DataLuxe Technologies"
)]
struct Cli {
    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Check if there is any difference between pip installed packages and requirements"
    )]
    all: bool,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Check if there are requirements listed that are not installed"
    )]
    pip: bool,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Check if there are installed packages that are not listed in requirements"
    )]
    file: bool,
}

fn split_module_and_version(line: &str) -> (String, String) {
    let default = format!("{line}== ");

    let mut split_text = if line.contains("@") {
        line.splitn(2, "@")
    } else if line.contains("==") {
        line.splitn(2, "==")
    } else if line.contains("===") {
        line.splitn(2, "===")
    } else if line.contains("<=") {
        line.splitn(2, "<=")
    } else if line.contains(">=") {
        line.splitn(2, ">=")
    } else if line.contains("!=") {
        line.splitn(2, "!=")
    } else if line.contains("~=") {
        line.splitn(2, "~=")
    } else if line.contains(">") {
        line.splitn(2, ">")
    } else if line.contains("<") {
        line.splitn(2, "<")
    } else {
        default.splitn(2, "==")
    };

    (
        split_text.next().unwrap().trim().to_owned(),
        split_text.next().unwrap().trim().to_owned(),
    )
}

fn get_file_dependencies() -> HashMap<String, String> {
    let mut map = HashMap::new();

    let req_pattern = "requirements*.txt";
    println!("Searching for files matching: '{}'", req_pattern);

    for entry in glob(req_pattern).unwrap() {
        if let Ok(path) = entry {
            if !path.is_file() {
                continue;
            }

            println!("Reading content from: {}", path.display());

            if let Ok(file_content) = fs::read_to_string(&path) {
                for line in file_content.lines() {
                    let (key, value) = split_module_and_version(line);
                    map.insert(key, value);
                }
            } else {
                eprintln!("Failed to read file content.");
            }
        }
    }

    let toml_pattern = "pyproject.toml";
    println!("Searching for files matching: '{}'", toml_pattern);

    for entry in glob(toml_pattern).unwrap() {
        if let Ok(path) = entry {
            if !path.is_file() {
                continue;
            }

            println!("Reading content from: {}", path.display());

            if let Ok(file_content) = fs::read_to_string(&path) {
                let table = file_content.parse::<Table>();
                if table.is_err() {
                    continue;
                }

                let table = table.unwrap();
                let project = table.get("project");
                if project.is_none() {
                    continue;
                }
                let project = match project.unwrap() {
                    toml::Value::Table(project) => project,
                    _ => panic!("Project is invalid!"),
                };

                let dependencies = project.get("dependencies");
                if let Some(dependencies) = dependencies {
                    let dependencies = match dependencies {
                        toml::Value::Array(d) => d,
                        _ => panic!("Dependencies is invalid!"),
                    };
                    for dependency in dependencies {
                        match dependency {
                            toml::Value::String(s) => {
                                let (key, value) = split_module_and_version(s);
                                map.insert(key, value);
                            }
                            _ => (),
                        }
                    }
                }
                let optional_dependencies = project.get("optional-dependencies");
                if let Some(dependencies) = optional_dependencies {
                    let dependencies = match dependencies {
                        toml::Value::Table(d) => d.values().collect::<Vec<&toml::Value>>(),
                        _ => panic!("Dependencies is invalid!"),
                    };
                    for dependency in dependencies {
                        match dependency {
                            toml::Value::String(s) => {
                                let (key, value) = split_module_and_version(s);
                                map.insert(key, value);
                            }
                            _ => (),
                        }
                    }
                }
            } else {
                eprintln!("Failed to read file content.");
            }
        }
    }

    println!();
    map
}

fn get_pip_dependencies() -> HashMap<String, String> {
    let pip_freeze_result = Command::new("pip")
        .arg("freeze")
        .output()
        .expect("Failed to execute `pip freeze`");

    if !pip_freeze_result.status.success() {
        eprintln!("Error executing `pip freeze`");
        return HashMap::new();
    }

    let pip_content = std::str::from_utf8(&pip_freeze_result.stdout).unwrap();

    let mut map = HashMap::new();
    for line in pip_content.lines() {
        let (key, value) = split_module_and_version(line);
        map.insert(key, value);
    }
    map
}

fn compare_pip_with_file(
    file_lines: &HashMap<String, String>,
    pip_lines: &HashMap<String, String>,
) {
    println!("Comparing requirements with pip.");
    let mut has_error = false;
    for (file_module, file_version) in file_lines.iter() {
        match pip_lines.get(file_module) {
            Some(env_version) => {
                if env_version == file_version {
                    continue;
                }

                eprintln!(
                    "File dependency {} has different version than pip:\n\t- File version: {}\n\t- Pip version: {}",
                    file_module, file_version, env_version
                );
                has_error = true;
            }
            None => {
                eprintln!("Pip is missing dependency: {}", file_module);
                has_error = true;
            }
        }
    }

    println!();
    if !has_error {
        println!("No dependency diversion found from requirements to pip.");
    } else {
        println!("Check above errors for diversions.");
    }
    println!();
}

fn compare_file_with_pip(
    file_dependencies: &HashMap<String, String>,
    pip_dependencies: &HashMap<String, String>,
) {
    println!("Comparing pip with requirements.");
    let mut has_error = false;
    for (pip_module, pip_version) in pip_dependencies.iter() {
        match file_dependencies.get(pip_module) {
            Some(file_version) => {
                if file_version == pip_version {
                    continue;
                }

                eprintln!(
                    "Pip dependency {} has different version than requirements:\n\t- Pip version: {}\n\t- File version: {}",
                    pip_module, pip_version, file_version
                );
                has_error = true;
            }
            None => {
                eprintln!("Requirements is missing dependency: {}", pip_module);
                has_error = true;
            }
        }
    }

    println!();
    if !has_error {
        println!("No dependency diversion found from pip to requirements.");
    } else {
        println!("Check above errors for diversions.");
    }
    println!();
}

fn main() {
    let cli = Cli::parse();

    let file_lines = get_file_dependencies();
    let pip_lines = get_pip_dependencies();

    if cli.all {
        compare_pip_with_file(&file_lines, &pip_lines);
        compare_file_with_pip(&file_lines, &pip_lines);
        return;
    }

    if cli.pip {
        compare_pip_with_file(&file_lines, &pip_lines);
    }

    if cli.file {
        compare_file_with_pip(&file_lines, &pip_lines);
    }
}
