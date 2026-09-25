/**
 * A supported JavaScript language target, by ES edition.
 */
type EsTarget =
    | "es2015"
    | "es2016"
    | "es2017"
    | "es2018"
    | "es2019"
    | "es2020"
    | "es2021"
    | "es2022"
    | "es2023"
    | "es2024"
    | "es2025"
    | "es2026"
    | "esnext";

/**
 * A browser or runtime engine version target, keyed by engine name.
 */
type EngineTarget =
    | { chrome: number }
    | { deno: number }
    | { edge: number }
    | { firefox: number }
    | { ios: number }
    | { node: number }
    | { opera: number }
    | { safari: number }
    | { samsung: number }
    | { electron: number };

/**
 * A compilation target: an ES edition, a fixed runtime, or an engine version.
 */
type TransformTarget = EsTarget | "hermes" | "rhino" | EngineTarget;

export type { EngineTarget, EsTarget, TransformTarget };
