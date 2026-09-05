use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use uzor_vectorize::{
    diff_heatmap, hotspot_strip, parity_rgb, parity_sheet, quantized_image, rasterize_svg,
    vectorize_path, vis_hotspots, GpuMode, VectorizeOptions,
};

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!(
            "uzor-vectorize svg <in> -o <out.svg> [--preview p.png]\n\
             uzor-vectorize parity <in> -o <out.svg> [--preview p.png] [--diff d.png]\n\
             uzor-vectorize inspect <in.jpg> <in.svg> --dir <crops/> [--n 12]"
        );
        return ExitCode::from(2);
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "svg" | "parity" => {
            if let Err(e) = run(&cmd, &args) {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        }
        "inspect" => {
            if let Err(e) = inspect(&args) {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        }
        _ => {
            eprintln!("unknown command {cmd}");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

fn run(cmd: &str, args: &[String]) -> Result<(), String> {
    let mut src: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut preview: Option<PathBuf> = None;
    let mut diff: Option<PathBuf> = None;
    let mut opt = VectorizeOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--out" => {
                i += 1;
                out = Some(PathBuf::from(args.get(i).ok_or("-o needs a path")?));
            }
            "--preview" => {
                i += 1;
                preview = Some(PathBuf::from(args.get(i).ok_or("--preview needs a path")?));
            }
            "--diff" => {
                i += 1;
                diff = Some(PathBuf::from(args.get(i).ok_or("--diff needs a path")?));
            }
            "--colors" => {
                i += 1;
                opt.colors = args.get(i).ok_or("--colors")?.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
            }
            "--merge" => {
                i += 1;
                opt.merge = args.get(i).ok_or("--merge")?.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
            }
            "--min-area" => {
                i += 1;
                opt.min_area = args.get(i).ok_or("--min-area")?.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
            }
            "--epsilon" => {
                i += 1;
                opt.epsilon = args.get(i).ok_or("--epsilon")?.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
            }
            "--absorb-dist" => {
                i += 1;
                opt.absorb_dist = args.get(i).ok_or("--absorb-dist")?.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
            }
            "--kmeans-iters" => {
                i += 1;
                opt.kmeans_iters = args.get(i).ok_or("--kmeans-iters")?.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
            }
            "--majority" => {
                opt.majority = true;
            }
            "--tau" => {
                i += 1;
                opt.tau = args.get(i).ok_or("--tau")?.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
            }
            "--median-cut" => {
                opt.tau = 0.0;
                opt.slic = 0;
            }
            "--cpu" => {
                opt.gpu = GpuMode::Cpu;
            }
            "--hybrid" => {
                opt.gpu = GpuMode::Hybrid;
            }
            "--gpu" => {
                opt.gpu = GpuMode::Gpu;
            }
            "--slic" => {
                i += 1;
                opt.slic = args.get(i).ok_or("--slic")?.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
            }
            "--fz" => {
                opt.slic = 0;
            }
            s if s.starts_with('-') => return Err(format!("unknown flag {s}")),
            s => {
                if src.is_some() {
                    return Err("extra positional".into());
                }
                src = Some(PathBuf::from(s));
            }
        }
        i += 1;
    }
    let src = src.ok_or("missing input")?;
    let out = out.ok_or("missing -o")?;
    let doc = vectorize_path(&src, &opt).map_err(|e| e.to_string())?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&out, &doc.svg).map_err(|e| e.to_string())?;
    println!(
        "svg {}x{}  colors={}  contours={}  {} bytes  denoise={}  -> {}",
        doc.width,
        doc.height,
        doc.colors_kept,
        doc.contours,
        doc.svg.len(),
        doc.denoise_device,
        out.display()
    );
    if cmd == "svg" {
        if let Some(p) = preview {
            let img = rasterize_svg(&doc.svg, doc.width, doc.height).map_err(|e| e.to_string())?;
            img.save(&p).map_err(|e| e.to_string())?;
            println!("preview -> {}", p.display());
        }
        return Ok(());
    }

    let prev = preview.unwrap_or_else(|| out.with_extension("png"));
    let dif = diff.unwrap_or_else(|| {
        let mut p = prev.clone();
        p.set_file_name(format!(
            "{}-diff.png",
            prev.file_stem().unwrap().to_string_lossy()
        ));
        p
    });
    let quant_path = {
        let mut p = prev.clone();
        p.set_file_name(format!(
            "{}-quant.png",
            prev.file_stem().unwrap().to_string_lossy()
        ));
        p
    };
    let src_img = image::open(&src).map_err(|e| e.to_string())?.to_rgb8();
    let svg_img = rasterize_svg(&doc.svg, doc.width, doc.height).map_err(|e| e.to_string())?;
    let q_img = quantized_image(&doc);
    if let Some(parent) = prev.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    svg_img.save(&prev).map_err(|e| e.to_string())?;
    q_img.save(&quant_path).map_err(|e| e.to_string())?;
    let heat = diff_heatmap(&src_img, &svg_img);
    heat.save(&dif).map_err(|e| e.to_string())?;
    let sheet_path = {
        let mut p = prev.clone();
        p.set_file_name(format!(
            "{}-sheet.png",
            prev.file_stem().unwrap().to_string_lossy()
        ));
        p
    };
    parity_sheet(&src_img, &svg_img, &q_img, &heat)
        .save(&sheet_path)
        .map_err(|e| e.to_string())?;
    let vs_src = parity_rgb(&src_img, &svg_img);
    let vs_q = parity_rgb(&q_img, &svg_img);
    println!(
        "vs source   MAE={:.2}  close={:.1}%  visible={:.1}%",
        vs_src.mae, vs_src.close_pct, vs_src.visible_pct
    );
    println!(
        "vs quant    MAE={:.2}  close={:.1}%  visible={:.1}%  (vectorization ceiling)",
        vs_q.mae, vs_q.close_pct, vs_q.visible_pct
    );
    println!(
        "preview={}  diff={}  quant={}  sheet={}",
        prev.display(),
        dif.display(),
        quant_path.display(),
        sheet_path.display()
    );
    Ok(())
}

