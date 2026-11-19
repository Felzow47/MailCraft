//! Compressed RTF ([MS-OXRTFCP]): the LZ77 variant Outlook uses for `PR_RTF_COMPRESSED`.

const PREBUF: &[u8] = b"{\\rtf1\\ansi\\mac\\deff0\\deftab720{\\fonttbl;}{\\f0\\fnil \\froman \\fswiss \\fmodern \\fscript \\fdecor MS Sans SerifSymbolArialTimes New RomanCourier{\\colortbl\\red0\\green0\\blue0\r\n\\par \\pard\\plain\\f0\\fs20\\b\\i\\u\\tab\\tx";

const COMPRESSED: u32 = 0x7546_5A4C; // "LZFu"
const UNCOMPRESSED: u32 = 0x414C_454D; // "MELA"

pub fn decompress(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 16 {
        return None;
    }
    let u32_at = |i: usize| u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
    let raw_size = u32_at(4) as usize;
    match u32_at(8) {
        UNCOMPRESSED => return Some(data[16..].iter().take(raw_size).copied().collect()),
        COMPRESSED => {}
        _ => return None,
    }
    let mut dict = [0u8; 4096];
    dict[..PREBUF.len()].copy_from_slice(PREBUF);
    let mut w = PREBUF.len();
    let mut out = Vec::with_capacity(raw_size);
    let mut i = 16;
    while i < data.len() {
        let flags = data[i];
        i += 1;
        for bit in 0..8 {
            if i >= data.len() {
                break;
            }
            if flags & (1 << bit) == 0 {
                let c = data[i];
                i += 1;
                out.push(c);
                dict[w] = c;
                w = (w + 1) & 4095;
            } else {
                if i + 1 >= data.len() {
                    return Some(out);
                }
                let r = (data[i] as usize) << 8 | data[i + 1] as usize;
                i += 2;
                let off = r >> 4;
                let len = (r & 15) + 2;
                if off == w {
                    return Some(out);
                }
                for k in 0..len {
                    let c = dict[(off + k) & 4095];
                    out.push(c);
                    dict[w] = c;
                    w = (w + 1) & 4095;
                }
            }
        }
    }
    Some(out)
}
