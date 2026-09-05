use crate::contour::Pt;

pub fn path_d(loops: &[Vec<Pt>]) -> String {
    let mut s = String::new();
    for lp in loops {
        if lp.is_empty() {
            continue;
        }
        s.push_str(&format!("M{:.1},{:.1}", lp[0].0 as f32, lp[0].1 as f32));
        for p in lp.iter().skip(1) {
            s.push_str(&format!("L{:.1},{:.1}", p.0 as f32, p.1 as f32));
        }
        s.push('Z');
    }
    s
}

pub fn hex_color(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

pub fn fill_path(hex: &str, d: &str) -> String {
    format!("  <path fill=\"{hex}\" fill-rule=\"nonzero\" d=\"{d}\"/>")
}

pub fn stroke_path(hex: &str, width: f32, d: &str) -> String {
    format!(
        "  <path fill=\"none\" stroke=\"{hex}\" stroke-width=\"{width:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" d=\"{d}\"/>"
    )
}

pub fn document(width: u32, height: u32, paths: &[String]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" shape-rendering=\"geometricPrecision\">\n"
    ));
    for p in paths {
        out.push_str(p);
        out.push('\n');
    }
    out.push_str("</svg>\n");
    out
}
