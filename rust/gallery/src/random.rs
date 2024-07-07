const MODULUS: u64 = 2 << 32;
const MULTIPLIER: u64 = 1664525;
const INCREMENT: u64 = 1013904223;

#[derive(Default, Debug)]
pub struct Rng {
    pub seed: u64,
}

/// A simple linear congruential random number generator, as described in
/// https://en.wikipedia.org/wiki/Linear_congruential_generator.
///
/// The parameters for this RNG are taken from Numerical Recipes
/// by Knuth and H. W. Lewis.
impl Rng {
    pub fn new(seed: Option<u64>) -> Self {
        Rng {
            seed: seed.unwrap_or_else(|| seed_from_current_time()),
        }
    }

    pub fn random(&mut self) -> f64 {
        self.seed = (MULTIPLIER * self.seed + INCREMENT) % MODULUS;
        self.latest_random()
    }

    pub fn latest_random(&self) -> f64 {
        (self.seed as f64) / (MODULUS as f64)
    }

    pub fn shuffle<T>(&mut self, array: &mut [T]) {
        for i in 0..array.len() {
            let target = (self.random() * array.len() as f64).floor() as usize;
            array.swap(i, target);
        }
    }
}

impl Iterator for Rng {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.random())
    }
}

fn seed_from_current_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
