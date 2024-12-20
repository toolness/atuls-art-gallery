use std::collections::HashSet;

use crate::art_object::ArtObjectId;

use super::{
    gallery_db::{ArtObjectLayoutInfo, LayoutRecord},
    gallery_wall::GalleryWall,
};

use anyhow::Result;

/// Try to push paintings down closer to eye level if possible.
const PAINTING_EYE_LEVEL_Y_OFFSET: f64 = 0.5;

/// This is only used when vertically stacking paintings: if we
/// stack, we'll only do so if the stacked painting is this far
/// from the floor.
const PAINTING_MIN_DISTANCE_FROM_FLOOR: f64 = 0.75;

const PAINTING_HORIZ_MARGIN: f64 = 0.5;

const PAINTING_VERT_MARGIN: f64 = 0.25;

const PAINTING_HORIZ_MIN_MOUNT_AREA: f64 = 2.0;

const PAINTING_VERT_MIN_MOUNT_AREA: f64 = 0.5;

pub struct ArtObjectLayoutFitter {
    unused: Vec<ArtObjectLayoutInfo>,
    remaining: Vec<ArtObjectLayoutInfo>,
    warnings: bool,
}

impl ArtObjectLayoutFitter {
    pub fn new(remaining: Vec<ArtObjectLayoutInfo>, warnings: bool) -> Self {
        ArtObjectLayoutFitter {
            unused: vec![],
            remaining,
            warnings,
        }
    }

    pub fn get_object_fitting_in(
        &mut self,
        max_width: f64,
        max_height: f64,
        walls: &Vec<GalleryWall>,
    ) -> Option<ArtObjectLayoutInfo> {
        let idx = self
            .unused
            .iter()
            .position(|art_object| can_object_fit_in(&art_object, max_width, max_height));
        if let Some(idx) = idx {
            return Some(self.unused.swap_remove(idx));
        }
        while let Some(art_object) = self.remaining.pop() {
            if can_object_fit_in(&art_object, max_width, max_height) {
                return Some(art_object);
            }
            if can_object_fit_anywhere(&art_object, &walls) {
                self.unused.push(art_object);
            } else if self.warnings {
                println!(
                    "Warning: object {:?} can't fit on any walls.",
                    art_object.id
                );
            }
        }

        None
    }

    pub fn is_empty(&self) -> bool {
        self.unused.is_empty() && self.remaining.is_empty()
    }

    pub fn get_remaining(&self) -> usize {
        self.unused.len() + self.remaining.len()
    }
}

fn can_object_fit_in(object_layout: &ArtObjectLayoutInfo, max_width: f64, max_height: f64) -> bool {
    object_layout.width < max_width && object_layout.height < max_height
}

fn can_object_fit_anywhere(object_layout: &ArtObjectLayoutInfo, walls: &Vec<GalleryWall>) -> bool {
    for wall in walls {
        if can_object_fit_in(
            object_layout,
            wall.width - PAINTING_HORIZ_MARGIN * 2.0,
            wall.height,
        ) {
            return true;
        }
    }
    false
}

pub fn place_paintings_along_wall<'a>(
    gallery_id: i64,
    walls: &Vec<GalleryWall>,
    wall_name: &'a str,
    finder: &mut ArtObjectLayoutFitter,
    x_start: f64,
    y_start: f64,
    max_width: f64,
    max_height: f64,
    center_vertically: bool,
    use_dense_layout: bool,
    layout_records: &mut Vec<LayoutRecord<&'a str>>,
    except_art_object_ids: &HashSet<ArtObjectId>,
) {
    let max_painting_width = max_width - PAINTING_HORIZ_MARGIN * 2.0;
    if max_painting_width <= 0.0 {
        return;
    }
    if let Some(art_object) = finder.get_object_fitting_in(max_painting_width, max_height, &walls) {
        let x = x_start + max_width / 2.0;
        let y = y_start
            + if center_vertically {
                let base_y = max_height / 2.0;
                if art_object.height < max_height - PAINTING_EYE_LEVEL_Y_OFFSET * 2.0 {
                    base_y - PAINTING_EYE_LEVEL_Y_OFFSET
                } else {
                    base_y
                }
            } else {
                max_height - art_object.height / 2.0 - PAINTING_VERT_MARGIN
            };
        let margin_height = y - art_object.height / 2.0;
        let margin_width = max_width / 2.0 - art_object.width / 2.0;

        // Note that even if the art object shouldn't be placed, we leave an empty space where it
        // would've been. This helps keep layouts consistent.
        if !except_art_object_ids.contains(&art_object.id) {
            layout_records.push(LayoutRecord {
                gallery_id,
                wall_id: &wall_name,
                art_object_id: art_object.id,
                x,
                y,
            });
        }

        // Only put at most one painting below the one that's centered vertically.
        if use_dense_layout && center_vertically {
            let vertical_space_below = margin_height - PAINTING_MIN_DISTANCE_FROM_FLOOR;
            let below_y_start = y_start + PAINTING_MIN_DISTANCE_FROM_FLOOR;
            if vertical_space_below > PAINTING_VERT_MIN_MOUNT_AREA {
                let left_edge =
                    x_start + (max_width / 2.0 - art_object.width / 2.0 - PAINTING_HORIZ_MARGIN);
                let right_edge =
                    x_start + (max_width / 2.0 + art_object.width / 2.0 + PAINTING_HORIZ_MARGIN);
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
                    except_art_object_ids,
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
                except_art_object_ids,
            );
            place_paintings_along_wall(
                gallery_id,
                walls,
                wall_name,
                finder,
                x_start + (max_width / 2.0 + art_object.width / 2.0),
                y_start,
                margin_width,
                max_height,
                center_vertically,
                use_dense_layout,
                layout_records,
                except_art_object_ids,
            );
        }
    }
}

pub fn layout<'a>(
    use_dense_layout: bool,
    gallery_start_id: i64,
    walls: &'a Vec<GalleryWall>,
    mut art_objects: Vec<ArtObjectLayoutInfo>,
    except_art_object_ids: &HashSet<ArtObjectId>,
    warnings: bool,
) -> Result<(usize, Vec<LayoutRecord<&'a str>>)> {
    // Reverse the objects, since we'll be popping them off the end of the vec.
    // This isn't terribly efficient but it'll do for now.
    art_objects.reverse();
    let mut finder = ArtObjectLayoutFitter::new(art_objects, warnings);
    let mut layout_records: Vec<LayoutRecord<&'a str>> = vec![];
    let mut wall_idx = 0;
    let mut gallery_id = gallery_start_id;
    let mut galleries_created: usize = 0;
    while !finder.is_empty() {
        let wall = walls.get(wall_idx).unwrap();
        place_paintings_along_wall(
            gallery_id,
            &walls,
            &wall.name,
            &mut finder,
            0.0,
            0.0,
            wall.width,
            wall.height,
            true,
            use_dense_layout,
            &mut layout_records,
            except_art_object_ids,
        );
        wall_idx += 1;
        if wall_idx == walls.len() {
            wall_idx = 0;
            gallery_id += 1;
            galleries_created += 1;
        }
    }
    if layout_records.len() > 0 {
        // We have to account for the very first gallery too.
        galleries_created += 1;
    }
    Ok((galleries_created, layout_records))
}
