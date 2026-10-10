//! Mandos (ClanBomber2: Controller_Joystick.cpp). El joystick N es el N-ésimo mando conectado;
//! el stick izquierdo (o la cruceta) mueve y el botón principal (A / sur) pone la bomba.

use cb_core::Input;
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};

/// Zona muerta del original: 12000 de 32767.
const TOLERANCE: f32 = 12_000.0 / 32_767.0;

/// Los eventos de mando que nos importan, sin los detalles de gilrs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadEvent {
    Axis(Axis, f32),
    Press(Button),
    Release(Button),
}

impl PadEvent {
    fn from_gilrs(ev: &EventType) -> Option<PadEvent> {
        match *ev {
            EventType::AxisChanged(a, v, _) => Some(PadEvent::Axis(a, v)),
            EventType::ButtonPressed(b, _) => Some(PadEvent::Press(b)),
            EventType::ButtonReleased(b, _) => Some(PadEvent::Release(b)),
            _ => None,
        }
    }
}

/// Estado acumulado de un mando a partir de sus eventos.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PadState {
    /// Stick izquierdo; en gilrs "arriba" es positivo.
    x: f32,
    y: f32,
    /// Cruceta como botones (izquierda, derecha, arriba, abajo)...
    dpad: [bool; 4],
    /// ...o como eje (`DPadX`, `DPadY`).
    dpad_axis: (f32, f32),
    /// Pulsación de bomba pendiente: el original solo actúa al bajar el botón, no al mantenerlo.
    bomb_latch: bool,
}

impl PadState {
    pub fn apply(&mut self, ev: PadEvent) {
        match ev {
            PadEvent::Axis(Axis::LeftStickX, v) => self.x = v,
            PadEvent::Axis(Axis::LeftStickY, v) => self.y = v,
            PadEvent::Axis(Axis::DPadX, v) => self.dpad_axis.0 = v,
            PadEvent::Axis(Axis::DPadY, v) => self.dpad_axis.1 = v,
            PadEvent::Press(Button::South) => self.bomb_latch = true,
            PadEvent::Press(b) => self.set_dpad(b, true),
            PadEvent::Release(b) => self.set_dpad(b, false),
            PadEvent::Axis(..) => {}
        }
    }

    fn set_dpad(&mut self, b: Button, down: bool) {
        match b {
            Button::DPadLeft => self.dpad[0] = down,
            Button::DPadRight => self.dpad[1] = down,
            Button::DPadUp => self.dpad[2] = down,
            Button::DPadDown => self.dpad[3] = down,
            _ => {}
        }
    }

    /// Las teclas "pulsadas" ahora; consume la pulsación de bomba pendiente.
    pub fn input(&mut self) -> Input {
        let bomb = std::mem::take(&mut self.bomb_latch);
        Input {
            left: self.x < -TOLERANCE || self.dpad[0] || self.dpad_axis.0 < -0.5,
            right: self.x > TOLERANCE || self.dpad[1] || self.dpad_axis.0 > 0.5,
            up: self.y > TOLERANCE || self.dpad[2] || self.dpad_axis.1 > 0.5,
            down: self.y < -TOLERANCE || self.dpad[3] || self.dpad_axis.1 < -0.5,
            bomb,
        }
    }
}

/// Todos los mandos conectados, en el orden en que aparecieron.
pub struct Joysticks {
    gilrs: Option<Gilrs>,
    pads: Vec<(GamepadId, PadState)>,
}

impl Joysticks {
    pub fn new() -> Self {
        let gilrs = Gilrs::new()
            .map_err(|e| eprintln!("sin soporte de mandos: {e}"))
            .ok();
        let pads = gilrs
            .as_ref()
            .map(|g| {
                g.gamepads()
                    .map(|(id, _)| (id, PadState::default()))
                    .collect()
            })
            .unwrap_or_default();
        Joysticks { gilrs, pads }
    }

    /// Procesa los eventos pendientes (conexiones, ejes, botones). Llamar cada fotograma.
    pub fn update(&mut self) {
        let Some(g) = self.gilrs.as_mut() else { return };
        while let Some(ev) = g.next_event() {
            match ev.event {
                EventType::Connected => {
                    if !self.pads.iter().any(|(id, _)| *id == ev.id) {
                        self.pads.push((ev.id, PadState::default()));
                    }
                }
                EventType::Disconnected => self.pads.retain(|(id, _)| *id != ev.id),
                other => {
                    if !self.pads.iter().any(|(id, _)| *id == ev.id) {
                        self.pads.push((ev.id, PadState::default()));
                    }
                    if let (Some(pe), Some((_, st))) = (
                        PadEvent::from_gilrs(&other),
                        self.pads.iter_mut().find(|(id, _)| *id == ev.id),
                    ) {
                        st.apply(pe);
                    }
                }
            }
        }
    }

    /// Cuántos mandos hay (cuántos `Joystick(n)` se pueden elegir).
    pub fn count(&self) -> usize {
        self.pads.len()
    }

    /// Teclas del mando `index` (0 = el primero); sin mando, nada pulsado.
    pub fn input(&mut self, index: usize) -> Input {
        self.pads
            .get_mut(index)
            .map_or_else(Input::default, |(_, st)| st.input())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn axis(a: Axis, v: f32) -> PadEvent {
        PadEvent::Axis(a, v)
    }

    #[test]
    fn the_stick_has_a_dead_zone() {
        let mut p = PadState::default();
        p.apply(axis(Axis::LeftStickX, 0.3));
        assert!(!p.input().right, "dentro de la zona muerta");
        p.apply(axis(Axis::LeftStickX, 0.4));
        let i = p.input();
        assert!(i.right && !i.left);
        p.apply(axis(Axis::LeftStickX, -0.9));
        let i = p.input();
        assert!(i.left && !i.right);
    }

    #[test]
    fn up_is_positive_in_gilrs_and_down_is_negative() {
        let mut p = PadState::default();
        p.apply(axis(Axis::LeftStickY, 0.8));
        assert!(p.input().up);
        p.apply(axis(Axis::LeftStickY, -0.8));
        let i = p.input();
        assert!(i.down && !i.up);
        p.apply(axis(Axis::LeftStickY, 0.0));
        let i = p.input();
        assert!(!i.up && !i.down);
    }

    #[test]
    fn the_dpad_works_as_buttons_or_as_an_axis() {
        let mut p = PadState::default();
        p.apply(PadEvent::Press(Button::DPadLeft));
        assert!(p.input().left);
        p.apply(PadEvent::Release(Button::DPadLeft));
        assert!(!p.input().left);
        p.apply(axis(Axis::DPadY, -1.0));
        assert!(p.input().down);
    }

    #[test]
    fn the_bomb_button_acts_once_per_press() {
        let mut p = PadState::default();
        p.apply(PadEvent::Press(Button::South));
        assert!(p.input().bomb);
        assert!(!p.input().bomb, "mantenerlo no repite");
        p.apply(PadEvent::Release(Button::South));
        assert!(!p.input().bomb);
        p.apply(PadEvent::Press(Button::South));
        assert!(p.input().bomb, "una nueva pulsación vuelve a actuar");
    }

    #[test]
    fn other_buttons_do_nothing() {
        let mut p = PadState::default();
        p.apply(PadEvent::Press(Button::East));
        assert_eq!(p.input(), Input::default());
    }
}
