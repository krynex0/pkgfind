use std::process::Command;

#[derive(serde::Serialize)]
struct Package {
    pkgname: String,
    pkgver: String,
    pkgrel: String,
    repo: String,
    pkgdesc: String,
    installed: bool,
}

// ==== parse "pacman -Ss <query>" output into Packages ====
// Output format:
//   extra/firefox 143.0-1 [installed]
//       Mozilla Firefox web browser
// Non-indented lines are package headers, indented lines are descriptions.
fn parse_search_output(raw: &str) -> Vec<Package> {
    let mut packages = Vec::new();
    let mut lines = raw.lines().peekable();

    while let Some(line) = lines.next() {
        // skip description/empty lines here — they're consumed below,
        // right after their matching header
        if line.starts_with(' ') || line.starts_with('\t') || line.trim().is_empty() {
            continue;
        }

        // pacman -Ss marks installed packages with "[installed]"
        let installed = line.contains("[installed]");

        // header format: "repo/name version [votes/popularity] [age] [installed]"
        // (yay appends extra parentheses after the version — pacman does not)
        let Some(slash_pos) = line.find('/') else { continue };
        let repo = line[..slash_pos].to_string();
        let after_slash = &line[slash_pos + 1..];

        let Some(name_end) = after_slash.find(' ') else { continue };
        let pkgname = after_slash[..name_end].to_string();

        // version is only the next "word" after the name; the rest of the line
        // (votes, age, installed marker) is ignored
        let rest = after_slash[name_end + 1..].trim_start();
        let version_end = rest.find(' ').unwrap_or(rest.len());
        let version = rest[..version_end].to_string();

        let (pkgver, pkgrel) = match version.rsplit_once('-') {
            Some((ver, rel)) => (ver.to_string(), rel.to_string()),
            None => (version.clone(), String::new()),
        };

        // the next line (if indented) is the description
        let pkgdesc = match lines.peek() {
            Some(next) if next.starts_with(' ') || next.starts_with('\t') => {
                let desc = next.trim().to_string();
                lines.next(); // consume the description line
                desc
            }
            _ => String::new(),
        };

        if !pkgname.is_empty() {
            packages.push(Package {
                pkgname,
                pkgver,
                pkgrel,
                repo,
                pkgdesc,
                installed,
            });
        }
    }

    packages
}

// ==== the Tauri command exposed to the frontend ====
#[tauri::command]
fn search_official(query: String) -> Result<Vec<Package>, String> {
    let output = Command::new("pacman")
        .arg("-Ss")
        .arg(&query)
        .output()
        .map_err(|e| format!("failed to run pacman -Ss: {e}"))?;

    // pacman exits non-zero when nothing is found — that's not a runtime
    // error, just an empty result
    if !output.status.success() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut packages = parse_search_output(&text);

    packages.sort_by(|a, b| a.pkgname.cmp(&b.pkgname));
    packages.truncate(25);

    Ok(packages)
}

#[tauri::command]
fn search_aur(query: String) -> Result<Vec<Package>, String> {
    let output = Command::new("yay")
        .arg("-Ss")
        .arg("--aur")
        .arg(&query)
        .output()
        .map_err(|e| format!("failed to run yay: {e} (yay may not be installed)"))?;

    // yay exits non-zero when nothing is found — that's not a runtime
    // error, just an empty result
    if !output.status.success() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut packages = parse_search_output(&text);

    // can't sort by popularity here (yay -Ss doesn't expose a number for it),
    // keep yay's own result order and just truncate
    packages.truncate(25);

    Ok(packages)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![search_official, search_aur])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}