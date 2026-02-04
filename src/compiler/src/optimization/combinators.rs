use std::{collections::VecDeque, iter::from_fn, marker::PhantomData, mem, sync::Arc};

pub trait Rewriter: Default {
    // Making `Component` and `GlobalData` generic makes it easier to test combinators and allows the framework to potentially be used for optimization passes in other parts of the compiler
    // DEFAULT: WithSpan<OptimizingCodeComponent>
    type Component;
    // DEFAULT: GlobalRegs
    type GlobalData;

    /// Feed an instruction into the rewriter and return any instructions that are known to be optimized as best as possible under the given rule
    fn rewrite(
        &mut self,
        component: Self::Component,
        global_regs: &Self::GlobalData,
    ) -> Vec<Self::Component>;

    /// Dump all instructions out of the rewriter
    fn eof(self, global_regs: &Self::GlobalData) -> Vec<Self::Component>;
}

pub fn push_to_pull<R: Rewriter + 'static>(
    rewriter: R,
    mut iter: impl Iterator<Item = R::Component> + 'static,
    global_regs: Arc<R::GlobalData>,
) -> impl Iterator<Item = R::Component> + 'static {
    let mut rewriter = Some(rewriter);
    let mut output_so_far = VecDeque::new();

    from_fn(move || {
        loop {
            if let Some(output) = output_so_far.pop_front() {
                return Some(output);
            }

            match iter.next() {
                Some(component) => output_so_far
                    .extend(rewriter.as_mut().unwrap().rewrite(component, &global_regs)),
                None => match rewriter.take() {
                    Some(this) => output_so_far.extend(Box::new(this).eof(&global_regs)),
                    None => return None,
                },
            }
        }
    })
}

impl<R1: Rewriter, R2: Rewriter<Component = R1::Component, GlobalData = R1::GlobalData>> Rewriter
    for (R1, R2)
{
    type Component = R1::Component;
    type GlobalData = R1::GlobalData;

    fn rewrite(
        &mut self,
        component: Self::Component,
        global_regs: &Self::GlobalData,
    ) -> Vec<Self::Component> {
        let first_out = self.0.rewrite(component, global_regs);

        let mut out = Vec::new();

        for component in first_out {
            out.extend(self.1.rewrite(component, global_regs));
        }

        out
    }

    fn eof(mut self, global_regs: &Self::GlobalData) -> Vec<Self::Component> {
        let first_out = self.0.eof(global_regs);

        let mut out = Vec::new();

        for component in first_out {
            out.extend(self.1.rewrite(component, global_regs));
        }

        out.extend(self.1.eof(global_regs));

        out
    }
}

struct Stages<R: Rewriter> {
    // INVARIANT: The `VecDeque` in each stage is empty before and after every `rewrite` call
    stages: VecDeque<(R, VecDeque<R::Component>)>,
}

impl<R: Rewriter> Default for Stages<R> {
    fn default() -> Self {
        Self {
            stages: VecDeque::new(),
        }
    }
}

impl<R: Rewriter> Rewriter for Stages<R> {
    type Component = R::Component;
    type GlobalData = R::GlobalData;

    fn rewrite(
        &mut self,
        component: Self::Component,
        global_regs: &Self::GlobalData,
    ) -> Vec<Self::Component> {
        if self.stages.is_empty() {
            return vec![component];
        }

        self.stages[0].1.push_front(component);

        let mut current = 0;

        let mut output_found = Vec::new();

        while current != 0 || !self.stages[0].1.is_empty() {
            if self.stages[current].1.is_empty() {
                current -= 1;
                continue;
            }

            while let Some(instr) = self.stages[current].1.pop_front() {
                let output = self.stages[current].0.rewrite(instr, global_regs);

                if current == self.stages.len() - 1 {
                    output_found.extend(output);
                    break;
                }

                current += 1;

                self.stages[current].1.extend(output);
            }
        }

        output_found
    }

