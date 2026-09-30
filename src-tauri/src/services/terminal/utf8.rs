//! 增量 UTF-8 解码：ConPTY 输出按 chunk 到达，多字节序列可能跨 chunk 劈开。
//!
//! 每轮取最长合法前缀解码输出；末尾残缺序列（≤3 字节）扣留到下一轮；
//! 确定性非法序列以 U+FFFD 损耗（GBK 程序输出为已知限制，同任何 UTF-8 终端）。

#[derive(Debug, Default)]
pub struct Utf8Accumulator {
    pending: Vec<u8>,
}

impl Utf8Accumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂入一段字节，返回本轮可完整解码的文本（可能为空）。
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(s) => {
                    out.push_str(s);
                    self.pending.clear();
                    return out;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    if valid > 0 {
                        out.push_str(&String::from_utf8_lossy(&self.pending[..valid]));
                        self.pending.drain(..valid);
                    }
                    match e.error_len() {
                        // 确定性非法序列：以 U+FFFD 损耗后继续处理剩余
                        Some(invalid_len) => {
                            out.push('\u{FFFD}');
                            let take = invalid_len.min(self.pending.len());
                            self.pending.drain(..take);
                        }
                        // 末尾不完整序列（≤3 字节）：扣留到下一轮
                        None => return out,
                    }
                }
            }
        }
    }

    /// 无扣留残余（单测断言用）。
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passes_through() {
        let mut acc = Utf8Accumulator::new();
        assert_eq!(acc.push(b"hello"), "hello");
        assert!(acc.is_empty());
    }

    #[test]
    fn multibyte_split_across_chunks() {
        // "中" = E4 B8 AD，劈成三段各 1 字节
        let mut acc = Utf8Accumulator::new();
        assert_eq!(acc.push(&[0xE4]), "");
        assert_eq!(acc.push(&[0xB8]), "");
        assert_eq!(acc.push(&[0xAD]), "中");
        assert!(acc.is_empty());
    }

    #[test]
    fn multibyte_split_with_leading_ascii() {
        let mut acc = Utf8Accumulator::new();
        assert_eq!(acc.push(b"a\xE4\xB8"), "a");
        assert!(!acc.is_empty());
        assert_eq!(acc.push(&[0xAD, b'b']), "中b");
    }

    #[test]
    fn invalid_sequence_replaced_and_recovers() {
        let mut acc = Utf8Accumulator::new();
        // 0xFF 非法；随后 "文"（E6 96 87）应正常解码
        let text = acc.push(&[0xFF, 0xE6, 0x96, 0x87]);
        assert_eq!(text, "\u{FFFD}文");
        assert!(acc.is_empty());
    }

    #[test]
    fn gbk_stream_does_not_grow_pending_forever() {
        // 连续 GBK 字节流（每个双字节序列在 UTF-8 下均非法）应被持续损耗，
        // pending 不无限增长。流末恰好停在残缺前导字节上会被合法扣留，
        // 追加一个 ASCII 字节使其终结（真实场景下一 chunk 到来即解）。
        let mut acc = Utf8Accumulator::new();
        let gbk: Vec<u8> = [0xD6, 0xD0, 0xCE, 0xC4]
            .iter()
            .cycle()
            .copied()
            .take(10_000)
            .collect();
        let text = acc.push(&gbk);
        assert!(text.chars().count() >= 9_999); // 全部以替换符或合法字节形式出栈
        let tail = acc.push(b"x");
        assert!(tail.ends_with('x'));
        assert!(acc.is_empty());
    }
}
