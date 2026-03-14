#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn area(&self) -> u32 {
        (self.width as u32) * (self.height as u32)
    }

    pub fn intersection(self, other: Rect) -> Rect {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        if x1 < x2 && y1 < y2 {
            Rect {
                x: x1,
                y: y1,
                width: x2 - x1,
                height: y2 - y1,
            }
        } else {
            Rect::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constraint {
    Length(u16),
    Percentage(u16),
    Ratio(u32, u32),
    Fill(u32), // weight-based distribution
    Min(u16),
    Max(u16),
    Intrinsic,         // Both dimensions determined by content
    IntrinsicVertical, // Height determined by content, width constrained
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub struct Layout {
    direction: Direction,
    constraints: Vec<Constraint>,
    margin: u16,
    spacing: u16,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            direction: Direction::Vertical,
            constraints: Vec::new(),
            margin: 0,
            spacing: 0,
        }
    }
}

impl Layout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    pub fn constraints<C>(mut self, constraints: C) -> Self
    where
        C: Into<Vec<Constraint>>,
    {
        self.constraints = constraints.into();
        self
    }

    pub fn margin(mut self, margin: u16) -> Self {
        self.margin = margin;
        self
    }

    pub fn spacing(mut self, spacing: u16) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn split(&self, area: Rect) -> Vec<Rect> {
        self.split_with_measures(area, &[])
    }

    pub fn split_with_measures(&self, area: Rect, measures: &[(u16, u16)]) -> Vec<Rect> {
        let mut results = Vec::with_capacity(self.constraints.len());

        let (available_space, start_pos) = match self.direction {
            Direction::Horizontal => (
                area.width.saturating_sub(2 * self.margin),
                area.x + self.margin,
            ),
            Direction::Vertical => (
                area.height.saturating_sub(2 * self.margin),
                area.y + self.margin,
            ),
        };

        if available_space == 0 {
            for _ in 0..self.constraints.len() {
                results.push(Rect::default());
            }
            return results;
        }

        // Subtract spacing between elements
        let total_spacing = if self.constraints.len() > 1 {
            (self.constraints.len() as u16 - 1) * self.spacing
        } else {
            0
        };
        let available_space_content = available_space.saturating_sub(total_spacing);

        let mut remaining_space = available_space_content;
        let mut total_fill_weight = 0u32;
        let mut fill_indices = Vec::new();
        let mut sizes = vec![0u16; self.constraints.len()];

        // Pass 1: Handle Fixed, Percentage, Ratio, Intrinsic
        for (i, &constraint) in self.constraints.iter().enumerate() {
            match constraint {
                Constraint::Length(l) => {
                    let size = l.min(remaining_space);
                    sizes[i] = size;
                    remaining_space -= size;
                }
                Constraint::Percentage(p) => {
                    let size = (available_space_content as u32 * p as u32 / 100) as u16;
                    let size = size.min(remaining_space);
                    sizes[i] = size;
                    remaining_space -= size;
                }
                Constraint::Ratio(n, d) => {
                    if d > 0 {
                        let size = (available_space_content as u32 * n / d) as u16;
                        let size = size.min(remaining_space);
                        sizes[i] = size;
                        remaining_space -= size;
                    }
                }
                Constraint::Intrinsic => {
                    let (w, h) = measures.get(i).cloned().unwrap_or((0, 0));
                    let size = match self.direction {
                        Direction::Horizontal => w,
                        Direction::Vertical => h,
                    };
                    let size = size.min(remaining_space);
                    sizes[i] = size;
                    remaining_space -= size;
                }
                Constraint::IntrinsicVertical => {
                    let (_w, h) = measures.get(i).cloned().unwrap_or((0, 0));
                    let size = match self.direction {
                        Direction::Horizontal => available_space_content,
                        Direction::Vertical => h,
                    };
                    let size = size.min(remaining_space);
                    sizes[i] = size;
                    remaining_space -= size;
                }
                Constraint::Fill(w) => {
                    total_fill_weight += w.max(1);
                    fill_indices.push(i);
                }
                Constraint::Min(m) => {
                    let size = m.min(remaining_space);
                    sizes[i] = size;
                    remaining_space -= size;
                }
                Constraint::Max(_) => {
                    // Max is tricky, handled as flexible if not constrained otherwise
                    total_fill_weight += 1;
                    fill_indices.push(i);
                }
            }
        }

        // Pass 2: Distribute remaining space among Fills
        if total_fill_weight > 0 && remaining_space > 0 {
            let space_per_weight = remaining_space as u32 / total_fill_weight;
            let mut distributed_remainder = remaining_space as u32 % total_fill_weight;

            for &i in &fill_indices {
                let weight = match self.constraints[i] {
                    Constraint::Fill(w) => w.max(1),
                    Constraint::Max(_) => 1,
                    _ => 0,
                };

                let mut size = (weight * space_per_weight) as u16;
                if distributed_remainder > 0 {
                    size += 1;
                    distributed_remainder -= 1;
                }

                // Enforce Max if applicable
                if let Constraint::Max(max) = self.constraints[i] {
                    size = size.min(max);
                }

                sizes[i] = size;
            }
        }

        // Convert sizes to Rects
        let mut current_pos = start_pos;
        for size in sizes {
            match self.direction {
                Direction::Horizontal => {
                    results.push(Rect {
                        x: current_pos,
                        y: area.y + self.margin,
                        width: size,
                        height: area.height.saturating_sub(2 * self.margin),
                    });
                    current_pos += size + self.spacing;
                }
                Direction::Vertical => {
                    results.push(Rect {
                        x: area.x + self.margin,
                        y: current_pos,
                        width: area.width.saturating_sub(2 * self.margin),
                        height: size,
                    });
                    current_pos += size + self.spacing;
                }
            }
        }

        results
    }
}
