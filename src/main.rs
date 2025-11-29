use anyhow::Result;
use chrono::{DateTime, Local};
use reqwest::blocking::Client;
use roxmltree::Document;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod gui;
mod platform;

use platform::ProcessMemory;
use platform::find_cemu_process;

#[cfg(target_os = "linux")]
use platform::LinuxProcessMemory as PlatformProcessMemory;

#[cfg(target_os = "windows")]
use platform::WindowsProcessMemory as PlatformProcessMemory;

#[cfg(target_os = "macos")]
use platform::MacProcessMemory as PlatformProcessMemory;

const PLAYER_ROOT_PTR: u32 = 0x101DD330;
const PLAYER_LIST_OFFSET: u32 = 0x10;
const PLAYER_SLOT_STRIDE: u32 = 0x4;

const OFF_NAME: u32 = 0x6;
const OFF_AREA: u32 = 0x2C;
const OFF_GENDER: u32 = 0x34;
const OFF_SKIN_TONE: u32 = 0x38;
const OFF_EYE_COLOR: u32 = 0x3C;
const OFF_SHOES: u32 = 0x54;
const OFF_CLOTH: u32 = 0x70;
const OFF_HAT: u32 = 0x8C;
const OFF_TANK_ID: u32 = 0xA8;
const OFF_RANK: u32 = 0xAC;
const OFF_RANK_POINTS: u32 = 0xB0;
const OFF_FEST_TEAM: u32 = 0xB4;
const OFF_FEST_ID: u32 = 0xB8;
const OFF_FEST_GRADE: u32 = 0xBC;
const OFF_WEAPON_WORD: u32 = 0x41;
const OFF_PID: u32 = 0xD0;

const SESSION_ROOT_PTR: u32 = 0x101E8980;
const SESSION_INDEX_OFFSET: u32 = 0xBD;
const SESSION_ID_BASE_OFFSET: u32 = 0xCC;

