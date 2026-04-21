use std::{borrow::Cow, iter::repeat_with, num::NonZeroU16, time::Duration};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use cycle_combination_finder::{
    orderexps::OrderExps,
    possible_orders::{LcmOrders, OrdersDashSet, OrdersSet},
    trie::MaxOrderTrie,
};
use rayon::ThreadPoolBuilder;

fn mk_data<const N: usize>(count: usize) -> Vec<OrderExps<N>> {
    repeat_with(|| fastrand::u16(..))
        .filter_map(|x| OrderExps::<N>::try_from(NonZeroU16::new(x.max(1)).unwrap()).ok())
        .take(count)
        .collect::<Vec<_>>()
}

fn trie_lcm_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_lcm_benchmark");

    const SMALL: usize = 10usize.pow(2);
    const MID: usize = 10usize.pow(3);
    const BIG: usize = 10usize.pow(4);

    ThreadPoolBuilder::new().build_global().unwrap();

    group.measurement_time(Duration::from_secs(1));
    group.sample_size(10);
    for producer in [SMALL, MID, BIG] {
        for walker in [SMALL, MID, BIG] {
            group.throughput(Throughput::Elements(
                u64::try_from(producer * walker).unwrap(),
            ));
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("producer_{producer}_walker_{walker}")),
                &(producer, walker),
                |b, &(producer, walker)| {
                    b.iter(|| {
                        let producer = mk_data::<8>(producer);
                        let walker = mk_data::<8>(walker);
                        let out = OrdersDashSet::default();
                        let mut root = MaxOrderTrie::new(0);
                        for order in producer {
                            root.insert(order);
                        }
                        root.par_collect_distinct_orders(
                            Cow::Owned(LcmOrders::OrbitOrders(OrdersSet::from_iter(walker))),
                            &out,
                        );
                        out
                    })
                },
            );
        }
    }
}

criterion_group!(benches, trie_lcm_benchmark);
criterion_main!(benches);
