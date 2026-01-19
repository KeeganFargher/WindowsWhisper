use crate::history::TranscriptionHistory;
use crate::settings::Settings;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn hide_popup(window: tauri::Window) {
    let _ = window.hide();
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    settings.save()?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
pub fn get_history(state: State<AppState>) -> TranscriptionHistory {
    state.history.lock().unwrap().clone()
}

#[tauri::command]
pub fn clear_history(state: State<AppState>) -> Result<(), String> {
    let mut history = state.history.lock().unwrap();
    history.clear();
    Ok(())
}
