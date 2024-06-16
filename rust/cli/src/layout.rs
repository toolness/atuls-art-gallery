use gallery::{
    gallery_db::{LayoutRecord, MetObjectLayoutInfo},
    gallery_wall::GalleryWall,
};

/// Try to push paintings down closer to eye level if possible.
const PAINTING_Y_OFFSET: f64 = 0.5;

const PAINTING_MIN_MOUNT_AREA: f64 = 2.0;

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
    max_width: f64,
    max_height: f64,
    layout_records: &mut Vec<LayoutRecord<&'a str>>,
) {
    if let Some(met_object) = finder.get_object_fitting_in(max_width, max_height, &walls) {
        let x = x_start + max_width / 2.0;
        let mut y = max_height / 2.0;
        if met_object.height < max_height - PAINTING_Y_OFFSET * 2.0 {
            y -= PAINTING_Y_OFFSET;
        }
        layout_records.push(LayoutRecord {
            gallery_id,
            wall_id: &wall_name,
            met_object_id: met_object.id,
            x,
            y,
        });
        let margin_width = max_width / 2.0 - met_object.width / 2.0;
        if margin_width > PAINTING_MIN_MOUNT_AREA {
            place_paintings_along_wall(
                gallery_id,
                walls,
                wall_name,
                finder,
                x_start,
                margin_width,
                max_height,
                layout_records,
            );
            place_paintings_along_wall(
                gallery_id,
                walls,
                wall_name,
                finder,
                x_start + (max_width / 2.0 + met_object.width / 2.0),
                margin_width,
                max_height,
                layout_records,
            );
        }
    }
}