fn inspect(args: &[String]) -> Result<(), String> {
    let mut src: Option<PathBuf> = None;
    let mut svg_path: Option<PathBuf> = None;
    let mut dir: Option<PathBuf> = None;
    let mut n = 12usize;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                i += 1;
                dir = Some(PathBuf::from(args.get(i).ok_or("--dir")?));
            }
            "--n" => {
                i += 1;
                n = args.get(i).ok_or("--n")?.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
            }
            s if s.starts_with('-') => return Err(format!("unknown flag {s}")),
            s => {
                if src.is_none() {
                    src = Some(PathBuf::from(s));
                } else if svg_path.is_none() {
                    svg_path = Some(PathBuf::from(s));
                } else {
                    return Err("extra positional".into());
                }
            }
        }
        i += 1;
    }
    let src = src.ok_or("inspect <src> <svg> --dir <dir>")?;
    let svg_path = svg_path.ok_or("inspect <src> <svg> --dir <dir>")?;
    let dir = dir.ok_or("missing --dir")?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src_img = image::open(&src).map_err(|e| e.to_string())?.to_rgb8();
    let svg_text = fs::read_to_string(&svg_path).map_err(|e| e.to_string())?;
    let svg_img = rasterize_svg(&svg_text, src_img.width(), src_img.height()).map_err(|e| e.to_string())?;
    let heat = diff_heatmap(&src_img, &svg_img);
    let spots = vis_hotspots(&src_img, &svg_img, n);
    let mut json = String::from("[\n");
    for (k, hs) in spots.iter().enumerate() {
        let strip = hotspot_strip(&src_img, &svg_img, &heat, hs);
        let name = format!("crop-{:02}.png", k + 1);
        strip
            .save(dir.join(&name))
            .map_err(|e| e.to_string())?;
        println!(
            "{}  {}x{} @{},{}  area={}  mean_err={:.1}",
            name, hs.w, hs.h, hs.x, hs.y, hs.area, hs.mean_err
        );
        if k > 0 {
            json.push_str(",\n");
        }
        json.push_str(&format!(
            "  {{\"file\":\"{name}\",\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"area\":{},\"mean_err\":{:.1}}}",
            hs.x, hs.y, hs.w, hs.h, hs.area, hs.mean_err
        ));
    }
    json.push_str("\n]\n");
    fs::write(dir.join("crops.json"), json).map_err(|e| e.to_string())?;
    println!("wrote {} crops -> {}", spots.len(), dir.display());
    Ok(())
}
