use std::sync::OnceLock;

use bevy::prelude::*;
use serde_json::Value;

struct Section {
    height: f32,
    width: f32,
    back: f32,
    front: Vec<f32>,
}

pub struct Head {
    lateral: Vec<f32>,
    sections: Vec<Section>,
}

pub fn head(index: usize) -> &'static Head {
    static HEADS: OnceLock<[Head; 2]> = OnceLock::new();
    &HEADS.get_or_init(|| {
        [
            Head::parse(include_str!("../../assets/avatars/host-1-head.json"))
                .expect("valid bundled browser head"),
            Head::parse(include_str!("../../assets/avatars/host-2-head.json"))
                .expect("valid bundled microphone head"),
        ]
    })[index]
}

fn numbers(value: &Value) -> Result<Vec<f32>, String> {
    value
        .as_array()
        .ok_or("expected numeric array")?
        .iter()
        .map(|value| {
            let number = value.as_f64().ok_or("expected number")? as f32;
            if number.is_finite() && (0.0..=1.0).contains(&number) {
                Ok(number)
            } else {
                Err("head coordinates must be finite and within 0..1".into())
            }
        })
        .collect()
}

impl Head {
    fn parse(source: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
        let lateral = numbers(&value["lateral_coordinates"])?;
        if lateral.len() < 3
            || lateral[0] != 0.0
            || *lateral.last().unwrap() != 1.0
            || !lateral.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err("lateral coordinates must increase from zero to one".into());
        }
        let rows = value["sections"]
            .as_array()
            .ok_or("expected head sections")?;
        let mut sections = Vec::new();
        for row in rows {
            let row = row.as_array().ok_or("expected section array")?;
            if row.len() != 4 {
                return Err("each section needs height, width, back and front depths".into());
            }
            let dimensions = numbers(&Value::Array(row[..3].to_vec()))?;
            let front = numbers(&row[3])?;
            if front.len() != lateral.len()
                || *front.last().unwrap() != 0.0
                || dimensions[1] <= 0.0
                || dimensions[2] <= 0.0
            {
                return Err("invalid section dimensions or lateral depths".into());
            }
            sections.push(Section {
                height: dimensions[0],
                width: dimensions[1],
                back: dimensions[2],
                front,
            });
        }
        if sections.len() < 4
            || !sections
                .windows(2)
                .all(|pair| pair[0].height < pair[1].height)
        {
            return Err("head heights must increase".into());
        }
        Ok(Self { lateral, sections })
    }

    pub fn heights(&self) -> (f32, f32) {
        (
            self.sections[0].height,
            self.sections.last().unwrap().height,
        )
    }

    fn sample(&self, height: f32, value: impl Fn(&Section) -> f32) -> f32 {
        let section = self
            .sections
            .windows(2)
            .position(|pair| height <= pair[1].height)
            .unwrap_or(self.sections.len() - 2);
        let start = &self.sections[section];
        let end = &self.sections[section + 1];
        let amount = ((height - start.height) / (end.height - start.height)).clamp(0.0, 1.0);
        let previous = &self.sections[section.saturating_sub(1)];
        let next = &self.sections[(section + 2).min(self.sections.len() - 1)];
        let start_slope = (value(end) - value(previous)) / (end.height - previous.height);
        let end_slope = (value(next) - value(start)) / (next.height - start.height);
        hermite(
            value(start),
            value(end),
            start_slope,
            end_slope,
            end.height - start.height,
            amount,
        )
        .max(0.0)
    }

    pub fn profile(&self, height: f32) -> Vec2 {
        Vec2::new(
            self.sample(height, |section| section.width),
            self.sample(height, |section| section.back),
        )
    }

    pub fn front(&self, horizontal: f32, height: f32) -> f32 {
        let fraction = (horizontal.abs() / self.profile(height).x).clamp(0.0, 1.0);
        let section = self
            .lateral
            .windows(2)
            .position(|pair| fraction <= pair[1])
            .unwrap_or(self.lateral.len() - 2);
        let amount = (fraction - self.lateral[section])
            / (self.lateral[section + 1] - self.lateral[section]);
        let start = self.sample(height, |row| row.front[section]);
        let end = self.sample(height, |row| row.front[section + 1]);
        let previous = section.saturating_sub(1);
        let next = (section + 2).min(self.lateral.len() - 1);
        let start_slope = if section == 0 {
            0.0
        } else {
            (end - self.sample(height, |row| row.front[previous]))
                / (self.lateral[section + 1] - self.lateral[previous])
        };
        let end_slope = (self.sample(height, |row| row.front[next]) - start)
            / (self.lateral[next] - self.lateral[section]);
        hermite(
            start,
            end,
            start_slope,
            end_slope,
            self.lateral[section + 1] - self.lateral[section],
            amount,
        )
        .max(0.0)
    }
}

