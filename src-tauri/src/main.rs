// Do not show a console window on Windows
#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use std::io::Write;
use tauri::{
  api::{file, notification::Notification, path},
  Manager,
};
extern crate chrono;
extern crate timer;

#[derive(Serialize, Deserialize)]
struct Credentials {
  username: String,
  password: String,
}

fn save_creds(creds: Credentials, save_file: &std::path::Path) {
  let mut file = std::fs::File::create(&save_file).unwrap();
  write!(&mut file, "{}", serde_json::to_string(&creds).unwrap()).unwrap();
}

fn load_creds(save_file: &std::path::Path) -> Result<Credentials, String> {
  let creds_string = file::read_string(save_file);
  if creds_string.is_ok() {
    let creds: Credentials = serde_json::from_str(&creds_string.unwrap()).unwrap();
    return Ok(creds);
  } else {
    return Err("Credentials not saved".to_string());
  }
}

static mut PROCEED_CAMPNET_ATTEMPT: bool = false;
static mut LOGOUT_CAMPNET: bool = false;

unsafe fn connect_campnet(file_path: &std::path::PathBuf) {
  if PROCEED_CAMPNET_ATTEMPT {
    let campnet_status = reqwest::blocking::get("https://campnet.bits-goa.ac.in:8090/");
    if campnet_status.is_ok() {
      let login_status = reqwest::blocking::get("https://www.google.com");
      if login_status.is_err() {
        let helper_file = file_path.parent().unwrap().join("credentials.json");
        let creds = load_creds(&helper_file);
        if creds.is_ok() {
          let creds = creds.unwrap();
          let body: String = format!(
            "mode=191&username={}&password={}&a={}&producttype=1",
            creds.username,
            creds.password,
            std::time::SystemTime::now()
              .duration_since(std::time::UNIX_EPOCH)
              .unwrap()
              .as_millis()
          );
          let client = reqwest::blocking::Client::new();
          let res = client
            .post("https://campnet.bits-goa.ac.in:8090/login.xml")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Content-Length", body.chars().count())
            .body(body)
            .send();
          if res.is_ok() {
            let res_body: String = res.unwrap().text().unwrap();
            if res_body.contains("LIVE") {
              Notification::new("com.riskycase.autocampnet")
                .title("Connected to Campnet!")
                .body("Logged in successfully to BPGC network")
                .show()
                .unwrap();
            } else if res_body.contains("failed") {
              Notification::new("com.riskycase.autocampnet")
                .title("Could not connect to Campnet!")
                .body("Incorrect credentials were provided")
                .show()
                .unwrap();
              PROCEED_CAMPNET_ATTEMPT = false;
            } else if res_body.contains("exceeded") {
              Notification::new("com.riskycase.autocampnet")
                .title("Could not connect to Campnet!")
                .body("Daily data limit exceeded on credentials")
                .show()
                .unwrap();
              PROCEED_CAMPNET_ATTEMPT = false
            } else {
              Notification::new("com.riskycase.autocampnet")
                .title("Could not to Campnet!")
                .body("There was an issue with the login attempt")
                .show()
                .unwrap();
              PROCEED_CAMPNET_ATTEMPT = false;
            }
          }
        }
      }
    }
  }
  if LOGOUT_CAMPNET {
    let helper_file = file_path.parent().unwrap().join("credentials.json");
    let creds = load_creds(&helper_file);
    if creds.is_ok() {
      let creds = creds.unwrap();
      let body: String = format!(
        "mode=193&username={}&a={}&producttype=1",
        creds.username,
        std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .unwrap()
          .as_millis()
      );
      let client = reqwest::blocking::Client::new();
      let res = client
        .post("https://campnet.bits-goa.ac.in:8090/logout.xml")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Content-Length", body.chars().count())
        .body(body)
        .send();
      if res.is_ok() {
        let res_body: String = res.unwrap().text().unwrap();
        if res_body.contains("LOGIN") {
          Notification::new("com.riskycase.autocampnet")
            .title("Logged out of Campnet")
            .show()
            .unwrap();
        }
      }
      LOGOUT_CAMPNET = false;
    }
  }

  let callback_timer = timer::Timer::new();
  let callback_path = file_path.parent().unwrap().join("credentials.json");
  let _callback_gaurd =
    callback_timer.schedule_with_delay(chrono::Duration::milliseconds(2500), move || {
      connect_campnet(&callback_path);
    });
  std::thread::sleep(std::time::Duration::from_millis(3000));
}

