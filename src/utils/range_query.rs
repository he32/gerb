/*
 * gerb
 *
 * Copyright 2022 - Manos Pitsidianakis
 *
 * This file is part of gerb.
 *
 * gerb is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * gerb is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with gerb. If not, see <http://www.gnu.org/licenses/>.
 */

use super::{IPoint, Point};

use generational_arena::{Arena, Index};
use std::cmp::Ordering;
use uuid::Uuid;

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Coordinate {
    X,
    Y,
}

impl Coordinate {
    #[inline(always)]
    fn next(self) -> Self {
        match self {
            Coordinate::X => Coordinate::Y,
            Coordinate::Y => Coordinate::X,
        }
    }
}

/*
#[test]
fn test_range_query() {
    let points = vec![
        (11, 52),
        (12, 53),
        (26, 328),
        (27, 328),
        (27, 338),
        (27, 339),
        (53, 72),
        (54, 55),
        (54, 72),
        (55, 298),
        (55, 299),
        (61, 333),
        (96, 209),
        (97, 209),
        (97, 301),
        (98, 306),
        (98, 334),
        (100, 180),
        (100, 79),
        (101, 78),
        (105, 176),
        (123, 330),
        (123, 330),
        (124, 337),
        (124, 339),
        (125, 78),
        (126, 79),
        (135, 176),
        (136, 177),
        (138, 209),
        (140, 210),
        (174, 53),
        (174, 55),
        (183, 192),
        (183, 201),
        (187, 192),
        (203, 243),
        (205, 245),
        (209, 67),
        (215, 296),
        (216, 120),
        (217, 178),
        (218, 122),
        (241, 327),
        (241, 327),
        (244, 197),
        (251, 63),
        (253, 237),
        (252, 237),
        (253, 241),
        (263, 304),
        (265, 182),
        (266, 124),
        (266, 127),
        (287, 359),
        (290, 317),
        (292, 317),
        (316, 318),
        (332, 294),
        (335, 295),
        (339, 301),
        (339, 303),
    ];
    let hash_set = points
        .clone()
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let mut points = hash_set.into_iter().collect::<Vec<_>>();
    //let idx_points = points
    //    .clone()
    //    .into_iter()
    //    .enumerate()
    //    .collect::<Vec<(usize, _)>>();
    //let kd_tree = KDTree::new(&mut points);
    //println!("{:?}", kd_tree);
    //println!("{:?}", kd_tree.query((135, 176), RADIUS as usize));
    let mut kd_tree = KdTree::new(&points);
    println!("{:?}\n", kd_tree.query(std::dbg!((135, 176)), 2));
    let mut results = kd_tree.query((135, 176), 2);
    if let Some((i, p)) = results.pop() {
        kd_tree.remove(p, i);
    }
    println!("{:?}\n", kd_tree.query(std::dbg!((135, 176)), 2));
    if let Some((i, p)) = results.pop() {
        kd_tree.remove(p, i);
    }
    println!("{:?}\n", kd_tree.query(std::dbg!((135, 176)), 2));
    //println!("{:#?}\n\n", idx_points);
    //println!("{}", kd_tree.to_svg());
    //let range_tree = RangeTree::new(&points).unwrap();
    //println!("{:?}", range_tree.query(136, 177));
    //println!("\n\n{:?}", range_tree.query2(136, 177));
    //range_tree.to_svg("./range_tree.svg");
}
*/

macro_rules! contains {
    ($range:expr, $point:expr) => {{
        let (bottom_left, top_right) = $range;
        let IPoint { x, y, .. } = $point;
        x >= bottom_left.x && x <= top_right.x && y >= bottom_left.y && y <= top_right.y
    }};
}

macro_rules! intersects {
    ($self:expr, $other:expr) => {{
        let (a1, a2) = $self;
        let (b1, b2) = $other;
        a1.x <= b2.x && a2.x >= b1.x && a1.y <= b2.y && a2.y >= b1.y
    }};
}

#[inline(always)]
fn max_point(a: IPoint, b: IPoint) -> IPoint {
    IPoint {
        x: std::cmp::max(a.x, b.x),
        y: std::cmp::max(a.y, b.y),
    }
}

#[inline(always)]
fn min_point(a: IPoint, b: IPoint) -> IPoint {
    IPoint {
        x: std::cmp::min(a.x, b.x),
        y: std::cmp::min(a.y, b.y),
    }
}

