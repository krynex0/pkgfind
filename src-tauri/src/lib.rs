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
// Формат виводу:
//   extra/firefox 143.0-1 [installed]
//       Mozilla Firefox web browser
// Непарні рядки (без відступу) — заголовок пакету, парні (з відступом) — опис.
fn parse_search_output(raw: &str) -> Vec<Package> {
    let mut packages = Vec::new();
    let mut lines = raw.lines().peekable();

    while let Some(line) = lines.next() {
        // рядки з описом/порожні пропускаємо тут — вони обробляються нижче,
        // одразу після відповідного заголовка
        if line.starts_with(' ') || line.starts_with('\t') || line.trim().is_empty() {
            continue;
        }

        // yay/pacman дають рядок installed по-різному: pacman -Ss пише "[installed]",
        // локалізований yay може писати "(Встановлено)" — перевіряємо обидва варіанти
        let installed = line.contains("[installed]") || line.contains("(Встановлено)");

        // формат заголовка: "repo/name version [голоси/популярність] [час] [installed]"
        // (yay додає додаткові дужки після версії — pacman цього не робить)
        let Some(slash_pos) = line.find('/') else { continue };
        let repo = line[..slash_pos].to_string();
        let after_slash = &line[slash_pos + 1..];

        let Some(name_end) = after_slash.find(' ') else { continue };
        let pkgname = after_slash[..name_end].to_string();

        // версія — тільки наступне "слово" після назви, решта рядка (голоси, дата, installed) ігнорується
        let rest = after_slash[name_end + 1..].trim_start();
        let version_end = rest.find(' ').unwrap_or(rest.len());
        let version = rest[..version_end].to_string();

        let (pkgver, pkgrel) = match version.rsplit_once('-') {
            Some((ver, rel)) => (ver.to_string(), rel.to_string()),
            None => (version.clone(), String::new()),
        };

        // наступний рядок (якщо є і починається з відступу) — це опис
        let pkgdesc = match lines.peek() {
            Some(next) if next.starts_with(' ') || next.starts_with('\t') => {
                let desc = next.trim().to_string();
                lines.next(); // споживаємо рядок опису
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
        .map_err(|e| format!("не вдалося запустити pacman -Ss: {e}"))?;

    // pacman повертає ненульовий код виходу, якщо нічого не знайдено —
    // це не помилка виконання, просто порожній результат
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
        .map_err(|e| format!("не вдалося запустити yay: {e} (можливо yay не встановлено)"))?;

    // yay повертає ненульовий код виходу, якщо нічого не знайдено —
    // це не помилка виконання, просто порожній результат
    if !output.status.success() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut packages = parse_search_output(&text);

    // сортуємо за популярністю неможливо (yay -Ss не дає числа тут),
    // лишаємо порядок видачі yay і просто обрізаємо
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