mod plugin;

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, criterion_group, criterion_main,
    measurement::WallTime,
};

use telarel::{CompileOptions, SharedPluginable, compile};

use crate::plugin::TransformPlugin;

const FIXTURES: [&str; 1] = ["react-page.tsx"];

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures")
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|error| panic!("read fixture {name}: {error}"))
}

fn options(code: String) -> CompileOptions {
    CompileOptions {
        cwd: "/repo".to_string(),
        file: "index.tsx".to_string(),
        code,
    }
}

fn bench_case(
    criterion: &mut Criterion,
    runtime: &tokio::runtime::Runtime,
    case: &str,
    plugins: &[SharedPluginable],
) {
    let mut group: BenchmarkGroup<'_, WallTime> =
        criterion.benchmark_group("compile");

    for fixture in FIXTURES {
        let code: String = read_fixture(fixture);

        let plugins: Vec<SharedPluginable> = plugins.to_vec();

        group.bench_with_input(
            BenchmarkId::new(case, fixture),
            &(code, plugins),
            |bencher, (code, plugins)| {
                bencher.to_async(runtime).iter(|| {
                    let code: String = code.clone();

                    let plugins: Vec<SharedPluginable> = plugins.clone();

                    async {
                        let output: telarel::CompileOutput =
                            compile(options(code), plugins).await.unwrap();

                        black_box(output.code.len());
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_compile(criterion: &mut Criterion) {
    let runtime: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build current-thread tokio runtime");

    bench_case(criterion, &runtime, "common", &[]);
    bench_case(criterion, &runtime, "plugin", &[Arc::new(TransformPlugin)]);
}

criterion_group!(benches, bench_compile);
criterion_main!(benches);
