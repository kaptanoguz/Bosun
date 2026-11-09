//! Son bir saatin küçük eğrisi. Renk temanın vurgu renginden gelir; üzerine gelince o anki
//! değer ve saat görünür.

use adw::prelude::*;
use chrono::{DateTime, Utc};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct Series {
    main: Vec<f64>,
    second: Vec<f64>,
    max: Option<f64>,
    times: Vec<DateTime<Utc>>,
}

pub struct Spark {
    pub area: gtk::DrawingArea,
    data: Rc<RefCell<Series>>,
}

impl Spark {
    /// `fmt` bir noktanın değerini metne çevirir (araç ipucu için).
    pub fn new(fmt: fn(f64) -> String) -> Spark {
        let area = gtk::DrawingArea::builder()
            .content_height(40)
            .hexpand(true)
            .build();
        area.add_css_class("accent");
        area.add_css_class("spark");
        let data: Rc<RefCell<Series>> = Rc::default();
        {
            let data = data.clone();
            area.set_draw_func(move |a, cr, w, h| draw(a, cr, w, h, &data.borrow()));
        }
        area.set_has_tooltip(true);
        {
            let data = data.clone();
            area.connect_query_tooltip(move |a, x, _, _, tip| {
                let d = data.borrow();
                let n = d.main.len();
                if n < 2 || a.width() <= 0 {
                    return false;
                }
                let i = ((x as f64 / a.width() as f64) * (n - 1) as f64)
                    .round()
                    .clamp(0.0, (n - 1) as f64) as usize;
                let mut text = fmt(d.main[i]);
                if let Some(s) = d.second.get(i) {
                    text = format!("↓ {text}  ↑ {}", fmt(*s));
                }
                if let Some(ts) = d.times.get(i) {
                    text = format!("{} · {text}", crate::i18n::time_of_day(*ts));
                }
                tip.set_text(Some(&text));
                true
            });
        }
        Spark { area, data }
    }

    pub fn set(
        &self,
        main: Vec<f64>,
        second: Option<Vec<f64>>,
        max: Option<f64>,
        times: Vec<DateTime<Utc>>,
    ) {
        *self.data.borrow_mut() = Series {
            main,
            second: second.unwrap_or_default(),
            max,
            times,
        };
        self.area.queue_draw();
    }
}

fn draw(area: &gtk::DrawingArea, cr: &gtk::cairo::Context, w: i32, h: i32, d: &Series) {
    if d.main.len() < 2 {
        return;
    }
    let c = area.color();
    let (r, g, b) = (c.red() as f64, c.green() as f64, c.blue() as f64);
    let top = d
        .max
        .unwrap_or_else(|| {
            d.main
                .iter()
                .chain(d.second.iter())
                .copied()
                .fold(0.0, f64::max)
        })
        .max(1e-9);
    let (w, h) = (w as f64, h as f64 - 2.0);
    let point = |i: usize, v: f64, n: usize| {
        (
            i as f64 / (n - 1) as f64 * w,
            1.0 + h - (v / top).clamp(0.0, 1.0) * h,
        )
    };

    let path = |vals: &[f64]| {
        for (i, v) in vals.iter().enumerate() {
            let (x, y) = point(i, *v, vals.len());
            if i == 0 {
                cr.move_to(x, y);
            } else {
                cr.line_to(x, y);
            }
        }
    };

    // Altı hafif dolgulu ana eğri.
    path(&d.main);
    cr.line_to(w, h + 1.0);
    cr.line_to(0.0, h + 1.0);
    cr.close_path();
    cr.set_source_rgba(r, g, b, 0.14);
    let _ = cr.fill();
    path(&d.main);
    cr.set_source_rgba(r, g, b, 1.0);
    cr.set_line_width(1.5);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    let _ = cr.stroke();

    // İkinci seri (yazma / gönderme) kesikli ve soluk.
    if d.second.len() >= 2 {
        path(&d.second);
        cr.set_source_rgba(r, g, b, 0.55);
        cr.set_dash(&[3.0, 3.0], 0.0);
        let _ = cr.stroke();
    }
}
