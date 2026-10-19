use ab_glyph::{Font, FontRef, ScaleFont, point};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use chrono::{Datelike, Local, NaiveDate};

pub const TITLE: &str = "AI Talks with Jati & Oli";
pub const DEFAULT_SUBTITLE: &str = "An AI and a human talk about AI and humans";
const FONT_DATA: &[u8] = include_bytes!("../assets/studio/DejaVuSans-Bold.ttf");

#[derive(Resource, Clone, Debug)]
pub struct StudioSigns {
    pub subtitle: String,
    pub date: NaiveDate,
}

impl Default for StudioSigns {
    fn default() -> Self {
        Self {
            subtitle: DEFAULT_SUBTITLE.into(),
            date: Local::now().date_naive(),
        }
    }
}

impl StudioSigns {
    pub fn title_label(&self) -> String {
        format!("{TITLE}\n{}", self.subtitle)
    }

    pub fn calendar_lines(&self) -> [String; 4] {
        let months = [
            "JANUAR",
            "FEBRUAR",
            "MÄRZ",
            "APRIL",
            "MAI",
            "JUNI",
            "JULI",
            "AUGUST",
            "SEPTEMBER",
            "OKTOBER",
            "NOVEMBER",
            "DEZEMBER",
        ];
        let weekdays = [
            "Montag",
            "Dienstag",
            "Mittwoch",
            "Donnerstag",
            "Freitag",
            "Samstag",
            "Sonntag",
        ];
        [
            months[self.date.month0() as usize].into(),
            weekdays[self.date.weekday().num_days_from_monday() as usize].into(),
            self.date.day().to_string(),
            self.date.year().to_string(),
        ]
    }

    pub fn title_image(&self) -> Image {
        let mut canvas = Canvas::new(2048, 352, [22, 36, 44]);
        let gold = [189, 149, 90];
        canvas.rectangle(8, 8, 2032, 8, gold);
        canvas.rectangle(8, 336, 2032, 8, gold);
        canvas.rectangle(8, 8, 8, 336, gold);
        canvas.rectangle(2032, 8, 8, 336, gold);
        canvas.text(TITLE, (1024.0, 132.0), 150.0, 1940.0, [255, 240, 214]);
        canvas.text(
            &self.subtitle,
            (1024.0, 264.0),
            84.0,
            1940.0,
            [245, 188, 117],
        );
        canvas.image()
    }

    pub fn calendar_image(&self) -> Image {
        let mut canvas = Canvas::new(512, 704, [255, 240, 214]);
        let red = [160, 46, 49];
        canvas.rectangle(0, 0, 512, 160, red);
        let [month, weekday, day, year] = self.calendar_lines();
        canvas.text(&month, (256.0, 105.0), 84.0, 470.0, [255, 240, 214]);
        canvas.text(&weekday, (256.0, 220.0), 78.0, 470.0, red);
        canvas.text(&day, (256.0, 422.0), 414.0, 450.0, [44, 41, 43]);
        canvas.text(&year, (256.0, 620.0), 104.0, 470.0, red);
        for horizontal in [93, 395] {
            canvas.rectangle(horizontal, 16, 24, 45, [191, 196, 194]);
        }
        canvas.image()
    }
}

struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32, color: [u8; 3]) -> Self {
        Self {
            width,
            height,
            pixels: [color[0], color[1], color[2], 255].repeat((width * height) as usize),
        }
    }

    fn rectangle(&mut self, left: u32, top: u32, width: u32, height: u32, color: [u8; 3]) {
        for vertical in top..(top + height).min(self.height) {
            for horizontal in left..(left + width).min(self.width) {
                let offset = ((vertical * self.width + horizontal) * 4) as usize;
                self.pixels[offset..offset + 3].copy_from_slice(&color);
            }
        }
    }

    fn text(&mut self, text: &str, center: (f32, f32), size: f32, max_width: f32, color: [u8; 3]) {
        let font = FontRef::try_from_slice(FONT_DATA).expect("valid bundled font");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let measure = |size: f32| {
            let scaled = font.as_scaled(size);
            let mut width = 0.0;
            let mut previous = None;
            for character in text.chars() {
                let glyph = font.glyph_id(character);
                if let Some(previous) = previous {
                    width += scaled.kern(previous, glyph);
                }
                width += scaled.h_advance(glyph);
                previous = Some(glyph);
            }
            width
        };
        let size = size * (max_width / measure(size).max(1.0)).min(1.0);
        let scaled = font.as_scaled(size);
        let mut cursor = 0.0;
        let mut previous = None;
        let mut outlines = Vec::new();
        for character in text.chars() {
            let glyph = font.glyph_id(character);
            if let Some(previous) = previous {
                cursor += scaled.kern(previous, glyph);
            }
            if let Some(outline) =
                font.outline_glyph(glyph.with_scale_and_position(size, point(cursor, 0.0)))
            {
                outlines.push(outline);
            }
            cursor += scaled.h_advance(glyph);
            previous = Some(glyph);
        }
        if outlines.is_empty() {
            return;
        }
        let left = outlines
            .iter()
            .map(|outline| outline.px_bounds().min.x)
            .fold(f32::INFINITY, f32::min);
        let right = outlines
            .iter()
            .map(|outline| outline.px_bounds().max.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let top = outlines
            .iter()
            .map(|outline| outline.px_bounds().min.y)
            .fold(f32::INFINITY, f32::min);
        let bottom = outlines
            .iter()
            .map(|outline| outline.px_bounds().max.y)
            .fold(f32::NEG_INFINITY, f32::max);
        let offset = point(
            center.0 - (left + right) * 0.5,
            center.1 - (top + bottom) * 0.5,
        );
        for outline in outlines {
            let bounds = outline.px_bounds();
            outline.draw(|horizontal, vertical, coverage| {
                let horizontal = (horizontal as f32 + bounds.min.x + offset.x).round() as i32;
                let vertical = (vertical as f32 + bounds.min.y + offset.y).round() as i32;
                if horizontal < 0
                    || vertical < 0
                    || horizontal >= self.width as i32
                    || vertical >= self.height as i32
                {
                    return;
                }
                let index = ((vertical as u32 * self.width + horizontal as u32) * 4) as usize;
                for (channel, foreground) in color.iter().enumerate() {
                    self.pixels[index + channel] = (self.pixels[index + channel] as f32
                        * (1.0 - coverage)
                        + *foreground as f32 * coverage)
                        .round() as u8;
                }
            });
        }
    }

    fn image(self) -> Image {
        Image::new(
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            self.pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_use_local_today_and_requested_title_and_subtitle() {
        let before = Local::now().date_naive();
        let signs = StudioSigns::default();
        let after = Local::now().date_naive();
        assert!(signs.date == before || signs.date == after);
        assert_eq!(
            signs.title_label(),
            "AI Talks with Jati & Oli\nAn AI and a human talk about AI and humans"
        );
    }

    #[test]
    fn calendar_derives_german_month_weekday_day_and_year_from_date() {
        let font = FontRef::try_from_slice(FONT_DATA).unwrap();
        for character in "ÄÖÜäöüß".chars() {
            assert_ne!(
                font.glyph_id(character).0,
                0,
                "missing German glyph {character}"
            );
        }
        let mut signs = StudioSigns {
            date: NaiveDate::from_ymd_opt(2026, 10, 17).unwrap(),
            ..default()
        };
        assert_eq!(signs.calendar_lines(), ["OKTOBER", "Samstag", "17", "2026"]);
        signs.date = NaiveDate::from_ymd_opt(2028, 2, 29).unwrap();
        assert_eq!(
            signs.calendar_lines(),
            ["FEBRUAR", "Dienstag", "29", "2028"]
        );
        signs.date = NaiveDate::from_ymd_opt(2027, 3, 1).unwrap();
        assert_eq!(signs.calendar_lines(), ["MÄRZ", "Montag", "1", "2027"]);
    }

    #[test]
    fn subtitle_and_date_change_the_actual_texture_pixels_independently() {
        let defaults = StudioSigns::default();
        let mut custom = defaults.clone();
        custom.subtitle = "Sonderausgabe: Iris hat Geburtstag".into();
        assert_ne!(custom.title_image().data, defaults.title_image().data);
        assert_eq!(custom.calendar_image().data, defaults.calendar_image().data);
        custom.date = defaults.date.succ_opt().unwrap();
        assert_ne!(custom.calendar_image().data, defaults.calendar_image().data);
        custom.subtitle = "Längerer Untertitel mit Umlauten: ÄÖÜ äöü ß. ".repeat(20);
        let image = custom.title_image();
        assert_eq!(image.width(), 2048);
        assert_eq!(image.height(), 352);
        let pixels = image.data.unwrap();
        for vertical in 190..320 {
            for horizontal in [20, 2020] {
                let offset = (vertical * 2048 + horizontal) * 4;
                assert_eq!(&pixels[offset..offset + 3], &[22, 36, 44]);
            }
        }
        custom.subtitle.clear();
        assert_eq!(custom.title_image().width(), 2048);
    }
}
