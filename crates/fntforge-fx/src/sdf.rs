//! Euclidean distance transform (Felzenszwalb & Huttenlocher).
//! Positive = outside the glyph, negative = inside.

pub fn signed_distance(mask: &[u8], w: usize, h: usize) -> Vec<f32> {
    let n = w * h;
    let inf = 1.0e10f32;
    let mut outside = vec![inf; n];
    let mut inside = vec![inf; n];
    for i in 0..n {
        let a = mask[i] as f32 / 255.0;
        // Fractional coverage: treat 0.5 as the edge.
        if a >= 0.5 {
            inside[i] = 0.0;
            outside[i] = (1.0 - a).max(0.0);
        } else {
            outside[i] = 0.0;
            inside[i] = a;
        }
    }
    edt_2d(&mut outside, w, h);
    edt_2d(&mut inside, w, h);
    (0..n)
        .map(|i| outside[i].sqrt() - inside[i].sqrt())
        .collect()
}

fn edt_2d(grid: &mut [f32], w: usize, h: usize) {
    let mut row = vec![0.0f32; w];
    for y in 0..h {
        row.copy_from_slice(&grid[y * w..y * w + w]);
        edt_1d(&mut row);
        grid[y * w..y * w + w].copy_from_slice(&row);
    }
    let mut col = vec![0.0f32; h];
    for x in 0..w {
        for y in 0..h {
            col[y] = grid[y * w + x];
        }
        edt_1d(&mut col);
        for y in 0..h {
            grid[y * w + x] = col[y];
        }
    }
}

fn edt_1d(f: &mut [f32]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f32; n + 1];
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let mut s;
        loop {
            let p = v[k];
            s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q as f32 - p as f32));
            if s > z[k] {
                break;
            }
            if k == 0 {
                break;
            }
            k -= 1;
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    let mut out = vec![0.0f32; n];
    for q in 0..n {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let dx = q as f32 - v[k] as f32;
        out[q] = dx * dx + f[v[k]];
    }
    f.copy_from_slice(&out);
}

/// Coverage in [0,1] for a band: pixels whose sdf is in (lo, hi], with 1px AA.
pub fn band(sdf: &[f32], lo: f32, hi: f32) -> Vec<f32> {
    sdf.iter()
        .map(|&d| {
            let a = smoothstep(lo - 0.5, lo + 0.5, d);
            let b = 1.0 - smoothstep(hi - 0.5, hi + 0.5, d);
            (a * b).clamp(0.0, 1.0)
        })
        .collect()
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0).max(1e-5)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn max_inside(sdf: &[f32]) -> f32 {
    sdf.iter().fold(0.0f32, |m, &d| if d < 0.0 { m.max(-d) } else { m })
}