#[derive(Debug, Clone)]
pub enum KdNode<Identifier: std::fmt::Debug + Copy, const N: usize> {
    Leaf {
        split_at: Coordinate,
        min: IPoint,
        max: IPoint,
        //TODO: use an inline array instead of Vec
        points: Vec<(Identifier, IPoint)>,
        size: usize,
    },
    Division {
        split_value: i64,
        split_at: Coordinate,
        min: IPoint,
        max: IPoint,
        size: usize,
        left: Index,
        right: Index,
    },
}

#[derive(Debug)]
struct TempLeaf<Identifier: std::fmt::Debug> {
    split_at: Coordinate,
    min: IPoint,
    max: IPoint,
    points: Vec<(Identifier, IPoint)>,
    size: usize,
}

impl<I: std::fmt::Debug + Copy + std::cmp::PartialEq, const N: usize> KdNode<I, N> {
    fn split(index: Index, arena: &mut Arena<Self>) {
        if let Some(KdNode::Leaf {
            split_at,
            min,
            max,
            points,
            size: _,
        }) = arena.get_mut(index)
        {
            let min = *min;
            let max = *max;
            let next_split = *split_at;
            let split_value = median(points.as_slice(), next_split).unwrap() as i64;
            //let split_value = match next_split {
            //    Coordinate::X => min.x + (max.x - min.x) / 2,
            //    Coordinate::Y => min.y + (max.y - min.y) / 2,
            //};
            let mut left = TempLeaf {
                split_at: next_split.next(),
                min: MAX_IPOINT,
                max: MIN_IPOINT,
                points: vec![],
                size: 0,
            };
            let mut right = TempLeaf {
                split_at: next_split.next(),
                min: MAX_IPOINT,
                max: MIN_IPOINT,
                points: vec![],
                size: 0,
            };
            while !points.is_empty() {
                let (i, next_point) = points.swap_remove(0);
                if match next_split {
                    Coordinate::X => next_point.x <= split_value,
                    Coordinate::Y => next_point.y <= split_value,
                } {
                    /* belongs to left subtree */
                    left.min = min_point(left.min, next_point);
                    left.max = max_point(left.max, next_point);
                    left.size += 1;
                    left.points.push((i, next_point));
                } else {
                    /* belongs to right subtree */
                    right.min = min_point(right.min, next_point);
                    right.max = max_point(right.max, next_point);
                    right.size += 1;
                    right.points.push((i, next_point));
                }
            }
            let left_size = left.size;
            let right_size = right.size;
            let left = arena.insert(KdNode::Leaf {
                split_at: left.split_at,
                min: left.min,
                max: left.max,
                points: left.points,
                size: left.size,
            });
            let right = arena.insert(KdNode::Leaf {
                split_at: right.split_at,
                min: right.min,
                max: right.max,
                points: right.points,
                size: right.size,
            });
            *arena.get_mut(index).unwrap() = KdNode::Division {
                split_value,
                split_at: next_split,
                min,
                max,
                left,
                right,
                size: left_size + right_size,
            };
            if left_size > N && right_size != 0 {
                Self::split(left, arena);
            }
            if right_size > N && left_size != 0 {
                Self::split(right, arena);
            }
        } else {
            unreachable!()
        }
    }

    fn insert(mut index: Index, point: impl Into<IPoint>, identifier: I, arena: &mut Arena<Self>) {
        let point: IPoint = point.into();

        loop {
            match arena.get_mut(index) {
                Some(KdNode::Leaf {
                    split_at: _,
                    min,
                    max,
                    points,
                    size,
                }) => {
                    *size += 1;
                    *min = min_point(*min, point);
                    *max = max_point(*max, point);
                    points.push((identifier, point));
                    if *size > N {
                        /*split leaf*/
                        Self::split(index, arena);
                    }
                    break;
                }
                Some(KdNode::Division {
                    split_value,
                    split_at,
                    min,
                    max,
                    left,
                    right,
                    size: _,
                }) => {
                    *min = min_point(*min, point);
                    *max = max_point(*max, point);
                    if match split_at {
                        Coordinate::X => point.x <= *split_value,
                        Coordinate::Y => point.y <= *split_value,
                    } {
                        /* belongs to left subtree */
                        index = *left;
                    } else {
                        /* belongs to right subtree */
                        index = *right;
                    }
                }
                None => {
                    unreachable!()
                }
            }
        }
    }

