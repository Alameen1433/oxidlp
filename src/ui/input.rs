use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::events::AppEvent;

pub fn handle_key(key: KeyEvent, app: &mut App) -> Option<AppEvent> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(AppEvent::Quit);
    }

    if app.confirm_quit {
        return match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(AppEvent::ConfirmQuit),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(AppEvent::CancelQuit),
            _ => None,
        };
    }

    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                app.show_help = false;
            }
            _ => {}
        }
        return None;
    }

    if app.format_popup.is_some() {
        return handle_format_popup(key);
    }

    if app.settings_popup.is_some() {
        return handle_settings_popup(key, app);
    }

    if key.code == KeyCode::Tab {
        return Some(AppEvent::ToggleInputMode);
    }

    if app.input_mode {
        handle_input_mode(key, app)
    } else {
        handle_queue_mode(key, app)
    }
}

fn handle_format_popup(key: KeyEvent) -> Option<AppEvent> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(AppEvent::FormatSelectNext),
        KeyCode::Char('k') | KeyCode::Up => Some(AppEvent::FormatSelectPrev),
        KeyCode::Char('a') => Some(AppEvent::ToggleAudioOnly),
        KeyCode::Char('A') => Some(AppEvent::ToggleApplyToAll),
        KeyCode::Enter => Some(AppEvent::ConfirmFormat),
        KeyCode::Esc | KeyCode::Char('q') => Some(AppEvent::CloseFormatPopup),
        _ => None,
    }
}

fn handle_input_mode(key: KeyEvent, app: &mut App) -> Option<AppEvent> {
    match key.code {
        KeyCode::Enter => {
            if !app.input_buffer.is_empty() {
                let url = std::mem::take(&mut app.input_buffer);
                return Some(AppEvent::AddUrl(url));
            }
            None
        }
        KeyCode::Backspace => {
            app.input_buffer.pop();
            None
        }
        KeyCode::Esc => {
            app.input_mode = false;
            None
        }
        KeyCode::Char(c) => {
            app.input_buffer.push(c);
            None
        }
        _ => None,
    }
}

fn handle_queue_mode(key: KeyEvent, app: &mut App) -> Option<AppEvent> {
    match key.code {
        KeyCode::Char('q') => Some(AppEvent::Quit),
        KeyCode::Char('?') => Some(AppEvent::ToggleHelp),
        KeyCode::Char('g') => Some(AppEvent::ToggleSettings),
        KeyCode::Char('j') | KeyCode::Down => Some(AppEvent::SelectNext),
        KeyCode::Char('k') | KeyCode::Up => Some(AppEvent::SelectPrev),
        KeyCode::Enter => {
            if let Some(job) = app.selected_job() {
                if job.can_select_format() {
                    return Some(AppEvent::OpenFormatPopup);
                }
            }
            None
        }
        KeyCode::Char('s') => Some(AppEvent::StartDownloads),
        KeyCode::Char('S') => Some(AppEvent::ToggleSysInfo),
        KeyCode::Char('d') => app.selected_job().map(|j| AppEvent::RemoveJob(j.id)),
        KeyCode::Char('c') => app.selected_job().map(|j| AppEvent::CancelJob(j.id)),
        KeyCode::Char('i') | KeyCode::Char('/') => {
            app.input_mode = true;
            None
        }
        _ => None,
    }
}

fn handle_settings_popup(key: KeyEvent, app: &App) -> Option<AppEvent> {
    let settings = app.settings_popup.as_ref()?;
    
    if settings.editing_path {
        return match key.code {
            KeyCode::Esc | KeyCode::Enter => Some(AppEvent::SettingsToggleEdit),
            KeyCode::Backspace => Some(AppEvent::SettingsBackspace),
            KeyCode::Char(c) => Some(AppEvent::SettingsCharInput(c)),
            _ => None,
        };
    }
    
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(AppEvent::SettingsNext),
        KeyCode::Char('k') | KeyCode::Up => Some(AppEvent::SettingsPrev),
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Char('+') => Some(AppEvent::SettingsIncrement),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('-') => Some(AppEvent::SettingsDecrement),
        KeyCode::Enter => {
            if settings.selected_field == 0 {
                Some(AppEvent::SaveSettings)
            } else {
                Some(AppEvent::SettingsToggleEdit)
            }
        }
        KeyCode::Char('s') => Some(AppEvent::SaveSettings),
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('g') => Some(AppEvent::CloseSettings),
        _ => None,
    }
}
