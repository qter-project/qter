use std::collections::HashMap;

use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::span::WithSpan;

use crate::{
    LabelReference,
    optimization::{OptimizingPrimitive, combinators::GlobalRewriter},
    primitive_match,
    strip_expanded::GlobalRegs,
};

use super::OptimizingCodeComponent;

pub struct DeadLabelRemover;

impl GlobalRewriter for DeadLabelRemover {
    type Component = WithSpan<OptimizingCodeComponent>;
    type GlobalData = GlobalRegs;

    fn rewrite(instructions: Vec<Self::Component>, _: &Self::GlobalData) -> Vec<Self::Component> {
        let mut label_locations = HashMap::new();
        let mut program_counter = 0;

        let instructions = instructions
            .into_iter()
            .inspect(|component| {
                if let OptimizingCodeComponent::Label(label) = &**component {
                    label_locations.insert(
                        LabelReference {
                            name: ArcIntern::clone(&label.name),
                            block_id: label.maybe_block_id.unwrap(),
                        },
                        false,
                    );
                }

                program_counter += 1;
            })
            .collect_vec();

        for instruction in &instructions {
            primitive_match!((OptimizingPrimitive::Goto { label } | OptimizingPrimitive::SolvedGoto { label, .. }) = Some(instruction); else { continue; });

            let Some(is_seen) = label_locations.get_mut(&LabelReference {
                name: ArcIntern::clone(&label.name),
                block_id: label.block_id,
            }) else {
                continue;
            };

            *is_seen = true;
        }

        instructions
            .into_iter()
            .filter(move |component| {
                let OptimizingCodeComponent::Label(label) = &**component else {
                    return true;
                };

                *label_locations
                    .get(&LabelReference {
                        name: ArcIntern::clone(&label.name),
                        block_id: label.maybe_block_id.unwrap(),
                    })
                    .unwrap()
            })
            .collect()
    }
}