    fn create(points: &[(I, IPoint)], depth: usize, arena: &mut Arena<Self>) -> Index {
        let split_at = if depth % 2 == 0 {
            Coordinate::X
        } else {
            Coordinate::Y
        };
        if points.len() == 1 {
            return arena.insert(KdNode::Leaf {
                split_at,
                size: 1,
                min: points[0].1,
                max: points[0].1,
                points: vec![points[0]],
            });
        }

        let size = points.len();
        let split_value = median(points, split_at).unwrap() as i64;

        let mut left = vec![];
        let mut right = vec![];

        let mut min = MAX_IPOINT;
        let mut max = MIN_IPOINT;

        for (i, next_point) in points.iter() {
            min = min_point(min, *next_point);
            max = max_point(max, *next_point);
            if match split_at {
                Coordinate::X => next_point.x <= split_value,
                Coordinate::Y => next_point.y <= split_value,
            } {
                /* belongs to left subtree */
                left.push((*i, *next_point));
            } else {
                /* belongs to right subtree */
                right.push((*i, *next_point));
            }
        }

        if left.is_empty() || right.is_empty() {
            let points = if left.is_empty() { right } else { left };
            return arena.insert(KdNode::Leaf {
                split_at,
                size: points.len(),
                min,
                max,
                points,
            });
        }
        let left = Self::create(&left, depth + 1, arena);
        let right = Self::create(&right, depth + 1, arena);

        arena.insert(KdNode::Division {
            split_value,
            split_at,
            min,
            max,
            size,
            left,
            right,
        })
    }

    fn remove(
        mut index: Index,
        point: impl Into<IPoint>,
        identifier: I,
        arena: &mut Arena<Self>,
    ) -> bool {
        let point: IPoint = point.into();

        let mut path = vec![];
        let mut update_path = None;
        let mut ret = false;
        loop {
            path.push(index);
            match arena.get_mut(index) {
                Some(KdNode::Leaf {
                    split_at: _,
                    min,
                    max,
                    points,
                    size,
                }) => {
                    if let Some(pos) = points.iter().position(|e| *e == (identifier, point)) {
                        *size -= 1;
                        points.swap_remove(pos);
                        ret = true;
                    }
                    if !points.is_empty() {
                        let mut new_min = MAX_IPOINT;
                        let mut new_max = MIN_IPOINT;

                        for (_, p) in points.iter() {
                            new_min = min_point(new_min, *p);
                            new_max = max_point(new_max, *p);
                        }

                        *min = new_min;
                        *max = new_max;
                    }
                    path.pop();
                    if let Some(parent) = path.pop() {
                        update_path = Some(((index, parent), (*min, *max)));
                    }
                    break;
                }
                Some(KdNode::Division {
                    split_value,
                    split_at,
                    min: _,
                    max: _,
                    left,
                    right,
                    size: _,
                }) => {
                    if match split_at {
                        Coordinate::X => point.x <= *split_value,
                        Coordinate::Y => point.y <= *split_value,
                    } {
                        /* belongs to left subtree */
                        index = *left;
                    } else {
                        /* belongs to right subtree */
                        index = *right;
                    }
                }
                None => {
                    unreachable!()
                }
            }
        }

        while let Some(((leaf, index), (leaf_min, leaf_max))) = update_path.take() {
            match arena.get_mut(index) {
                Some(KdNode::Leaf { .. }) => {
                    unreachable!()
                }
                Some(KdNode::Division {
                    split_value: _,
                    split_at: _,
                    min,
                    max,
                    left,
                    right,
                    size: _,
                }) => {
                    if *left == leaf {
                        *min = std::cmp::min(*min, leaf_min);
                    } else if *right == leaf {
                        *max = std::cmp::max(*max, leaf_max);
                    } else {
                        unreachable!()
                    }
                    if let Some(parent) = path.pop() {
                        update_path = Some(((index, parent), (*min, *max)));
                    }
                }
                None => {
                    unreachable!()
                }
            }
        }
        ret
    }
}

const MAX_IPOINT: IPoint = IPoint {
    x: i64::MAX,
    y: i64::MAX,
};
const MIN_IPOINT: IPoint = IPoint {
    x: i64::MIN,
    y: i64::MIN,
};

type TDArena = Arena<KdNode<((usize, usize), Uuid), 2>>;

#[derive(Debug, Clone, Default)]
pub struct KdTree {
    arena: TDArena,
    size: usize,
    min: IPoint,
    max: IPoint,
    root: Option<Index>,
}

