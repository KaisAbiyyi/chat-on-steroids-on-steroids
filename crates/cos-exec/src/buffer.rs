#[derive(Debug, Clone)]
pub struct HeadTail {
    max: usize,
    head: Vec<u8>,
    tail: std::collections::VecDeque<u8>,
    omitted: u64,
}
impl HeadTail {
    pub fn prepend(&mut self, before: &Self) {
        let after = std::mem::replace(self, Self::new(self.max));
        self.push(&before.head);
        self.push(&before.tail.iter().copied().collect::<Vec<_>>());
        self.omitted += before.omitted;
        self.push(&after.head);
        self.push(&after.tail.iter().copied().collect::<Vec<_>>());
        self.omitted += after.omitted;
    }
    pub fn is_empty(&self) -> bool {
        self.head.is_empty() && self.tail.is_empty() && self.omitted == 0
    }
    pub fn new(max: usize) -> Self {
        Self {
            max,
            head: vec![],
            tail: std::collections::VecDeque::new(),
            omitted: 0,
        }
    }
    pub fn push(&mut self, bytes: &[u8]) {
        let head_n = (self.max / 2 - self.head.len()).min(bytes.len());
        self.head.extend_from_slice(&bytes[..head_n]);
        let rest = &bytes[head_n..];
        let budget = self.max - self.max / 2;
        for byte in rest {
            if self.tail.len() == budget {
                if budget > 0 {
                    self.tail.pop_front();
                }
                self.omitted += 1;
            }
            if budget > 0 {
                self.tail.push_back(*byte);
            }
        }
    }
    pub fn snapshot(&self) -> String {
        let mut bytes = self.head.clone();
        if self.omitted > 0 {
            bytes.extend_from_slice(
                format!("\n... {} bytes omitted ...\n", self.omitted).as_bytes(),
            );
        }
        bytes.extend(self.tail.iter());
        String::from_utf8_lossy(&bytes).into_owned()
    }
    pub fn take(&mut self) -> String {
        let output = self.snapshot();
        self.head.clear();
        self.tail.clear();
        self.omitted = 0;
        output
    }
}
