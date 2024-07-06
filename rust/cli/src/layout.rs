use gallery::{
    gallery_db::{LayoutRecord, MetObjectLayoutInfo},
    gallery_wall::GalleryWall,
};

/// Try to push paintings down closer to eye level if possible.
const PAINTING_EYE_LEVEL_Y_OFFSET: f64 = 0.5;

const PAINTING_MIN_DISTANCE_FROM_FLOOR: f64 = 0.75;

const PAINTING_HORIZ_MARGIN: f64 = 0.5;

const PAINTING_VERT_MARGIN: f64 = 0.25;

const PAINTING_HORIZ_MIN_MOUNT_AREA: f64 = 2.0;

const PAINTING_VERT_MIN_MOUNT_AREA: f64 = 0.5;

pub struct MetObjectLayoutFitter {
    unused: Vec<MetObjectLayoutInfo>,
    remaining: Vec<MetObjectLayoutInfo>,
}

impl MetObjectLayoutFitter {
    pub fn new(remaining: Vec<MetObjectLayoutInfo>) -> Self {
        MetObjectLayoutFitter {
            unused: vec![],
            remaining,
        }
    }

    pub fn get_object_fitting_in(
        &mut self,
        max_width: f64,
        max_height: f64,
        walls: &Vec<GalleryWall>,
    ) -> Option<MetObjectLayoutInfo> {
        let idx = self
            .unused
            .iter()
            .position(|met_object| can_object_fit_in(&met_object, max_width, max_height));
        if let Some(idx) = idx {
            return Some(self.unused.swap_remove(idx));
        }
        while let Some(met_object) = self.remaining.pop() {
            if can_object_fit_in(&met_object, max_width, max_height) {
                return Some(met_object);
            }
            if can_object_fit_anywhere(&met_object, &walls) {
                self.unused.push(met_object);
            } else {
                println!("Warning: object {} can't fit on any walls.", met_object.id);
            }
        }

        None
    }

    pub fn is_empty(&self) -> bool {
        self.unused.is_empty() && self.remaining.is_empty()
    }
}

fn can_object_fit_in(object_layout: &MetObjectLayoutInfo, max_width: f64, max_height: f64) -> bool {
    object_layout.width < max_width && object_layout.height < max_height
}

fn can_object_fit_anywhere(object_layout: &MetObjectLayoutInfo, walls: &Vec<GalleryWall>) -> bool {
    for wall in walls {
        if can_object_fit_in(object_layout, wall.width, wall.height) {
            return true;
        }
    }
    false
}

pub fn place_paintings_along_wall<'a>(
    gallery_id: i64,
    walls: &Vec<GalleryWall>,
    wall_name: &'a str,
    finder: &mut MetObjectLayoutFitter,
    x_start: f64,
    y_start: f64,
    max_width: f64,
    max_height: f64,
    center_vertically: bool,
    use_dense_layout: bool,
    layout_records: &mut Vec<LayoutRecord<&'a str>>,
) {
    let max_painting_width = max_width - PAINTING_HORIZ_MARGIN * 2.0;
    if max_painting_width <= 0.0 {
        return;
    }
    if let Some(met_object) = finder.get_object_fitting_in(max_painting_width, max_height, &walls) {
        let x = x_start + max_width / 2.0;
        let y = y_start
            + if center_vertically {
                let base_y = max_height / 2.0;
                if met_object.height < max_height - PAINTING_EYE_LEVEL_Y_OFFSET * 2.0 {
                    base_y - PAINTING_EYE_LEVEL_Y_OFFSET
                } else {
                    base_y
                }
            } else {
                max_height - met_object.height / 2.0 - PAINTING_VERT_MARGIN
            };
        let margin_height = y - met_object.height / 2.0;
        let margin_width = max_width / 2.0 - met_object.width / 2.0;
        layout_records.push(LayoutRecord {
            gallery_id,
            wall_id: &wall_name,
            met_object_id: met_object.id,
            x,
            y,
        });
        // Only put at most one painting below the one that's centered vertically.
        if use_dense_layout && center_vertically {
            let vertical_space_below = margin_height - PAINTING_MIN_DISTANCE_FROM_FLOOR;
            let below_y_start = y_start + PAINTING_MIN_DISTANCE_FROM_FLOOR;
            if vertical_space_below > PAINTING_VERT_MIN_MOUNT_AREA {
                let left_edge =
                    x_start + (max_width / 2.0 - met_object.width / 2.0 - PAINTING_HORIZ_MARGIN);
                let right_edge =
                    x_start + (max_width / 2.0 + met_object.width / 2.0 + PAINTING_HORIZ_MARGIN);
                place_paintings_along_wall(
                    gallery_id,
                    walls,
                    wall_name,
                    finder,
                    left_edge,
                    below_y_start,
                    right_edge - left_edge,
                    vertical_space_below,
                    false,
                    false,
                    layout_records,
                );
            }
        }
        if margin_width > PAINTING_HORIZ_MIN_MOUNT_AREA {
            place_paintings_along_wall(
                gallery_id,
                walls,
                wall_name,
                finder,
                x_start,
                y_start,
                margin_width,
                max_height,
                center_vertically,
                use_dense_layout,
                layout_records,
            );
            place_paintings_along_wall(
                gallery_id,
                walls,
                wall_name,
                finder,
                x_start + (max_width / 2.0 + met_object.width / 2.0),
                y_start,
                margin_width,
                max_height,
                center_vertically,
                use_dense_layout,
                layout_records,
            );
        }
    }
}