    fn eof(mut self, global_regs: &Self::GlobalData) -> Vec<Self::Component> {
        let mut final_output = Vec::new();

        while let Some(stage) = self.stages.pop_front() {
            let stage_output = stage.0.eof(global_regs);

            for component in stage_output {
                final_output.extend(self.rewrite(component, global_regs));
            }
        }

        final_output
    }
}

pub struct RepeatUntilConvergence<R: Rewriter>
where
    R::Component: Clone + PartialEq + Eq,
{
    stages: Stages<R>,
    input_so_far: VecDeque<R::Component>,
    last_stage: R,
    output_so_far: Vec<R::Component>,
    correct_until: usize,
}

impl<R: Rewriter> Default for RepeatUntilConvergence<R>
where
    R::Component: Clone + PartialEq + Eq,
{
    fn default() -> Self {
        Self {
            stages: Stages::default(),
            input_so_far: VecDeque::new(),
            last_stage: Default::default(),
            output_so_far: Vec::new(),
            correct_until: Default::default(),
        }
    }
}

impl<R: Rewriter> RepeatUntilConvergence<R>
where
    R::Component: Clone + PartialEq + Eq,
{
    fn check_equality_and_maybe_new_stage(&mut self, global_regs: &R::GlobalData) {
        while !self.input_so_far.is_empty() && self.correct_until < self.output_so_far.len() {
            if self.input_so_far.pop_front().unwrap() != self.output_so_far[self.correct_until] {
                self.stages
                    .stages
                    .push_back((mem::take(&mut self.last_stage), VecDeque::new()));
                self.input_so_far = VecDeque::new();
                self.correct_until = 0;

                for component in mem::take(&mut self.output_so_far) {
                    self.input_so_far.push_back(component.clone());
                    self.output_so_far
                        .extend(self.last_stage.rewrite(component, global_regs));
                }

                self.check_equality_and_maybe_new_stage(global_regs);

                break;
            }

            self.correct_until += 1;
        }
    }
}

impl<R: Rewriter> Rewriter for RepeatUntilConvergence<R>
where
    R::Component: Clone + PartialEq + Eq,
{
    type Component = R::Component;
    type GlobalData = R::GlobalData;

    fn rewrite(
        &mut self,
        component: Self::Component,
        global_regs: &Self::GlobalData,
    ) -> Vec<Self::Component> {
        let stages_output = self.stages.rewrite(component, global_regs);

        for component in &stages_output {
            self.output_so_far
                .extend(self.last_stage.rewrite(component.clone(), global_regs));
        }

        self.input_so_far.extend(stages_output);
        self.check_equality_and_maybe_new_stage(global_regs);

        vec![]
    }

    fn eof(mut self, global_regs: &Self::GlobalData) -> Vec<Self::Component> {
        while !self.stages.stages.is_empty() {
            let (stage, _) = self.stages.stages.pop_back().unwrap();
            let components = stage.eof(global_regs);

            for component in components {
                self.rewrite(component, global_regs);
            }
        }

        let mut input = self.input_so_far.into();
        let mut output = mem::take(&mut self.output_so_far);
        output.extend(mem::take(&mut self.last_stage).eof(global_regs));

        while input != output {
            input = output;
            output = Vec::new();

            for component in input.clone() {
                output.extend(self.last_stage.rewrite(component, global_regs));
            }

            output.extend(mem::take(&mut self.last_stage).eof(global_regs));
        }

        output
    }
}

pub struct Peephole<R: PeepholeRewriter> {
    window: VecDeque<R::Component>,
    phantom_: PhantomData<R>,
}

