use oxc::transformer::Engine;
use oxc_compat::Version;

/// Build an oxc `Version` with only the major component set.
///
/// Engine majors exceeding the `u16` field saturate to `u16::MAX`,
/// keeping the resolution total.
fn major_version(major: u32) -> Version {
    let major: u16 = u16::try_from(major).unwrap_or(u16::MAX);
    Version(major, 0, 0)
}

/// A compile target for the transform plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransformTarget {
    Es2015,
    Es2016,
    Es2017,
    Es2018,
    Es2019,
    Es2020,
    Es2021,
    Es2022,
    Es2023,
    Es2024,
    Es2025,
    Es2026,
    EsNext,
    Chrome(u32),
    Deno(u32),
    Edge(u32),
    Firefox(u32),
    Ios(u32),
    Node(u32),
    Opera(u32),
    Safari(u32),
    Samsung(u32),
    Electron(u32),
    Hermes,
    Rhino,
}

impl TransformTarget {
    /// Resolve the target to an oxc `(Engine, Version)` pair.
    pub fn resolve(self) -> (Engine, Version) {
        match self {
            | Self::Es2015 => (Engine::Es, Version(2015, 0, 0)),
            | Self::Es2016 => (Engine::Es, Version(2016, 0, 0)),
            | Self::Es2017 => (Engine::Es, Version(2017, 0, 0)),
            | Self::Es2018 => (Engine::Es, Version(2018, 0, 0)),
            | Self::Es2019 => (Engine::Es, Version(2019, 0, 0)),
            | Self::Es2020 => (Engine::Es, Version(2020, 0, 0)),
            | Self::Es2021 => (Engine::Es, Version(2021, 0, 0)),
            | Self::Es2022 => (Engine::Es, Version(2022, 0, 0)),
            | Self::Es2023 => (Engine::Es, Version(2023, 0, 0)),
            | Self::Es2024 => (Engine::Es, Version(2024, 0, 0)),
            | Self::Es2025 => (Engine::Es, Version(2025, 0, 0)),
            | Self::Es2026 => (Engine::Es, Version(2026, 0, 0)),
            | Self::EsNext => (Engine::Es, Version(9999, 0, 0)),
            | Self::Chrome(version) => (Engine::Chrome, major_version(version)),
            | Self::Deno(version) => (Engine::Deno, major_version(version)),
            | Self::Edge(version) => (Engine::Edge, major_version(version)),
            | Self::Firefox(version) => {
                (Engine::Firefox, major_version(version))
            },
            | Self::Ios(version) => (Engine::Ios, major_version(version)),
            | Self::Node(version) => (Engine::Node, major_version(version)),
            | Self::Opera(version) => (Engine::Opera, major_version(version)),
            | Self::Safari(version) => (Engine::Safari, major_version(version)),
            | Self::Samsung(version) => {
                (Engine::Samsung, major_version(version))
            },
            | Self::Electron(version) => {
                (Engine::Electron, major_version(version))
            },
            | Self::Hermes => (Engine::Hermes, Version(1, 0, 0)),
            | Self::Rhino => (Engine::Rhino, Version(1, 7, 15)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_major_version_saturates_beyond_u16() {
        let version: Version = major_version(u32::MAX);

        assert_eq!(version, Version(u16::MAX, 0, 0));
    }

    #[test]
    fn test_resolve_es_years() {
        let (engine, version): (Engine, Version) =
            TransformTarget::Es2020.resolve();

        assert_eq!(engine, Engine::Es);
        assert_eq!(version, Version(2020, 0, 0));
    }

    #[test]
    fn test_resolve_es_next() {
        let (engine, version): (Engine, Version) =
            TransformTarget::EsNext.resolve();

        assert_eq!(engine, Engine::Es);
        assert_eq!(version, Version(9999, 0, 0));
    }

    #[test]
    fn test_resolve_es2015_is_lowest_year() {
        let (_, version): (Engine, Version) = TransformTarget::Es2015.resolve();

        assert_eq!(version, Version(2015, 0, 0));
    }

    #[test]
    fn test_resolve_es2026_is_highest_year() {
        let (_, version): (Engine, Version) = TransformTarget::Es2026.resolve();

        assert_eq!(version, Version(2026, 0, 0));
    }

    #[test]
    fn test_resolve_engine_variants() {
        let expected: [(TransformTarget, Engine); 10] = [
            (TransformTarget::Chrome(120), Engine::Chrome),
            (TransformTarget::Deno(2), Engine::Deno),
            (TransformTarget::Edge(91), Engine::Edge),
            (TransformTarget::Firefox(74), Engine::Firefox),
            (TransformTarget::Ios(13), Engine::Ios),
            (TransformTarget::Node(16), Engine::Node),
            (TransformTarget::Opera(67), Engine::Opera),
            (TransformTarget::Safari(13), Engine::Safari),
            (TransformTarget::Samsung(16), Engine::Samsung),
            (TransformTarget::Electron(13), Engine::Electron),
        ];

        for (target, engine) in expected {
            let (resolved_engine, _): (Engine, Version) = target.resolve();

            assert_eq!(resolved_engine, engine, "{target:?}");
        }
    }

    #[test]
    fn test_resolve_engine_versions_carry_value() {
        let (_, chrome): (Engine, Version) =
            TransformTarget::Chrome(140).resolve();

        assert_eq!(chrome, Version(140, 0, 0));

        let (_, node): (Engine, Version) = TransformTarget::Node(22).resolve();

        assert_eq!(node, Version(22, 0, 0));
    }

    #[test]
    fn test_resolve_hermes() {
        let (engine, version): (Engine, Version) =
            TransformTarget::Hermes.resolve();

        assert_eq!(engine, Engine::Hermes);
        assert_eq!(version, Version(1, 0, 0));
    }

    #[test]
    fn test_resolve_rhino() {
        let (engine, version): (Engine, Version) =
            TransformTarget::Rhino.resolve();

        assert_eq!(engine, Engine::Rhino);
        assert_eq!(version, Version(1, 7, 15));
    }

    #[test]
    fn test_resolve_covers_every_variant() {
        let targets: [TransformTarget; 25] = [
            TransformTarget::Es2015,
            TransformTarget::Es2016,
            TransformTarget::Es2017,
            TransformTarget::Es2018,
            TransformTarget::Es2019,
            TransformTarget::Es2020,
            TransformTarget::Es2021,
            TransformTarget::Es2022,
            TransformTarget::Es2023,
            TransformTarget::Es2024,
            TransformTarget::Es2025,
            TransformTarget::Es2026,
            TransformTarget::EsNext,
            TransformTarget::Chrome(1),
            TransformTarget::Deno(1),
            TransformTarget::Edge(1),
            TransformTarget::Firefox(1),
            TransformTarget::Ios(1),
            TransformTarget::Node(1),
            TransformTarget::Opera(1),
            TransformTarget::Safari(1),
            TransformTarget::Samsung(1),
            TransformTarget::Electron(1),
            TransformTarget::Hermes,
            TransformTarget::Rhino,
        ];

        for target in targets {
            let (engine, version): (Engine, Version) = target.resolve();

            let _ = (engine, version);
        }
    }
}
