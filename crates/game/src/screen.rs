//! Piezas comunes de las pantallas de menú.

use winit::keyboard::KeyCode;

/// Resultado de una pulsación en una pantalla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Stay,
    /// Volver al menú.
    Back,
}

/// Edición de una línea de texto (`CB_EnterText`): cursor, borrar, inicio/fin; Enter acepta y
/// Esc cancela.
#[derive(Clone, Debug)]
pub struct TextEdit {
    text: Vec<char>,
    cursor: usize,
}

pub enum EditResult {
    Editing,
    Accepted(String),
    Cancelled,
}

impl TextEdit {
    pub fn new(initial: &str) -> Self {
        let text: Vec<char> = initial.chars().collect();
        TextEdit {
            cursor: text.len(),
            text,
        }
    }

    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    /// `max` limita la longitud para que quepa en su casilla.
    pub fn key(&mut self, code: KeyCode, typed: Option<&str>, max: usize) -> EditResult {
        match code {
            KeyCode::Enter | KeyCode::NumpadEnter => return EditResult::Accepted(self.text()),
            KeyCode::Escape => return EditResult::Cancelled,
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.text.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.text.len() {
                    self.text.remove(self.cursor);
                }
            }
            KeyCode::ArrowLeft => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::ArrowRight => self.cursor = (self.cursor + 1).min(self.text.len()),
            KeyCode::ArrowUp | KeyCode::Home | KeyCode::PageUp => self.cursor = 0,
            KeyCode::ArrowDown | KeyCode::End | KeyCode::PageDown => self.cursor = self.text.len(),
            _ => {
                for c in typed.unwrap_or("").chars().filter(|c| !c.is_control()) {
                    if self.text.len() < max {
                        self.text.insert(self.cursor, c);
                        self.cursor += 1;
                    }
                }
            }
        }
        EditResult::Editing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_editing_and_accepting() {
        let mut e = TextEdit::new("Ana");
        e.key(KeyCode::KeyX, Some("x"), 10);
        assert_eq!(e.text(), "Anax");
        e.key(KeyCode::Backspace, None, 10);
        e.key(KeyCode::Home, None, 10);
        e.key(KeyCode::KeyZ, Some("Z"), 10);
        assert_eq!(e.text(), "ZAna");
        e.key(KeyCode::Delete, None, 10);
        assert_eq!(e.text(), "Zna");
        assert!(matches!(e.key(KeyCode::Enter, None, 10), EditResult::Accepted(s) if s == "Zna"));
    }

    #[test]
    fn length_is_limited_and_escape_cancels() {
        let mut e = TextEdit::new("abc");
        e.key(KeyCode::KeyD, Some("def"), 4);
        assert_eq!(e.text(), "abcd");
        assert!(matches!(
            e.key(KeyCode::Escape, None, 4),
            EditResult::Cancelled
        ));
    }

    #[test]
    fn accents_are_accepted() {
        let mut e = TextEdit::new("");
        e.key(KeyCode::KeyN, Some("ñ"), 8);
        assert_eq!(e.text(), "ñ");
    }
}