impl<R: PeepholeRewriter> Peephole<R> {
    fn do_try_match(&mut self, global_regs: &R::GlobalData) -> Vec<R::Component> {
        R::try_match(&mut self.window, global_regs);

        if self.window.len() >= R::MAX_WINDOW_SIZE {
            self.window
                .drain(0..=(self.window.len() - R::MAX_WINDOW_SIZE))
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl<R: PeepholeRewriter> Default for Peephole<R> {
    fn default() -> Self {
        Peephole {
            window: VecDeque::new(),
            phantom_: PhantomData,
        }
    }
}

pub trait PeepholeRewriter {
    type Component;
    type GlobalData;

    /// Must not be zero
    const MAX_WINDOW_SIZE: usize;

    /// Rewrite the contents of `window`. This is required to be idempotent. `window` is guaranteed to have size at most `MAX_WINDOW_SIZE` but may be smaller.
    fn try_match(window: &mut VecDeque<Self::Component>, global_regs: &Self::GlobalData);
}

impl<R: PeepholeRewriter> Rewriter for Peephole<R> {
    type Component = R::Component;
    type GlobalData = R::GlobalData;

    fn rewrite(
        &mut self,
        component: Self::Component,
        global_regs: &Self::GlobalData,
    ) -> Vec<Self::Component> {
        self.window.push_back(component);

        self.do_try_match(global_regs)
    }

    fn eof(mut self, global_regs: &Self::GlobalData) -> Vec<Self::Component> {
        let mut out = Vec::new();

        loop {
            out.extend(self.do_try_match(global_regs));

            match self.window.pop_front() {
                Some(first) => out.push(first),
                _ => return out,
            }
        }
    }
}

pub trait GlobalRewriter {
    type Component;
    type GlobalData;

    fn rewrite(instructions: Vec<Self::Component>, data: &Self::GlobalData)
    -> Vec<Self::Component>;
}

pub struct Global<R: GlobalRewriter> {
    data: Vec<R::Component>,
    _phantom: PhantomData<R>,
}

impl<R: GlobalRewriter> Default for Global<R> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<R: GlobalRewriter> Rewriter for Global<R> {
    type Component = R::Component;
    type GlobalData = R::GlobalData;

    fn rewrite(
        &mut self,
        component: Self::Component,
        _: &Self::GlobalData,
    ) -> Vec<Self::Component> {
        self.data.push(component);
        Vec::new()
    }

    fn eof(self, global_regs: &Self::GlobalData) -> Vec<Self::Component> {
        R::rewrite(self.data, global_regs)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Arc};

    use itertools::Itertools;

    use crate::optimization::combinators::{
        Global, GlobalRewriter, Peephole, PeepholeRewriter, RepeatUntilConvergence, Rewriter,
        Stages, push_to_pull,
    };

    struct TestBuf(VecDeque<i32>, usize, fn(i32) -> i32);

    impl TestBuf {
        fn new(f: fn(i32) -> i32) -> TestBuf {
            TestBuf(VecDeque::new(), 1, f)
        }
    }

    impl Default for TestBuf {
        fn default() -> Self {
            TestBuf::new(|v| v / 2)
        }
    }

    impl Rewriter for TestBuf {
        type Component = i32;
        type GlobalData = ();

        fn rewrite(&mut self, component: Self::Component, (): &()) -> Vec<Self::Component> {
            self.0.push_back((self.2)(component));

            if self.0.len() > self.1 {
                let out = self.0.drain(0..self.1).collect();
                self.1 += 1;
                out
            } else {
                vec![]
            }
        }

        fn eof(self, (): &()) -> Vec<Self::Component> {
            self.0.into()
        }
    }

    #[test]
    fn test_push_to_pull() {
        assert_eq!(
            push_to_pull(TestBuf::default(), 0..10, Arc::new(())).collect::<Vec<_>>(),
            (0..10).map(|v| v / 2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_rewriter_pair() {
        let rw = (TestBuf::new(|v| v + 1), TestBuf::new(|v| v * 2));

        assert_eq!(
            push_to_pull(rw, 0..10, Arc::new(())).collect::<Vec<_>>(),
            (0..10).map(|v| (v + 1) * 2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_stages() {
        let mut stages = Stages::<TestBuf>::default();

        assert_eq!(stages.rewrite(10, &()), vec![10]);
        assert_eq!(stages.rewrite(5, &()), vec![5]);

        stages
            .stages
            .push_back((TestBuf::new(|v| v * 2), VecDeque::new()));

        assert_eq!(stages.rewrite(0, &()), vec![]);
        assert_eq!(stages.rewrite(1, &()), vec![0]);
        assert_eq!(stages.rewrite(2, &()), vec![]);
        assert_eq!(stages.rewrite(3, &()), vec![2, 4]);
        assert_eq!(stages.rewrite(4, &()), vec![]);

        let mut next_stage = TestBuf::new(|v| v + 1);
        assert_eq!(next_stage.rewrite(0, &()), vec![]);
        assert_eq!(next_stage.rewrite(2, &()), vec![1]);
        assert_eq!(next_stage.rewrite(4, &()), vec![]);

        stages.stages.push_back((next_stage, VecDeque::new()));
        assert_eq!(stages.rewrite(5, &()), vec![]);
        assert_eq!(stages.rewrite(6, &()), vec![3, 5]);

        assert_eq!(stages.eof(&()), vec![7, 9, 11, 13]);
    }

    #[test]
    fn test_repeat_until_convergence_1() {
        let rw = RepeatUntilConvergence::<TestBuf>::default();

        assert_eq!(
            push_to_pull(rw, 0..10, Arc::new(())).collect::<Vec<_>>(),
            vec![0; 10]
        );
    }

    #[test]
    fn test_repeat_until_convergence_2() {
        let rw = RepeatUntilConvergence::<TestBuf>::default();

        // Ensure that EOF is handled properly
        assert_eq!(
            push_to_pull(rw, vec![1000].into_iter(), Arc::new(())).collect::<Vec<_>>(),
            vec![0]
        );
    }

    struct SixSevenMemeRecognizer;

    impl PeepholeRewriter for SixSevenMemeRecognizer {
        type Component = i32;
        type GlobalData = ();

        const MAX_WINDOW_SIZE: usize = 3;

        fn try_match(window: &mut VecDeque<Self::Component>, (): &Self::GlobalData) {
            if window.len() < 2 {
                return;
            }

            if (window[0] % 10 == 6 && window[1] == 7) || (window[0] % 10 == 7 && window[1] == 6) {
                window[1] += window[0] * 10;
                window.pop_front();
            }
        }
    }

    #[test]
    fn test_peephole() {
        assert_eq!(
            push_to_pull(
                Peephole::<SixSevenMemeRecognizer>::default(),
                vec![0, 1, 6, 7, 6, 4, 3, 6, 7, 6, 7, 8, 9, 6, 7].into_iter(),
                Arc::new(())
            )
            .collect::<Vec<_>>(),
            vec![0, 1, 676, 4, 3, 6767, 8, 9, 67]
        );
    }

    struct IntoCounts;

    impl GlobalRewriter for IntoCounts {
        type Component = i32;
        type GlobalData = ();

        #[expect(clippy::cast_possible_truncation)]
        #[expect(clippy::cast_possible_wrap)]
        fn rewrite(
            mut instructions: Vec<Self::Component>,
            (): &Self::GlobalData,
        ) -> Vec<Self::Component> {
            instructions.sort_unstable();
            instructions
                .into_iter()
                .dedup_with_count()
                .flat_map(|(a, b)| [b, a as i32])
                .collect()
        }
    }

    #[test]
    fn test_global() {
        assert_eq!(
            push_to_pull(
                Global::<IntoCounts>::default(),
                [0, 1, 2, 1, 0, 4, 100, 3, 4, 1].into_iter(),
                Arc::new(()),
            )
            .collect_vec(),
            vec![0, 2, 1, 3, 2, 1, 3, 1, 4, 2, 100, 1]
        );
    }
}