fn hermite(start: f32, end: f32, start_slope: f32, end_slope: f32, span: f32, amount: f32) -> f32 {
    let squared = amount * amount;
    let cubed = squared * amount;
    (2.0 * cubed - 3.0 * squared + 1.0) * start
        + (cubed - 2.0 * squared + amount) * span * start_slope
        + (-2.0 * cubed + 3.0 * squared) * end
        + (cubed - squared) * span * end_slope
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_heads_have_distinct_proportions_and_integrated_noses() {
        assert!(head(0).profile(0.235).x > head(1).profile(0.235).x);
        assert!(head(0).heights().1 < head(1).heights().1);
        for index in 0..2 {
            let head = head(index);
            for step in 0..1000 {
                let height =
                    head.heights().0 + step as f32 / 999.0 * (head.heights().1 - head.heights().0);
                assert!(head.profile(height).min_element() > 0.0);
                for column in 0..100 {
                    let horizontal = head.profile(height).x * column as f32 / 99.0;
                    let depth = head.front(horizontal, height);
                    assert!(depth.is_finite() && (0.0..0.25).contains(&depth));
                    assert!((depth - head.front(-horizontal, height)).abs() < 0.00001);
                }
            }
            assert!(head.front(0.0, 0.213) - head.front(0.055, 0.213) > 0.025);
            assert_eq!(head.front(head.profile(0.25).x, 0.25), 0.0);
        }
    }

    #[test]
    fn profile_slopes_are_continuous_at_authored_heights() {
        for index in 0..2 {
            let head = head(index);
            for section in &head.sections[1..head.sections.len() - 1] {
                let epsilon = 0.00001;
                let center = head.sample(section.height, |section| section.width);
                let left = (center
                    - head.sample(section.height - epsilon, |section| section.width))
                    / epsilon;
                let right = (head.sample(section.height + epsilon, |section| section.width)
                    - center)
                    / epsilon;
                assert!((left - right).abs() < 0.02);
            }
        }
    }

    #[test]
    fn cranial_sections_have_full_rounded_back_profiles() {
        for index in 0..2 {
            let head = head(index);
            for step in 0..400 {
                let height = 0.095 + step as f32 / 399.0 * 0.345;
                let profile = head.profile(height);
                assert!(profile.y + head.front(0.0, height) >= profile.x * 2.0);
            }
            let peak = (0..1000)
                .map(|step| head.profile(0.095 + step as f32 * 0.00035).y)
                .fold(0.0_f32, f32::max);
            assert!((0.30..0.33).contains(&peak));
            for section in &head.sections[2..head.sections.len() - 2] {
                let epsilon = 0.00001;
                let center = head.profile(section.height).y;
                let left = (center - head.profile(section.height - epsilon).y) / epsilon;
                let right = (head.profile(section.height + epsilon).y - center) / epsilon;
                assert!((left - right).abs() < 0.03);
            }
        }
    }

    #[test]
    fn malformed_head_assets_are_rejected() {
        for source in [
            "{}",
            "{\"lateral_coordinates\":[0,1],\"sections\":[]}",
            "{\"lateral_coordinates\":[0,0.5,1],\"sections\":[[0,0,0,[0,0,0]]]}",
        ] {
            assert!(Head::parse(source).is_err());
        }
    }
}