impl KdTree {
    /*pub fn new2(points: &[Point]) -> Self {
        let mut ret = Self {
            arena: Arena::new(),
            size: 0,
            min: MAX_IPOINT,
            max: MIN_IPOINT,
            root: None,
        };

        for p in points.iter().cloned() {
            ret.add(p);
        }

        ret
    }
    */

    pub fn new(points: &[((usize, usize), Point)]) -> Self {
        let mut arena = Arena::new();
        let mut min = MAX_IPOINT;
        let mut max = MIN_IPOINT;

        for (_, p) in points.iter() {
            min = min_point(min, p.into());
            max = max_point(max, p.into());
        }

        let root = if points.is_empty() {
            None
        } else {
            Some(KdNode::create(
                &points
                    .iter()
                    .cloned()
                    .map(|(i, p)| ((i, p.uuid), p.into()))
                    .collect::<Vec<_>>(),
                0,
                &mut arena,
            ))
        };
        Self {
            arena,
            size: points.len(),
            min,
            max,
            root,
        }
    }

    pub fn add(&mut self, identifier: ((usize, usize), Uuid), point: Point) {
        let point: IPoint = point.into();

        self.size += 1;
        self.min = min_point(self.min, point);
        self.max = max_point(self.max, point);
        let root = if let Some(root) = self.root {
            root
        } else {
            let index = self.arena.insert(KdNode::Leaf {
                split_at: Coordinate::X,
                size: 1,
                min: point,
                max: point,
                points: vec![(identifier, point)],
            });
            self.root = Some(index);
            return;
        };

        KdNode::insert(root, point, identifier, &mut self.arena);
    }

    pub fn remove(&mut self, identifier: ((usize, usize), Uuid), point: Point) -> bool {
        let point: IPoint = point.into();

        let root = if let Some(root) = self.root {
            root
        } else {
            return false;
        };

        if KdNode::remove(root, point, identifier, &mut self.arena) {
            self.size -= 1;
            true
        } else {
            false
        }
    }

    pub fn query_region(
        &self,
        (u, l): (impl Into<IPoint>, impl Into<IPoint>),
    ) -> Vec<(((usize, usize), Uuid), IPoint)> {
        let query_region = (u.into(), l.into());
        let root = if let Some(root) = self.root {
            root
        } else {
            return vec![];
        };

        fn report_subtree(
            root: Index,
            ret: &mut Vec<(((usize, usize), Uuid), IPoint)>,
            arena: &TDArena,
        ) {
            let mut queue = vec![root];
            while let Some(v) = queue.pop() {
                match arena.get(v) {
                    Some(KdNode::Leaf { points, .. }) => {
                        ret.extend(points.iter().cloned());
                    }
                    Some(KdNode::Division { left, right, .. }) => {
                        queue.push(*left);
                        queue.push(*right);
                    }
                    None => {}
                }
            }
        }

        let mut ret = vec![];
        let mut queue = vec![root];
        while let Some(v) = queue.pop() {
            match self.arena.get(v) {
                Some(KdNode::Leaf {
                    min, max, points, ..
                }) => {
                    if intersects!((min, max), query_region) {
                        ret.extend(
                            points
                                .iter()
                                .filter(|(_, p)| contains!(query_region, *p))
                                .cloned(),
                        );
                    }
                }
                Some(KdNode::Division {
                    split_value,
                    split_at,
                    min,
                    max,
                    left,
                    right,
                    size: _,
                }) => {
                    /* for each subtree check:
                     * - is it fully contained in the query range? then report_subtree
                     * - else does its range intersect the query range? then add it to the
                     *   queue
                     */
                    if contains!(query_region, *min) && contains!(query_region, *max) {
                        report_subtree(v, &mut ret, &self.arena);
                    } else {
                        let (left_split, right_split): ((IPoint, IPoint), (IPoint, IPoint)) =
                            match split_at {
                                Coordinate::X => (
                                    (*min, (*split_value, max.y).into()),
                                    ((*split_value, min.y).into(), *max),
                                ),
                                Coordinate::Y => (
                                    (*min, (max.x, *split_value).into()),
                                    ((min.x, *split_value).into(), *max),
                                ),
                            };

                        if intersects!(left_split, query_region) {
                            queue.push(*left);
                        }
                        if intersects!(right_split, query_region) {
                            queue.push(*right);
                        }
                    }
                }
                None => {}
            }
        }
        ret
    }

