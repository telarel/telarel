mod plugins;

use std::hint::black_box;
use std::path::{Path, PathBuf};

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, criterion_group, criterion_main,
    measurement::WallTime,
};

use telarel::{CompileOptions, Plugin, SharedPluginable, compile};

use crate::plugins::component_rename::ComponentRenamePlugin;
use crate::plugins::import_rewrite::ImportRewritePlugin;
use crate::plugins::jsx_attribute::JsxAttributePlugin;
use crate::plugins::void_fold::VoidFoldPlugin;

const FIXTURES: [&str; 1] = ["react-page.tsx"];

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures")
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|error| panic!("read fixture {name}: {error}"))
}

fn options(code: String) -> CompileOptions {
    CompileOptions { file: "index.tsx".to_string(), code, ..Default::default() }
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

    bench_case(criterion, &runtime, "plugin-0", &[]);

    bench_case(
        criterion,
        &runtime,
        "plugin-1",
        &[Plugin::new_shared(ImportRewritePlugin)],
    );

    bench_case(
        criterion,
        &runtime,
        "plugin-2",
        &[
            Plugin::new_shared(ImportRewritePlugin),
            Plugin::new_shared(ComponentRenamePlugin),
        ],
    );

    bench_case(
        criterion,
        &runtime,
        "plugin-4",
        &[
            Plugin::new_shared(ImportRewritePlugin),
            Plugin::new_shared(ComponentRenamePlugin),
            Plugin::new_shared(VoidFoldPlugin),
            Plugin::new_shared(JsxAttributePlugin),
        ],
    );
}

criterion_group!(benches, bench_compile);
criterion_main!(benches);
