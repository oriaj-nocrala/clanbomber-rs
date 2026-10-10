//! Enfermedades (ClanBomber2: Disease*.cpp). Cada una es un efecto temporal sobre el
//! control o la velocidad del bomber.

use crate::extra::ExtraKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiseaseKind {
    /// Koks: velocidad +600 durante 10 s.
    Fast,
    /// Viagra: pone bombas sin parar durante 10 s.
    PutBomb,
    /// Joint: invierte los controles durante 10 s.
    Stoned,
    /// Congela los controles 3 s. Ningún extra la provoca en el original.
    Frozen,
}

impl DiseaseKind {
    /// `Disease::start()`: duración en segundos.
    pub fn duration(self) -> f32 {
        match self {
            DiseaseKind::Frozen => 3.0,
            _ => 10.0,
        }
    }

    /// `Disease::spawn_extra()`: el extra que se suelta al curarse o morir.
    pub fn extra(self) -> ExtraKind {
        match self {
            DiseaseKind::Fast => ExtraKind::Koks,
            DiseaseKind::Stoned => ExtraKind::Joint,
            DiseaseKind::PutBomb | DiseaseKind::Frozen => ExtraKind::Viagra,
        }
    }
}

impl ExtraKind {
    /// La enfermedad que contagia el extra al recogerlo (`Extra_*::effect`), si es de las malas.
    pub fn disease(self) -> Option<DiseaseKind> {
        match self {
            ExtraKind::Joint => Some(DiseaseKind::Stoned),
            ExtraKind::Viagra => Some(DiseaseKind::PutBomb),
            ExtraKind::Koks => Some(DiseaseKind::Fast),
            _ => None,
        }
    }
}

/// Enfermedad en curso; al llegar `countdown` a cero se cura.
#[derive(Clone, Copy, Debug)]
pub struct Disease {
    pub kind: DiseaseKind,
    pub countdown: f32,
}