fn weapon_name_map() -> HashMap<u16, &'static str> {
    use std::iter::FromIterator;

    HashMap::from_iter([
        // Hero Shot (Story Mode Weapons)
        (0x03EE, "?Shot_Msn0Lv3"),
        (0x03ED, "?Shot_Msn0Lv2"),
        (0x03EC, "?Shot_Msn0Lv1"),
        (0x03EB, "?Shot_Msn0Lv0"),
        (0x07D3, "?Roller_Mission"),
        (0x0FB0, "?Charge_Mission"),
        // Octoshot Rival (Octoling Weapons)
        (0x03F4, "?Shot_Rvl0Lv0"),
        (0x03EF, "?Shot_Rvl0Lv1"),
        (0x03F0, "?Shot_Rvl0Lv2"),
        (0x03F1, "?Shot_Rvl0Lv3"),
        // Developer Weapons
        (0x07D2, "?Roller_KingSquid"),
        // Test Weapons
        (0x0BCE, "?BigBall_SpecAdjust01"),
        (0x0BCF, "?BigBall_SpecAdjust02"),
        // Normal Weapons
        (0x03E8, "Sploosh-o-matic"),
        (0x03E9, "Neo Sploosh-o-matic"),
        (0x03EA, "Sploosh-o-matic 7"),
        (0x03F2, "Splattershot Jr. (Default)"),
        (0x03F3, "Custom Splattershot Jr."),
        (0x03FC, "Splash-o-matic"),
        (0x03FD, "Neo Splash-o-matic"),
        (0x0406, "Aerospray MG"),
        (0x0407, "Aerospray RG"),
        (0x0408, "Aerospray PG"),
        (0x0410, "Splattershot"),
        (0x0411, "Tentatek Splattershot"),
        (0x0412, "Wasabi Splattershot"),
        (0x0415, "Hero Shot Replica"),
        (0x0416, "Octoshot Replica"),
        (0x041A, ".52 Gal"),
        (0x041B, ".52 Gal Deco"),
        (0x0424, "N-ZAP '85"),
        (0x0425, "N-ZAP '89"),
        (0x0426, "N-ZAP '83"),
        (0x042E, "Splattershot Pro"),
        (0x042F, "Forge Splattershot Pro"),
        (0x0430, "Berry Splattershot Pro"),
        (0x0438, ".96 Gal"),
        (0x0439, ".96 Gal Deco"),
        (0x0442, "Dual Squelcher"),
        (0x0443, "Custom Dual Squelcher"),
        (0x044C, "Jet Squelcher"),
        (0x044D, "Custom Jet Squelcher"),
        (0x0456, "Luna Blaster"),
        (0x0457, "Luna Blaster Neo"),
        (0x0460, "Blaster"),
        (0x0461, "Custom Blaster"),
        (0x046A, "Range Blaster"),
        (0x046B, "Custom Range Blaster"),
        (0x046C, "Grim Range Blaster"),
        (0x0474, "Rapid Blaster"),
        (0x0475, "Rapid Blaster Deco"),
        (0x047E, "Rapid Blaster Pro"),
        (0x047F, "Rapid Blaster Pro Deco"),
        (0x0488, "L-3 Nozzlenose"),
        (0x0489, "L-3 Nozzlenose D"),
        (0x0496, "H-3 Nozzlenose"),
        (0x0497, "H-3 Nozzlenose D"),
        (0x0498, "Cherry H-3 Nozzlenose"),
        (0x07D0, "Carbon Roller"),
        (0x07D1, "Carbon Roller Deco"),
        (0x07DA, "Splat Roller"),
        (0x07DB, "Krak-On Splat Roller"),
        (0x07DC, "CoroCoro Splat Roller"),
        (0x07DF, "Hero Roller Replica"),
        (0x07E4, "Dynamo Roller"),
        (0x07E5, "Gold Dynamo Roller"),
        (0x07E6, "Tempered Dynamo Roller"),
        (0x07EE, "Inkbrush"),
        (0x07EF, "Inkbrush Nouveau"),
        (0x07F0, "Permanent Inkbrush"),
        (0x07F8, "Octobrush"),
        (0x07F9, "Octobrush Nouveau"),
        (0x0BB8, "Slosher"),
        (0x0BB9, "Slosher Deco"),
        (0x0BBA, "Soda Slosher"),
        (0x0BC2, "Tri-Slosher"),
        (0x0BC3, "Tri-Slosher Nouveau"),
        (0x0BCC, "Sloshing Machine"),
        (0x0BCD, "Sloshing Machine Neo"),
        (0x0FA0, "Classic Squiffer"),
        (0x0FA1, "New Squiffer"),
        (0x0FA2, "Fresh Squiffer"),
        (0x0FAA, "Splat Charger"),
        (0x0FAB, "Kelp Splat Charger"),
        (0x0FAC, "Bento Splat Charger"),
        (0x0FAF, "Hero Charger Replica"),
        (0x0FB4, "Splatterscope"),
        (0x0FB5, "Kelp Splatterscope"),
        (0x0FB6, "Bento Splatterscope"),
        (0x0FBE, "E-Liter 3K"),
        (0x0FBF, "Custom E-Liter 3K"),
        (0x0FC8, "E-Liter 3K Scope"),
        (0x0FC9, "Custom E-Liter 3K Scope"),
        (0x0FD2, "Bamboozler 14 Mk I"),
        (0x0FD3, "Bamboozler 14 Mk II"),
        (0x0FD4, "Bamboozler 14 Mk III"),
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerRecord {
    pub index: u8,

    pub name: String,
    pub pid_hex: String,
    pub pid_dec: u32,
    pub pnid: String,

    pub area: u32,
    pub gender: u8,
    pub skin_tone: u8,
    pub eye_color: u8,
    pub shoes: u32,
    pub cloth: u32,
    pub hat: u32,
    pub tank_id: u32,
    pub weapon_id: u16,
    pub weapon_name: String,
    pub rank: u8,
    pub rank_points: u32,
    pub fest_team: u32,
    pub fest_id: u32,
    pub fest_grade: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResult {
    pub players: Vec<PlayerRecord>,
    pub session_id: Option<u32>,
    pub fetched_at: DateTime<Local>,
}

fn get_pnid(pid: i32) -> String {
    let client = match Client::builder().user_agent("Mozilla/5.0").build() {
        Ok(c) => c,
        Err(_) => return "0".to_string(),
    };

    let url = format!("http://account.pretendo.cc/v1/api/miis?pids={}", pid);
    let response = match client
        .get(&url)
        .header("X-Nintendo-Client-ID", "a2efa818a34fa16b8afbc8a74eba3eda")
        .header(
            "X-Nintendo-Client-Secret",
            "c91cdb5658bd4954ade78533a339cf9a",
        )
        .send()
    {
        Ok(r) => r,
        Err(_) => return "0".to_string(),
    };

    if !response.status().is_success() {
        return "0".to_string();
    }

    let body = match response.text() {
        Ok(b) => b,
        Err(_) => return "0".to_string(),
    };

    let doc = match Document::parse(&body) {
        Ok(d) => d,
        Err(_) => return "0".to_string(),
    };

    doc.descendants()
        .find(|n| n.tag_name().name() == "user_id")
        .and_then(|n| n.text())
        .unwrap_or("0")
        .to_string()
}

fn decode_name(bytes: &[u8]) -> String {
    let mut name_chars = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let code = u16::from_be_bytes([chunk[0], chunk[1]]);
        if code == 0 {
            break;
        }
        name_chars.push(code);
    }

    String::from_utf16_lossy(&name_chars)
        .trim()
        .replace(['\n', '\r'], "")
}

pub fn fetch_all() -> Result<FetchResult> {
    let cemu_pid = find_cemu_process()?;
    let proc_mem = PlatformProcessMemory::new(cemu_pid)?;

    let weapon_map = weapon_name_map();

    let mut players: Vec<PlayerRecord> = Vec::new();

    let ptr1 = proc_mem.read_u32(PLAYER_ROOT_PTR)?;
    let ptr2 = proc_mem.read_u32(ptr1 + PLAYER_LIST_OFFSET)?;

    for i in 0..8 {
        let player_ptr = proc_mem.read_u32(ptr2 + (i * PLAYER_SLOT_STRIDE))?;

        if player_ptr == 0 {
            players.push(PlayerRecord {
                index: i as u8,
                pid_hex: "00000000".to_string(),
                pid_dec: 0,
                pnid: "0".to_string(),

                area: 0,
                gender: 0,
                skin_tone: 0,
                eye_color: 0,
                shoes: 0,
                cloth: 0,
                hat: 0,
                tank_id: 0,
                rank: 0,
                rank_points: 0,
                fest_team: 0,
                fest_id: 0,
                fest_grade: 0,
                weapon_id: 0,
                weapon_name: "Unknown".to_string(),

                name: "????????".to_string(),
            });
            continue;
        }

        let name_bytes = proc_mem.read_bytes(player_ptr + OFF_NAME, 32)?;
        let name = decode_name(&name_bytes);

        let pid_raw = proc_mem.read_u32(player_ptr + OFF_PID)?;
        let pid_bytes = pid_raw.to_le_bytes();
        let pid_hex = format!(
            "{:02X}{:02X}{:02X}{:02X}",
            pid_bytes[0], pid_bytes[1], pid_bytes[2], pid_bytes[3]
        );
        let pnid = get_pnid(pid_raw as i32);

        let area = proc_mem.read_u32(player_ptr + OFF_AREA)?;
        let gender = proc_mem.read_u32(player_ptr + OFF_GENDER)? as u8;
        let skin_tone = proc_mem.read_u32(player_ptr + OFF_SKIN_TONE)? as u8;
        let eye_color = proc_mem.read_u32(player_ptr + OFF_EYE_COLOR)? as u8;
        let shoes = proc_mem.read_u32(player_ptr + OFF_SHOES)?;
        let cloth = proc_mem.read_u32(player_ptr + OFF_CLOTH)?;
        let hat = proc_mem.read_u32(player_ptr + OFF_HAT)?;
        let tank_id = proc_mem.read_u32(player_ptr + OFF_TANK_ID)?;
        let rank = proc_mem.read_u32(player_ptr + OFF_RANK)? as u8;
        let rank_points = proc_mem.read_u32(player_ptr + OFF_RANK_POINTS)?;
        let fest_team = proc_mem.read_u32(player_ptr + OFF_FEST_TEAM)?;
        let fest_id = proc_mem.read_u32(player_ptr + OFF_FEST_ID)?;
        let fest_grade = proc_mem.read_u32(player_ptr + OFF_FEST_GRADE)?;

        let weapon_word = proc_mem.read_u32(player_ptr + OFF_WEAPON_WORD)?;
        let weapon_id = ((weapon_word >> 8) & 0xFFFF) as u16;

        let weapon_name = weapon_map
            .get(&weapon_id)
            .copied()
            .unwrap_or("Unknown")
            .to_string();

        players.push(PlayerRecord {
            index: i as u8,
            pid_hex,
            pid_dec: pid_raw,
            pnid,

            area,
            gender,
            skin_tone,
            eye_color,
            shoes,
            cloth,
            hat,
            tank_id,
            rank,
            rank_points,
            fest_team,
            fest_id,
            fest_grade,
            weapon_id,
            weapon_name,

            name,
        });
    }

    let ptr = proc_mem.read_u32(SESSION_ROOT_PTR)?;
    let session_id = if ptr != 0 {
        let index = proc_mem.read_u8(ptr + SESSION_INDEX_OFFSET)? as u32;
        let session_id = proc_mem.read_u32(ptr + index + SESSION_ID_BASE_OFFSET)?;
        Some(session_id)
    } else {
        None
    };

    let datetime = Local::now();

    Ok(FetchResult {
        players,
        session_id,
        fetched_at: datetime,
    })
}

fn main() -> Result<()> {
    gui::run_app()
}
