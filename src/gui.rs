use anyhow::Result;
#[cfg(target_family = "unix")]
use anyhow::Context;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
#[cfg(target_family = "unix")]
use std::io::{Read, Write};
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_family = "unix")]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(target_family = "unix")]
use std::process::{Command, Stdio};
#[cfg(target_family = "unix")]
use std::thread;
#[cfg(target_family = "unix")]
use std::time::Duration;

use crate::{FetchResult, PlayerRecord, fetch_all};

#[cfg(target_family = "unix")]
pub fn maybe_run_helper() -> bool {
    if std::env::args().any(|a| a == "--helper") {
        let sock_path = "/tmp/pnidgrab.sock";
        let _ = std::fs::remove_file(sock_path);
        let listener = match UnixListener::bind(sock_path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Failed to bind helper socket: {e}");
                return true;
            }
        };
        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                if let Ok(result) = fetch_all() {
                    let json = serde_json::to_string(&result).unwrap_or_else(|_| "{}".into());
                    let _ = s.write_all(json.as_bytes());
                } else {
                    let _ = s.write_all(b"{}");
                }
            }
        }
        return true;
    }
    false
}

#[cfg(not(target_family = "unix"))]
pub fn maybe_run_helper() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn get_password() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/bin/osascript")
            .args([
                "-e",
                r#"tell application "System Events""#,
                "-e",
                r#"activate"#,
                "-e",
                r#"set dlg to display dialog "PNIDGrab needs your administrator password to read process memory." with title "PNIDGrab" default answer "" with icon caution buttons {"OK"} default button "OK" with hidden answer"#,
                "-e",
                r#"text returned of dlg"#,
                "-e",
                r#"end tell"#,
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let mut pw = String::from_utf8_lossy(&output.stdout).to_string();
        while pw.ends_with('\n') || pw.ends_with('\r') {
            pw.pop();
        }
        return if pw.is_empty() { None } else { Some(pw) };
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(target_family = "unix")]
fn start_privileged_helper() -> Result<()> {
    let exe = std::env::current_exe().context("current_exe failed")?;

    #[cfg(target_os = "linux")]
    {
        let helper_path = format!("/tmp/pnidgrab-helper-{}", std::process::id());
        let helper_exe = if std::fs::copy(&exe, &helper_path).is_ok() {
            let _ = std::fs::set_permissions(&helper_path, std::fs::Permissions::from_mode(0o755));
            helper_path.as_str()
        } else {
            exe.to_string_lossy().as_ref()
        };
        Command::new("pkexec")
            .arg(helper_exe)
            .arg("--helper")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn pkexec helper")?;
    }

    #[cfg(target_os = "macos")]
    {
        let Some(password) = get_password() else {
            anyhow::bail!("Password prompt was cancelled or failed");
        };

        let mut child = Command::new("sudo")
            .arg("-S")
            .arg("--")
            .arg(exe)
            .arg("--helper")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn sudo helper")?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = write!(stdin, "{}\n", password);
            let _ = stdin.flush();
        }
    }

    Ok(())
}

#[cfg(not(target_family = "unix"))]
fn request_fetch_via_helper() -> Result<FetchResult> {
    anyhow::bail!("Helper socket not supported on this platform");
}