fn main() {
  unsafe {
    let tray_menu = tauri::SystemTrayMenu::new()
      .add_item(tauri::CustomMenuItem::new("show", "Show window"))
      .add_native_item(tauri::SystemTrayMenuItem::Separator)
      .add_item(tauri::CustomMenuItem::new("reconnect", "Force reconnect"))
      .add_item(tauri::CustomMenuItem::new("logout", "Logout"))
      .add_item(tauri::CustomMenuItem::new("delete", "Delete credentials"))
      .add_native_item(tauri::SystemTrayMenuItem::Separator)
      .add_item(tauri::CustomMenuItem::new("quit", "Quit"));
    let system_tray = tauri::SystemTray::new().with_menu(tray_menu);
    tauri::Builder::default()
      .setup(|app: &mut tauri::App| {
        let save_dir = path::app_dir(&app.config()).unwrap();
        let file_creds = load_creds(&(save_dir.join("credentials.json")));
        if file_creds.is_ok() {
          let _creds = file_creds.unwrap();
          PROCEED_CAMPNET_ATTEMPT = true;
        } else {
          app.get_window("main").unwrap().show().unwrap();
        }
        let write_save_file = save_dir.join("credentials.json");
        let app_handle_save = app.app_handle();
        app.listen_global("save", move |event: tauri::Event| {
          let creds_save: Credentials = serde_json::from_str(event.payload().unwrap()).unwrap();
          save_creds(creds_save, &write_save_file);
          PROCEED_CAMPNET_ATTEMPT = true;
          std::thread::sleep(std::time::Duration::from_millis(3000));
          app_handle_save.get_window("main").unwrap().hide().unwrap();
          Notification::new("com.riskycase.autocampnet")
            .title("Credentials saved to disk")
            .body("App will try to login to campnet whenever available")
            .show()
            .unwrap();
        });
        let app_handle_minimise = app.app_handle();
        app.listen_global("minimise", move |_event: tauri::Event| {
          app_handle_minimise
            .get_window("main")
            .unwrap()
            .hide()
            .unwrap();
        });
        let read_save_file = save_dir.join("credentials.json");
        connect_campnet(&read_save_file);
        std::fs::create_dir_all(save_dir).unwrap();
        Ok(())
      })
      .system_tray(system_tray)
      .on_system_tray_event(|app: &tauri::AppHandle, event| match event {
        tauri::SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
          "quit" => {
            std::process::exit(0);
          }
          "show" => {
            let window: tauri::Window = app.get_window("main").unwrap();
            let save_file = path::app_dir(&app.config())
              .unwrap()
              .join("credentials.json");
            let creds = load_creds(&save_file);
            window.emit("credentials", &creds).unwrap();
            window.show().unwrap();
            window.unminimize().unwrap();
          }
          "logout" => {
            LOGOUT_CAMPNET = true;
            PROCEED_CAMPNET_ATTEMPT = false;
          }
          "reconnect" => {
            let save_file = path::app_dir(&app.config())
              .unwrap()
              .join("credentials.json");
            let creds = load_creds(&save_file);
            if creds.is_ok() {
              if PROCEED_CAMPNET_ATTEMPT {
                connect_campnet(&save_file);
              }
              PROCEED_CAMPNET_ATTEMPT = true;
            } else {
              let window: tauri::Window = app.get_window("main").unwrap();
              window.show().unwrap();
            }
          }
          "delete" => {
            let save_file = path::app_dir(&app.config())
              .unwrap()
              .join("credentials.json");
            let creds = load_creds(&save_file);
            if creds.is_ok() {
              std::fs::remove_file(&save_file).unwrap();
            }
            PROCEED_CAMPNET_ATTEMPT = false;
          }
          _ => {}
        },
        _ => {}
      })
      .run(tauri::generate_context!())
      .expect("error while running tauri application");
  }
}
