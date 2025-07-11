// commands/graph.rs

use crate::db::Db;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use image::{DynamicImage, ImageBuffer, RgbImage, ImageFormat};  // ← note ImageFormat
use poloto::prelude::*; // Figure, build::plot, frame_build, header, Label, Theme :contentReference[oaicite:0]{index=0}
use poloto::{build, frame_build, header, ticks};
use resvg::tiny_skia;
use resvg::usvg;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};
use teloxide::{prelude::Requester, types::InputFile, Bot};
use tokio::task;
use std::io::{BufReader, Read};
use std::fs::File;
use fontdb::Database;
use usvg::{Tree};
use usvg_text_layout::convert_text;

/// Convert an SVG file at `svg_path` into a raster image at `out_path`
/// keeping the same width/height, format chosen by extension (jpg/png).

fn convert_svg_to_image(svg_path: &str, out_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read SVG data
    let mut file = File::open(svg_path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    // 2. Parse with font support
    let mut opt = usvg::Options::default();
    // CORRECT: get a mutable reference to the inner Database, then load fonts
    opt.fontdb_mut().load_system_fonts();    // &mut Database → you can now call load_system_fonts() :contentReference[oaicite:0]{index=0}
    let tree = usvg::Tree::from_data(&data, &opt)?;
    // 3. Rasterize at the SVG’s own size
    let size = tree.size();
    let (w, h) = (size.width().round() as u32, size.height().round() as u32);
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("pixmap failed")?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    // 4. Save as JPEG (or PNG)
    let buffer: RgbImage = ImageBuffer::from_fn(w, h, |x, y| {
        let p = pixmap.pixel(x, y).unwrap();
        image::Rgb([p.red(), p.green(), p.blue()])
    });
    let dyn_img = DynamicImage::ImageRgb8(buffer);
    let mut out = File::create(out_path)?;
    dyn_img.write_to(&mut out, ImageFormat::Jpeg)?;

    Ok(())
}
/// 从过去一小时数据生成延迟折线图并发送（基于 poloto 19.1.2）
// … all your existing imports and convert_svg_to_image stay exactly the same …

pub async fn graph_command(
    bot: Bot,
    chat_id: teloxide::types::ChatId,
    db: Arc<Db>,
) -> anyhow::Result<()> {
    // 1. Fetch and prepare data exactly as you already do…
    let now   = Utc::now();
    let since = now - Duration::hours(1);
    let rows  = db.query_metrics(since).await.unwrap_or_default();


    // 4.2 Build a small tick generator at 0, 30, 60 minutes:
    let x_ticks = ticks::from_iter(vec![0.0, 30.0, 60.0].into_iter())
        .with_tick_fmt(|&v| {
            // 60 - v gives “minutes ago”
            format!("{}m ago", (60.0 - v) as usize)
        });

    // 1. 计算 rel_min ∈ [0,60]：0=1h 前，60=现在
    let base_secs = since.timestamp() as f64;
    let mut series_map: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    for (alias, ts, lat, _loss) in rows {
        let rel_min = (ts.timestamp() as f64 - base_secs) / 60.0;
        series_map.entry(alias).or_default().push((rel_min, lat));
    }



    let filename     = format!("graph_{}.svg", now.timestamp());
    let filename_buf = PathBuf::from(&filename);
    let series_owned = series_map.clone();

    // 2. 构建 SVG
    let svg = task::spawn_blocking(move || -> anyhow::Result<String> {
        let plots = series_owned.iter().map(|(alias, pts)| {
            build::plot(alias.clone()).line(pts.clone().into_iter())
        });

        // —— 手动指定 X 轴刻度：0, 20, 40, 60 —— 
        // 确保两个以上刻度，左端0代表“1h ago”，右端60代表“now”
        let x_ticks = ticks::from_iter(vec![0.0, 20.0, 40.0, 60.0].into_iter())
            .with_tick_fmt(|&v| {
                // 0→"60m ago"，60→"0m ago"，20→"40m ago"…
                format!("{}m ago", (60.0 - v) as usize)
            });

        // 仅在 Data 上注入自定义刻度
        let data = frame_build()
            .data(plots)
            .map_xticks(|_| x_ticks);

        let frame = data.build_and_label((
            "过去 1 小时延迟曲线",
            "时间 (minutes ago)",
            "延迟 (ms)",
        ));

        let svg_text = frame.append_to(header().light_theme()).render_string()?;
        fs::write(&filename_buf, svg_text.as_bytes())?;
        Ok(filename)
    })
        .await??;

    // 3. Rasterize & send exactly as you already do…
    let img_filename = svg.replace(".svg", ".jpg");
    let _ = convert_svg_to_image(&svg, &img_filename);
    bot.send_photo(chat_id, InputFile::file(&img_filename)).await?;

    Ok(())
}
