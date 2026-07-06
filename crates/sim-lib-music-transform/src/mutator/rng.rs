#[derive(Clone, Debug)]
pub(super) struct PatternRng {
    state: u64,
}

impl PatternRng {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    pub(super) fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    pub(super) fn range(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next_u64() as usize) % upper
        }
    }

    pub(super) fn chance(&mut self, percent: u8) -> bool {
        percent >= 100 || (percent > 0 && self.range(100) < usize::from(percent))
    }

    pub(super) fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let swap_with = self.range(index + 1);
            values.swap(index, swap_with);
        }
    }
}
