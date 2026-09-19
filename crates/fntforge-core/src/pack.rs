use crate::{Error, Result};

#[derive(Clone, Debug)]
pub struct PackOptions {
    pub max_size: u32,
    pub padding: u32,
    pub spacing: u32,
    pub power_of_two: bool,
    pub square: bool,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            max_size: 2048,
            padding: 1,
            spacing: 1,
            power_of_two: false,
            square: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PackedGlyph {
    pub index: usize,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub page: u32,
}

#[derive(Clone, Debug)]
pub struct PackedPage {
    pub width: u32,
    pub height: u32,
}

/// Row/shelf packer, no rotation. Overflows to extra pages.
pub fn pack_glyphs(sizes: &[(u32, u32)], opt: &PackOptions) -> Result<(Vec<PackedGlyph>, Vec<PackedPage>)> {
    if sizes.is_empty() {
        return Err(Error::Empty);
    }
    let gap = opt.spacing.max(1);
    for &(w, h) in sizes {
        if w + gap > opt.max_size || h + gap > opt.max_size {
            return Err(Error::AtlasTooSmall(opt.max_size));
        }
    }

    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(sizes[i].1));

    let mut glyphs = vec![
        PackedGlyph {
            index: 0,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            page: 0
        };
        sizes.len()
    ];
    let mut pages = Vec::new();
    let mut i = 0usize;
    while i < order.len() {
        let page_id = pages.len() as u32;
        let mut x = 0u32;
        let mut y = 0u32;
        let mut row_h = 0u32;
        let mut used_w = 1u32;
        let mut used_h = 1u32;
        let start = i;
        while i < order.len() {
            let idx = order[i];
            let (gw, gh) = sizes[idx];
            if x > 0 && x + gw + gap > opt.max_size {
                x = 0;
                y += row_h + gap;
                row_h = 0;
            }
            if y + gh > opt.max_size {
                break;
            }
            glyphs[idx] = PackedGlyph {
                index: idx,
                x,
                y,
                w: gw,
                h: gh,
                page: page_id,
            };
            used_w = used_w.max(x + gw);
            used_h = used_h.max(y + gh);
            x += gw + gap;
            row_h = row_h.max(gh);
            i += 1;
        }
        if i == start {
            return Err(Error::AtlasTooSmall(opt.max_size));
        }
        let mut pw = used_w;
        let mut ph = used_h;
        if opt.power_of_two {
            pw = pw.next_power_of_two().min(opt.max_size);
            ph = ph.next_power_of_two().min(opt.max_size);
        }
        if opt.square {
            let s = pw.max(ph);
            pw = s;
            ph = s;
        }
        pages.push(PackedPage {
            width: pw.max(1),
            height: ph.max(1),
        });
    }
    Ok((glyphs, pages))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_several_rects() {
        let sizes = vec![(10, 10), (20, 8), (6, 6)];
        let (g, pages) = pack_glyphs(&sizes, &PackOptions::default()).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(g.len(), 3);
        assert!(pages[0].width >= 20);
        assert!(pages[0].height < 200, "should pack in rows not a column");
    }
}