    pub fn query_point(
        &self,
        center: impl Into<IPoint>,
        radius: i64,
    ) -> Vec<(((usize, usize), Uuid), IPoint)> {
        let center: IPoint = center.into();

        /// Overflow guard
        macro_rules! o {
            ($left:expr, - $right:expr) => {{
                let (result, overflow_flag) = $left.overflowing_sub($right);
                if overflow_flag {
                    return vec![];
                }
                result
            }};
            ($left:expr, + $right:expr) => {{
                let (result, overflow_flag) = $left.overflowing_add($right);
                if overflow_flag {
                    return vec![];
                }
                result
            }};
        }

        let query_region: (IPoint, IPoint) = (
            IPoint {
                x: o! { center.x, - radius / 2 },
                y: o! { center.y, - radius / 2 },
            },
            IPoint {
                x: o! { center.x, + radius / 2 },
                y: o! { center.y, + radius / 2 },
            },
        );

        self.query_region(query_region)
    }

    #[cfg(test)]
    fn depth(&self, root: Index) -> usize {
        let mut ret = 1;
        match self.arena.get(root) {
            Some(KdNode::Leaf { .. }) => {}
            Some(KdNode::Division {
                split_value: _,
                split_at: _,
                min: _,
                max: _,
                left,
                right,
                size: _,
            }) => {
                ret += self.depth(*left);
                ret += self.depth(*right);
            }
            None => {}
        }
        ret
    }

