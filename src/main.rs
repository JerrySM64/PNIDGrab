use anyhow::Result;
use chrono::{DateTime, Local};
use reqwest::blocking::Client;
use roxmltree::Document;
use serde::{Deserialize, Serialize};

mod gui;
mod id;
mod platform;

use id::{
    clothes_name, eye_color_name, headgear_name, rank_label, shoes_name, tank_name,
    weapon_name_main, weapon_name_special, weapon_name_sub,
};

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
const OFF_WEAPONSET: u32 = 0x40;
// const OFF_WEAPONID_MAIN: u32 = 0x44;
const OFF_WEAPONID_SUB: u32 = 0x48;
const OFF_WEAPONID_SPECIAL: u32 = 0x4C;
const OFF_WEAPONTURF_TOTAL: u32 = 0x50;
const OFF_PID: u32 = 0xD0;
const SESSION_ROOT_PTR: u32 = 0x101E8980;
const SESSION_INDEX_OFFSET: u32 = 0xBD;
const SESSION_ID_BASE_OFFSET: u32 = 0xCC;

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
    pub eye_color_name: String,
    pub headgear: u32,
    pub headgear_name: String,
    pub clothes: u32,
    pub clothes_name: String,
    pub shoes: u32,
    pub shoes_name: String,
    pub tank_id: u32,
    pub tank_name: String,

    pub weapon_set: u16,
    pub weapon_set_name: String,
    pub weapon_id_sub: u16,
    pub weapon_sub_name: String,
    pub weapon_id_special: u8,
    pub weapon_special_name: String,
    pub weaponturf_total: u32,

    /*
    pub weapon_id_main: u16,
    pub weapon_main_name: String,
    */
    pub rank: i8,
    pub rank_points: i8,
    pub rank_label: String,
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
    let mut out = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let code = u16::from_be_bytes([chunk[0], chunk[1]]);
        if code == 0 {
            break;
        }
        out.push(code);
    }

    String::from_utf16_lossy(&out)
        .trim()
        .replace(['\n', '\r'], "")
}

pub fn fetch_all() -> Result<FetchResult> {
    let cemu_pid = find_cemu_process()?;
    let proc_mem = PlatformProcessMemory::new(cemu_pid)?;

    let mut players = Vec::new();

    let root = proc_mem.read_u32(PLAYER_ROOT_PTR)?;
    let list = proc_mem.read_u32(root + PLAYER_LIST_OFFSET)?;

    for i in 0..8 {
        let player_ptr = proc_mem.read_u32(list + (i * PLAYER_SLOT_STRIDE))?;

        if player_ptr == 0 {
            players.push(PlayerRecord {
                index: i as u8,
                name: "????????".to_string(),
                pid_hex: "00000000".to_string(),
                pid_dec: 0,
                pnid: "0".to_string(),

                area: 0,
                gender: 0,
                skin_tone: 0,
                eye_color: 0,
                eye_color_name: "Unknown".to_string(),

                headgear: 0,
                headgear_name: "Unknown".to_string(),
                clothes: 0,
                clothes_name: "Unknown".to_string(),
                shoes: 0,
                shoes_name: "Unknown".to_string(),
                tank_id: 0,
                tank_name: "Unknown".to_string(),

                weapon_set: 0,
                weapon_set_name: "Unknown".to_string(),
                weapon_id_sub: 0,
                weapon_sub_name: "Unknown".to_string(),
                weapon_id_special: 0,
                weapon_special_name: "Unknown".to_string(),
                weaponturf_total: 0,

                /*
                weapon_id_main: 0,
                weapon_main_name: "Unknown".to_string(),
                */
                rank: 0,
                rank_points: 0,
                rank_label: "Unknown".to_string(),
                fest_team: 0,
                fest_id: 0,
                fest_grade: 0,
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
        let eye_color_name = eye_color_name(eye_color).to_string();
        let headgear = proc_mem.read_u32(player_ptr + OFF_HAT)?;
        let headgear_name = headgear_name(headgear).to_string();
        let clothes = proc_mem.read_u32(player_ptr + OFF_CLOTH)?;
        let clothes_name = clothes_name(clothes).to_string();
        let shoes = proc_mem.read_u32(player_ptr + OFF_SHOES)?;
        let shoes_name = shoes_name(shoes).to_string();
        let tank_id = proc_mem.read_u32(player_ptr + OFF_TANK_ID)?;
        let tank_name = tank_name(tank_id).to_string();

        let rank = proc_mem.read_u32(player_ptr + OFF_RANK)? as i8;
        let rank_points = proc_mem.read_u32(player_ptr + OFF_RANK_POINTS)? as i8;
        let rank_label = rank_label(rank_points).to_string();
        let fest_team = proc_mem.read_u32(player_ptr + OFF_FEST_TEAM)?;
        let fest_id = proc_mem.read_u32(player_ptr + OFF_FEST_ID)?;
        let fest_grade = proc_mem.read_u32(player_ptr + OFF_FEST_GRADE)?;

        /*
        let weapon_word = proc_mem.read_u32(player_ptr + OFF_WEAPON_WORD)?;
        let weapon_id = ((weapon_word >> 8) & 0xFFFF) as u16;

        let weapon_id_main = proc_mem.read_u32(player_ptr + OFF_WEAPONID_MAIN)? as u16;
        let weapon_main_name = weapon_name_main(weapon_id_main).to_string();
        */

        let weapon_set = proc_mem.read_u32(player_ptr + OFF_WEAPONSET)? as u16;
        let weapon_set_name = weapon_name_main(weapon_set).to_string();
        let weapon_id_sub = proc_mem.read_u32(player_ptr + OFF_WEAPONID_SUB)? as u16;
        let weapon_sub_name = weapon_name_sub(weapon_id_sub).to_string();
        let weapon_id_special = proc_mem.read_u32(player_ptr + OFF_WEAPONID_SPECIAL)? as u8;
        let weapon_special_name = weapon_name_special(weapon_id_special).to_string();
        let weaponturf_total = proc_mem.read_u32(player_ptr + OFF_WEAPONTURF_TOTAL)?;

        players.push(PlayerRecord {
            index: i as u8,

            name,
            pid_hex,
            pid_dec: pid_raw,
            pnid,

            area,
            gender,
            skin_tone,
            eye_color,
            eye_color_name,
            headgear,
            headgear_name,
            clothes,
            clothes_name,
            shoes,
            shoes_name,
            tank_id,
            tank_name,

            weapon_set,
            weapon_set_name,
            weapon_id_sub,
            weapon_sub_name,
            weapon_id_special,
            weapon_special_name,
            weaponturf_total,

            /*
            weapon_id_main,
            weapon_main_name,
            */
            rank,
            rank_points,
            rank_label,
            fest_team,
            fest_id,
            fest_grade,
        });
    }

    let root2 = proc_mem.read_u32(SESSION_ROOT_PTR)?;
    let session_id = if root2 != 0 {
        let idx = proc_mem.read_u8(root2 + SESSION_INDEX_OFFSET)? as u32;
        Some(proc_mem.read_u32(root2 + idx + SESSION_ID_BASE_OFFSET)?)
    } else {
        None
    };

    Ok(FetchResult {
        players,
        session_id,
        fetched_at: Local::now(),
    })
}

fn main() -> Result<()> {
    gui::run_app()
}