#[cfg(target_family = "unix")]
fn request_fetch_via_helper() -> Result<FetchResult> {
    let mut retries = 0;
    loop {
        match UnixStream::connect("/tmp/pnidgrab.sock") {
            Ok(mut s) => {
                let mut buf = String::new();
                s.read_to_string(&mut buf)?;
                return Ok(serde_json::from_str(&buf)?);
            }
            Err(_) if retries < 10 => {
                retries += 1;
                thread::sleep(Duration::from_millis(300));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn gender_label(code: u8) -> &'static str {
    match code {
        0 => "Girl",
        1 => "Boy",
        2 => "Rival",
        _ => "Unknown",
    }
}

fn show_player_properties(parent: &adw::ApplicationWindow, player: &PlayerRecord) {
    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&format!("Player {}", player.index + 1))
        .default_width(420)
        .default_height(520)
        .build();

    let header = adw::HeaderBar::new();
    let title = gtk4::Label::new(Some(&format!("Player {}", player.index + 1)));
    title.add_css_class("title-3");
    header.set_title_widget(Some(&title));
    header.set_show_end_title_buttons(true);

    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content_box.set_margin_top(18);
    content_box.set_margin_bottom(18);
    content_box.set_margin_start(18);
    content_box.set_margin_end(18);

    let mk_header = |text: &str| {
        let lbl = gtk4::Label::new(Some(text));
        lbl.set_xalign(0.0);
        lbl.add_css_class("title-4");
        lbl.add_css_class("large-font");
        content_box.append(&lbl);
    };

    let mk = |text: String| {
        let lbl = gtk4::Label::new(Some(&text));
        lbl.set_xalign(0.0);
        lbl.add_css_class("large-font");
        content_box.append(&lbl);
    };

    mk_header("General");
    mk(format!("Name: {}", player.name));
    mk(format!("PID (Hex): {}", player.pid_hex));
    mk(format!("PID (Dec): {}", player.pid_dec));
    mk(format!("PNID: {}", player.pnid));
    content_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    mk_header("Appearance");
    mk(format!(
        "Gender: {} ({})",
        player.gender,
        gender_label(player.gender)
    ));
    mk(format!("Skin Tone: {}", player.skin_tone));
    mk(format!(
        "Eye Color: {} ({})",
        player.eye_color, player.eye_color_name
    ));
    content_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    mk_header("Equipment");
    mk(format!(
        "Headgear: {} ({})",
        player.headgear, player.headgear_name
    ));
    mk(format!(
        "Clothes: {} ({})",
        player.clothes, player.clothes_name
    ));
    mk(format!("Shoes: {} ({})", player.shoes, player.shoes_name));
    mk(format!(
        "Ink Tank: {} ({})",
        player.tank_id, player.tank_name
    ));
    content_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    mk_header("Weapon");
    mk(format!(
        "Main Weapon ID: {} ({})",
        player.weapon_id_main, player.weapon_main_name
    ));
    mk(format!(
        "Sub Weapon ID: {} ({})",
        player.weapon_id_sub, player.weapon_sub_name
    ));
    mk(format!(
        "Special Weapon ID: {} ({})",
        player.weapon_id_special, player.weapon_special_name
    ));
    mk(format!(
        "Total Weapon Turf Points: {}p",
        player.weaponturf_total
    ));
    content_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    mk_header("Level, Rank & Fest");
    mk(format!("Level: {}", player.rank + 1));
    mk(format!(
        "Rank: {} ({})",
        player.rank_label, player.rank_points
    ));
    mk(format!("Fest ID: {}", player.fest_id));
    mk(format!("Fest Team: {}", player.fest_team));
    mk(format!("Fest Title: {}", player.fest_grade));

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&content_box));

    let action_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    action_box.set_halign(gtk4::Align::End);
    action_box.set_margin_top(6);
    action_box.set_margin_bottom(12);
    action_box.set_margin_end(12);

    let close_button = gtk4::Button::with_label("Close");
    {
        let dialog_clone = dialog.clone();
        close_button.connect_clicked(move |_| dialog_clone.close());
    }
    action_box.append(&close_button);

    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    main_box.append(&header);
    main_box.append(&scrolled);
    main_box.append(&action_box);

    dialog.set_content(Some(&main_box));
    dialog.present();
}

pub fn run_app() -> Result<()> {
    if maybe_run_helper() {
        return Ok(());
    }

    #[cfg(target_family = "unix")]
    {
        if nix::unistd::geteuid().as_raw() != 0 {
            let _ = start_privileged_helper();
        }
    }

    let app = adw::Application::builder()
        .application_id("dev.jerrysm64.pnidgrab")
        .build();

    app.connect_activate(build_ui);

    app.run();
    Ok(())
}

