import type { DeepReadonly } from "#/@types/common";
import type { Language, SourceType } from "#/@types/grammar";
import type { Program } from "#/@types/program";
import type { SourceMap } from "#/@types/sourcemap/map";

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
     * The grammar of the program. `undefined` means unknown, because ESTree
     * cannot carry grammar.
     */
    language?: Language;
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
 * `transform`. Mutations in observer hooks are local to the hook.
 */
class Ast {
    readonly #program: Program;
    readonly #source: string;
    readonly #language?: Language;
    readonly #map?: SourceMap;

    public constructor(options: AstOptions) {
        this.#program = options.program;
        this.#source = options.source;
        this.#language = options.language;
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
     * The grammar of the program, or `undefined` when it is unknown.
     */
    public get language(): Language | void {
        return this.#language;
    }

    /**
     * The module kind of the program, read through from `program.sourceType`.
     */
    public get sourceType(): SourceType {
        return this.#program.sourceType;
    }

    /**
     * The incremental source map, or `null` when the AST carries none.
     */
    public get map(): DeepReadonly<SourceMap> | null {
        return this.#map ?? null;
    }

    /**
     * Runs `fn` with mutable access to the program and returns its result.
     *
     * A mutation carries forward only when a `transform` hook returns this AST.
     * A mutation in an observer hook is local to the hook.
     */
    public mutate<Result extends void | Promise<void>>(
        fn: (program: Program) => Result,
    ): Result {
        return fn(this.#program);
    }
}

export type { AstOptions };
export { Ast };
