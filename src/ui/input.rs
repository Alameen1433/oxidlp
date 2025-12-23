use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::app::App;
use crate::events::AppEvent;

pub fn handle_key(key: KeyEvent, app: &mut App) -> Option<AppEvent> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(AppEvent::Quit);
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

    if app.input_mode {
        handle_input_mode(key, app)
    } else {
        handle_normal_mode(key, app)
    }
}

fn handle_input_mode(key: KeyEvent, app: &mut App) -> Option<AppEvent> {
    match key.code {
        KeyCode::Enter => {
            if !app.input_buffer.is_empty() {
                let url = std::mem::take(&mut app.input_buffer);
                return Some(AppEvent::AddUrl(url));
            } else if !app.jobs.is_empty() {
                return Some(AppEvent::StartDownload);
            }
        }
        KeyCode::Char(c) => {
            app.input_buffer.push(c);
        }
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Esc => {
            app.input_mode = false;
        }
        KeyCode::Up | KeyCode::Down => {
            app.input_mode = false;
            return handle_normal_mode(key, app);
        }
        _ => {}
    }
    None
}

fn handle_normal_mode(key: KeyEvent, app: &mut App) -> Option<AppEvent> {
    match key.code {
        KeyCode::Char('q') => Some(AppEvent::Quit),
        KeyCode::Char('?') => Some(AppEvent::ToggleHelp),
        KeyCode::Char('j') | KeyCode::Down => Some(AppEvent::SelectNext),
        KeyCode::Char('k') | KeyCode::Up => Some(AppEvent::SelectPrev),
        KeyCode::Char('d') => {
            app.selected_job().map(|j| AppEvent::RemoveJob(j.id))
        }
        KeyCode::Char('c') => {
            app.selected_job().map(|j| AppEvent::CancelJob(j.id))
        }
        KeyCode::Char('i') | KeyCode::Char('/') => {
            app.input_mode = true;
            None
        }
        KeyCode::Enter => Some(AppEvent::StartDownload),
        _ => None,
    }
}
