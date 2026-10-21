import type { Program } from "@oxc-project/types";

import type { DeepReadonly } from "#/@types/common";
import type { SourceMap } from "#/@types/sourcemap";

/**
 * Options for constructing an {@link Ast}.
 */
type AstOptions = {
    /**
     * The ESTree program.
     */
    program: Program;
    /**
     * The source text the program's spans anchor to.
     */
    source: string;
    /**
     * An optional incremental source map.
     */
    map?: SourceMap;
};

/**
 * An AST together with the source text its spans anchor to and an optional
 * incremental source map.
 *
 * Every hook that carries an AST receives an `Ast`. Mutations made through
 * {@link mutate} are carried forward only when the AST is returned from
 * `transform`; mutations in observer hooks are local to the hook.
 */
class Ast {
    readonly #program: Program;
    readonly #source: string;
    readonly #map?: SourceMap;

    public constructor(options: AstOptions) {
        this.#program = options.program;
        this.#source = options.source;
        this.#map = options.map;
    }

    /**
     * A deeply read-only view of the ESTree program.
     */
    public get program(): DeepReadonly<Program> {
        return this.#program;
    }

    /**
     * The source text the program's spans anchor to.
     */
    public get source(): Readonly<string> {
        return this.#source;
    }

    /**
     * The incremental source map, or `null` when the AST carries none.
     */
    public get map(): DeepReadonly<SourceMap> | null {
        return this.#map ?? null;
    }

    /**
     * Runs `fn` with mutable access to the program and returns its result.
     */
    public mutate<Result extends void | Promise<void>>(
        fn: (program: Program) => Result,
    ): Result {
        return fn(this.#program);
    }
}

export type { AstOptions };
export { Ast };