fn build_ui(app: &adw::Application) {
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("PNIDGrab 4.0.0")
        .default_width(450)
        .default_height(335)
        .resizable(false)
        .build();

    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        r#"
        .large-font {
            font-size: 13px;
        }
        treeview {
            font-size: 13px;
        }
        treeview header button {
            font-size: 13px;
            font-weight: bold;
        }
        "#,
    );

    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let toast_overlay = adw::ToastOverlay::new();

    let header_bar = adw::HeaderBar::new();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);

    let list_store = gtk4::ListStore::new(&[
        glib::Type::U8,     // Player #
        glib::Type::STRING, // PID Hex
        glib::Type::U32,    // PID Dec
        glib::Type::STRING, // PNID
        glib::Type::STRING, // Name
    ]);

    let tree_view = gtk4::TreeView::with_model(&list_store);
    tree_view.add_css_class("large-font");

    fn add_column(tree: &gtk4::TreeView, title: &str, col_idx: i32) {
        let renderer = gtk4::CellRendererText::new();
        let column = gtk4::TreeViewColumn::new();
        column.set_title(title);
        column.pack_start(&renderer, true);
        column.add_attribute(&renderer, "text", col_idx);
        tree.append_column(&column);
    }

    add_column(&tree_view, "Player", 0);
    add_column(&tree_view, "PID (Hex)", 1);
    add_column(&tree_view, "PID (Dec)", 2);
    add_column(&tree_view, "PNID", 3);
    add_column(&tree_view, "Name", 4);

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&tree_view));

    let session_label = gtk4::Label::new(Some("Session ID: None"));
    session_label.set_halign(gtk4::Align::Start);
    session_label.add_css_class("large-font");

    let timestamp_label = gtk4::Label::new(Some("Fetched at: -"));
    timestamp_label.set_halign(gtk4::Align::Start);
    timestamp_label.add_css_class("large-font");

    let bottom_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    bottom_box.set_hexpand(true);
    bottom_box.append(&session_label);
    bottom_box.append(&timestamp_label);

    let fetch_button = gtk4::Button::with_label("Fetch");
    fetch_button.add_css_class("large-font");

    let copy_button = gtk4::Button::with_label("Copy");
    copy_button.add_css_class("large-font");

    let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    button_box.set_halign(gtk4::Align::End);
    button_box.append(&copy_button);
    button_box.append(&fetch_button);

    let info_button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    info_button_box.set_hexpand(true);
    info_button_box.append(&bottom_box);
    info_button_box.append(&button_box);

    vbox.append(&scrolled);
    vbox.append(&info_button_box);

    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    main_box.append(&header_bar);
    main_box.append(&vbox);

    toast_overlay.set_child(Some(&main_box));

    win.set_content(Some(&toast_overlay));

    let player_data = std::rc::Rc::new(std::cell::RefCell::new(Vec::<PlayerRecord>::new()));
    let session_id_data = std::rc::Rc::new(std::cell::RefCell::new(None::<u32>));
    let timestamp_data = std::rc::Rc::new(std::cell::RefCell::new(String::new()));

    let list_store_clone = list_store.clone();
    let session_label_clone = session_label.clone();
    let timestamp_label_clone = timestamp_label.clone();
    let player_data_clone = player_data.clone();
    let session_id_data_clone = session_id_data.clone();
    let timestamp_data_clone = timestamp_data.clone();

    let fetch_logic = move || {
        match request_fetch_via_helper().or_else(|_| fetch_all()) {
            Ok(result) => {
                list_store_clone.clear();
                let mut pd = player_data_clone.borrow_mut();
                *pd = result.players.clone();
                *session_id_data_clone.borrow_mut() = result.session_id;
                *timestamp_data_clone.borrow_mut() =
                    result.fetched_at.format("%Y-%m-%d %H:%M:%S").to_string();

                for p in result.players.iter() {
                    let iter = list_store_clone.append();
                    list_store_clone.set(
                        &iter,
                        &[
                            (0, &(p.index + 1)),
                            (1, &p.pid_hex),
                            (2, &p.pid_dec),
                            (3, &p.pnid),
                            (4, &p.name),
                        ],
                    );
                }
                match result.session_id {
                    Some(sid) => session_label_clone
                        .set_label(&format!("Session ID: {:08X} (Dec: {})", sid, sid)),
                    None => session_label_clone.set_label("Session ID: None"),
                }
                timestamp_label_clone.set_label(&format!(
                    "Fetched at: {}",
                    result.fetched_at.format("%Y-%m-%d %H:%M:%S")
                ));
            }
            Err(e) => eprintln!("Fetch error: {}", e),
        }
        glib::ControlFlow::Break
    };

    glib::idle_add_local(fetch_logic.clone());
    fetch_button.connect_clicked(move |_| {
        glib::idle_add_local(fetch_logic.clone());
    });

    let player_data_copy = player_data.clone();
    let session_id_data_copy = session_id_data.clone();
    let timestamp_data_copy = timestamp_data.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let win_clone = win.clone();

    copy_button.connect_clicked(move |_| {
        let mut copy_text = String::new();
        for p in player_data_copy.borrow().iter() {
            copy_text.push_str(&format!("Player {}\n", p.index + 1));
            copy_text.push_str(&format!("Name: {}\n", p.name));
            copy_text.push_str(&format!("PID Hex: {}\n", p.pid_hex));
            copy_text.push_str(&format!("PID Dec: {}\n", p.pid_dec));
            copy_text.push_str(&format!("PNID: {}\n", p.pnid));

            copy_text.push_str(&format!(
                "Gender: {} ({})\n",
                p.gender,
                gender_label(p.gender)
            ));
            copy_text.push_str(&format!("Skin Tone: {}\n", p.skin_tone));
            copy_text.push_str(&format!(
                "Eye Color: {} ({})\n",
                p.eye_color, p.eye_color_name
            ));
            copy_text.push_str(&format!(
                "Headgear: {} ({})\n",
                p.headgear, p.headgear_name
            ));
            copy_text.push_str(&format!(
                "Clothes: {} ({})\n",
                p.clothes, p.clothes_name
            ));
            copy_text.push_str(&format!(
                "Shoes: {} ({})\n",
                p.shoes, p.shoes_name
            ));
            copy_text.push_str(&format!(
                "Ink Tank: {} ({})\n",
                p.tank_id, p.tank_name
            ));
            copy_text.push_str(&format!(
                "Main Weapon ID: {} ({})\n",
                p.weapon_id_main, p.weapon_main_name
            ));
            copy_text.push_str(&format!(
                "Main Weapon ID: {} ({})\n",
                p.weapon_id_sub, p.weapon_sub_name
            ));
            copy_text.push_str(&format!(
                "Special Weapon ID: {} ({})\n",
                p.weapon_id_special, p.weapon_special_name
            ));

            copy_text.push_str(&format!("Level: {}\n", p.rank + 1));
            copy_text.push_str(&format!("Rank: {} ({})\n", p.rank_label, p.rank_points));
            copy_text.push_str(&format!("Fest Team: {}\n", p.fest_team));
            copy_text.push_str(&format!("Fest ID: {}\n", p.fest_id));
            copy_text.push_str(&format!("Fest Title: {}\n", p.fest_grade));
            copy_text.push('\n');
        }
        if let Some(sid) = *session_id_data_copy.borrow() {
            copy_text.push_str(&format!("Session ID: {:08X} (Dec: {})\n", sid, sid));
        } else {
            copy_text.push_str("Session ID: None\n");
        }
        copy_text.push_str(&format!("Fetched at: {}\n", *timestamp_data_copy.borrow()));

        let clipboard = win_clone.clipboard();
        clipboard.set_text(&copy_text);

        let toast = adw::Toast::new("Data copied to clipboard!");
        toast.set_timeout(2);
        toast_overlay_clone.add_toast(toast);
    });

    let win_for_dialog = win.clone();
    let player_data_for_dialog = player_data.clone();
    tree_view.connect_row_activated(move |view, path, _| {
        if let Some(model) = view.model() {
            if let Some(iter) = model.iter(path) {
                let idx_val: u8 = model.get(&iter, 0);
                let players = player_data_for_dialog.borrow();
                if idx_val == 0 {
                    return;
                }
                if let Some(p) = players.iter().find(|pp| pp.index == idx_val - 1) {
                    show_player_properties(&win_for_dialog, p);
                }
            }
        }
    });

    win.show();
}
