use crate::functions;
use crate::table::Table;
use ordered_float::OrderedFloat;

pub trait Criterion {
    fn calculate<T>(&self, target: T) -> f64
    where
        T: Iterator<Item = f64> + Clone;
}

#[derive(Debug)]
pub struct Mse;

impl Criterion for Mse {
    fn calculate<T>(&self, target: T) -> f64
    where
        T: Iterator<Item = f64> + Clone,
    {
        let n = target.clone().count() as f64;
        let m = functions::mean(target.clone());
        target.map(|y| (y - m).powi(2)).sum::<f64>() / n
    }
}

#[derive(Debug)]
pub struct Tree {
    root: Node,
}

impl Tree {
    pub fn fit<'a>(mut table: Table<'a>, criterion: impl Criterion, classification: bool) -> Self {
        let mut builder = NodeBuilder {
            criterion,
            classification,
        };
        let root = builder.build(&mut table);
        Self { root }
    }

    pub fn predict(&self, xs: &[f64]) -> f64 {
        self.root.predict(xs)
    }
}

#[derive(Debug)]
pub struct Node {
    label: f64,
    children: Option<Children>,
}

impl Node {
    fn new(label: f64) -> Self {
        Self {
            label,
            children: None,
        }
    }

    fn predict(&self, xs: &[f64]) -> f64 {
        if let Some(children) = &self.children {
            if xs[children.split.column] <= children.split.threshold {
                children.left.predict(xs)
            } else {
                children.right.predict(xs)
            }
        } else {
            self.label
        }
    }
}

#[derive(Debug)]
pub struct Children {
    split: SplitPoint,
    left: Box<Node>,
    right: Box<Node>,
}

#[derive(Debug)]
struct SplitPoint {
    information_gain: f64,
    column: usize,
    threshold: f64,
}

#[derive(Debug)]
struct NodeBuilder<C> {
    criterion: C,
    classification: bool,
}

impl<C> NodeBuilder<C>
where
    C: Criterion,
{
    fn build(&mut self, table: &mut Table) -> Node {
        if table.is_single_target() {
            let label = table.target().nth(0).expect("never fails");
            return Node::new(label);
        }

        let label = if self.classification {
            functions::most_frequent(table.target())
        } else {
            functions::mean(table.target())
        };

        let mut node = Node::new(label);
        let mut best: Option<SplitPoint> = None;
        let impurity = self.criterion.calculate(table.target());
        let rows = table.target().count();

        for column in 0..table.features().len() {
            if table.features()[column].iter().any(|f| f.is_nan()) {
                continue;
            }

            table.sort_rows_by_feature(column);
            for (row, threshold) in table.thresholds(column) {
                let impurity_l = self.criterion.calculate(table.target().take(row));
                let impurity_r = self.criterion.calculate(table.target().skip(row));
                let n_l = row as f64 / rows as f64;
                let n_r = 1.0 - n_l;

                let information_gain = impurity - (n_l * impurity_l + n_r * impurity_r);
                if best
                    .as_ref()
                    .map_or(true, |t| t.information_gain < information_gain)
                {
                    best = Some(SplitPoint {
                        information_gain,
                        column,
                        threshold,
                    });
                }
            }
        }

        let best = best.expect("never fails");
        node.children = Some(self.build_children(table, best));
        node
    }

    fn build_children(&mut self, table: &mut Table, split: SplitPoint) -> Children {
        table.sort_rows_by_feature(split.column);
        let row = table.features()[split.column]
            .binary_search_by_key(&OrderedFloat(split.threshold), |&f| OrderedFloat(f))
            .unwrap_or_else(|i| i);
        let (left, right) = table.with_split(row, |table| Box::new(self.build(table)));
        Children { split, left, right }
    }
}
