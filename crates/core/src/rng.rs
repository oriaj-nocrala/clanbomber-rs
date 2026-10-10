//! Generador pseudoaleatorio determinista.
//!
//! Misma recurrencia que `fast_random()` del original (WELL512, 16 palabras de estado),
//! pero con aritmética de 32 bits explícita y semilla fija: así una partida se puede
//! reproducir. (En el original el estado se siembra con `rand()`, y `unsigned long`
//! es de 64 bits en Linux, de modo que su secuencia exacta no es reproducible.)

#[derive(Clone, Debug)]
pub struct Rng {
    state: [u32; 16],
    idx: usize,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // splitmix64 para llenar el estado.
        let mut s = seed;
        let mut next = move || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)) as u32
        };
        let mut state = [0u32; 16];
        for w in &mut state {
            *w = next();
        }
        Rng { state, idx: 0 }
    }

    pub fn next_u32(&mut self) -> u32 {
        let st = &mut self.state;
        let a = st[self.idx];
        let mut c = st[(self.idx + 13) & 15];
        let b = a ^ c ^ (a << 16) ^ (c << 15);
        c = st[(self.idx + 9) & 15];
        c ^= c >> 11;
        let a = b ^ c;
        st[self.idx] = a;
        let d = a ^ ((a << 5) & 0xDA44_2D24);
        self.idx = (self.idx + 15) & 15;
        let a = st[self.idx];
        st[self.idx] = a ^ b ^ d ^ (a << 2) ^ (b << 18) ^ (c << 28);
        st[self.idx]
    }

    /// Equivale a `fast_random() % n`.
    pub fn below(&mut self, n: u32) -> u32 {
        self.next_u32() % n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
        assert_ne!(Rng::new(1).next_u32(), Rng::new(2).next_u32());
    }

    #[test]
    fn roughly_uniform_mod_8() {
        let mut r = Rng::new(7);
        let mut hist = [0u32; 8];
        for _ in 0..80_000 {
            hist[r.below(8) as usize] += 1;
        }
        for h in hist {
            assert!((9_000..11_000).contains(&h), "{hist:?}");
        }
    }
}