    #[cfg(test)]
    fn new_group(
        &self,
        root: Index,
        desc: String,
        depth: usize,
        output: &mut Vec<String>,
        group_ctr: &mut usize,
        queue: &mut Vec<Index>,
        max_depth: usize,
    ) {
        const WIDTHS: &[f64] = &[1.5, 1., 0.6, 0.3, 0.1, 0.05];
        let stroke_width = WIDTHS.get(depth).unwrap_or_else(|| WIDTHS.last().unwrap());
        *group_ctr += 1;
        let tx = |x| x - self.min.x;
        let ty = |y| y - self.min.y;
        queue.push(root);
        let group_id = format!("{counter}-{depth}", counter = group_ctr, depth = depth);
        output.push(format!(
            r#"<g id="{group_id}"><desc>{desc}</desc>"#,
            group_id = group_id,
            desc = desc
        ));
        while let Some(v) = queue.pop() {
            match self.arena.get(v) {
                Some(KdNode::Leaf {
                    min, max, points, ..
                }) => {
                    if !points.is_empty() {
                        let rect_width = (max.x - min.x).abs() + 6;
                        let rect_height = (max.y - min.y).abs() + 6;
                        output.push(format!(r#"<rect id="{desc}" x="{}" y="{}" width="{width}" height="{height}" fill="none" stroke="black" stroke-width="{stroke_width}"><desc>{desc}</desc></rect>"#, tx(min.x-3), ty(min.y-3), desc=format!("min: {:?}\nmax: {:?}\npoints: {:?}", min, max, points), width=rect_width, height=rect_height, stroke_width=stroke_width));
                        for (i, p) in points {
                            //output.push(format!(
                            //        r#" <text x="{}" y="{}">p{}<desc>{desc}</desc></text>"#,
                            //        tx(p.x as i64),
                            //        ty(p.y as i64 - 5),
                            //        i,
                            //        desc = format!("i {} p {:?}", i, p)
                            //));
                            output.push(format!(
                                        r#"<circle id="{desc}" cx="{}" cy="{}" r="1" fill="none" stroke="black" stroke-width="0.2"><desc>{desc}</desc></circle>"#,
                                        tx(p.x), ty(p.y), desc=format!("{:?} : {:?}", i, p),
                                ));
                        }
                    }
                }
                Some(KdNode::Division {
                    split_value,
                    split_at,
                    min,
                    max,
                    left,
                    right,
                    size,
                }) => {
                    match split_at {
                        Coordinate::X => {
                            output.push(format!(r#"<path id="{desc}" d="M {} {} L {} {}" stroke="{color}" fill="none" stroke-width="{width}"><desc>{desc}</desc></path>"#, tx(*split_value), ty(min.y), tx(*split_value), ty(max.y), color="red", desc=format!("split_val {} at {:?} size {}", split_value, split_at, size), width=stroke_width));
                        }
                        Coordinate::Y => {
                            output.push(format!(r#"<path id="{desc}" d="M {} {} L {} {}" stroke="{color}" fill="none" stroke-width="{width}"><desc>{desc}</desc></path>"#, tx(min.x), ty(*split_value), tx(max.x), ty(*split_value), color="blue", desc=format!("split_val {} at {:?} size {}", split_value, split_at, size), width=stroke_width));
                        }
                    }

                    self.new_group(
                        *left,
                        format!(
                            "left subtree of {}, split at {:?} val {} size {}",
                            group_id, split_at, split_value, size
                        ),
                        depth + 1,
                        output,
                        group_ctr,
                        queue,
                        max_depth,
                    );
                    self.new_group(
                        *right,
                        format!(
                            "right subtree of {}, split at {:?} val {} size {}",
                            group_id, split_at, split_value, size
                        ),
                        depth + 1,
                        output,
                        group_ctr,
                        queue,
                        max_depth,
                    );
                }
                None => {}
            }
        }
        output.push("</g>".to_string());
    }

    #[cfg(test)]
    fn to_svg(&self) -> String {
        let mut output = vec![];
        let (width, height) = (
            (self.max.x - self.min.x).abs(),
            (self.max.y - self.min.y).abs(),
        );
        output.push(format!(
            r#"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">"#,
            width, height
        ));
        if self.size == 0 {
            output.push("</svg>".to_string());
            return output.join("\n");
        }
        let root = self.root.unwrap();
        let mut queue = vec![];
        let mut group_ctr = 0;
        // fn new_group(&self, root: Index, desc: String, depth: usize, output: &mut Vec<String>, group_ctr: &mut usize, queue: &mut Vec<Index>) {
        let max_depth = self.depth(root);
        self.new_group(
            root,
            "root".to_string(),
            0,
            &mut output,
            &mut group_ctr,
            &mut queue,
            max_depth,
        );
        //output.push(format!(
        //    r#"  <path d="M {} {} L {} {}" stroke="{color}" fill="none"/>"#,
        //    prev_point.x as i64,
        //    prev_point.y as i64,
        //    new_point.x as i64,
        //    new_point.y as i64,
        //    color = colors[i % colors.len()]
        //));
        //output.push(format!(
        //    r#" <text x="{}" y="{}">{}</text>"#,
        //    (prev_point.x as i64 + new_point.x as i64) / 2,
        //    (prev_point.y as i64 + new_point.y as i64) / 2,
        //    pp,
        //));
        output.push("</svg>".to_string());
        output.join("\n")
    }
}

fn partition<I: Copy>(
    data: &[(I, IPoint)],
    c: Coordinate,
) -> Option<(Vec<(I, IPoint)>, i64, Vec<(I, IPoint)>)> {
    match data.len() {
        0 => None,
        _ => {
            let (pivot_slice, tail) = data.split_at(1);
            let pivot = match c {
                Coordinate::X => pivot_slice[0].1.x,
                Coordinate::Y => pivot_slice[0].1.y,
            };
            let (left, right) = tail.iter().fold((vec![], vec![]), |mut splits, next| {
                {
                    let (ref mut left, ref mut right) = &mut splits;
                    if match c {
                        Coordinate::X => next.1.x < pivot,
                        Coordinate::Y => next.1.y < pivot,
                    } {
                        left.push(*next);
                    } else {
                        right.push(*next);
                    }
                }
                splits
            });

            Some((left, pivot, right))
        }
    }
}

fn select<I: Copy>(data: &[(I, IPoint)], k: usize, c: Coordinate) -> Option<i64> {
    let part = partition(data, c);

    match part {
        None => None,
        Some((left, pivot, right)) => {
            let pivot_idx = left.len();

            match pivot_idx.cmp(&k) {
                Ordering::Equal => Some(pivot),
                Ordering::Greater => select(&left, k, c),
                Ordering::Less => select(&right, k - (pivot_idx + 1), c),
            }
        }
    }
}

fn median<I: Copy>(data: &[(I, IPoint)], c: Coordinate) -> Option<f64> {
    let size = data.len();

    match size {
        even if even % 2 == 0 => {
            let fst_med = select(data, (even / 2) - 1, c);
            let snd_med = select(data, even / 2, c);

            match (fst_med, snd_med) {
                (Some(fst), Some(snd)) => Some((fst + snd) as f64 / 2.0),
                _ => None,
            }
        }
        odd => select(data, odd / 2, c).map(|x| x as f64),
    }
}
